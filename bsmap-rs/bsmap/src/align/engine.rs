//! 单端比对引擎模块。
//!
//! 对应 C++ align.cpp 中的 `RunAlign()`、`FilterReads()` 和 `Do_Batch()` 函数。
//! 提供完整的单端比对流程控制，包括种子重排序、扩展比对和结果收集。
//!
//! ## 核心功能
//!
//! 1. **比对主控**: 协调种子提取、重排序和扩展比对
//! 2. **读段过滤**: 基于质量、长度等标准过滤读段
//! 3. **批量处理**: 高效处理一批读段
//! 4. **统计收集**: 记录比对统计信息
//!
//! ## 架构说明
//!
//! C++ BSMAP 按 segment 依次处理两条 read-chain：
//! - 两条链共享动态 mismatch 阈值和分层命中计数
//! - 命中在全局去重后进入对应 read-chain 的计数层
//! - `AddHit()` 的早停结果立即结束当前 segment 扩展

use std::collections::HashSet;
use rayon::prelude::*;

use crate::align::extend::{
    clear_hits, is_unique_hit, select_best_hits, snp_align_segment, HitCollector,
};
use crate::align::seed::{
    extract_seeds_and_masks_into, reorder_seeds_for_chain_with_masks_into, SeedSegment,
};
use crate::param::{AlignConfig, GHit, MAXSNPS};
use crate::reads::encode::EncodedRead;
use crate::reference::binseq::BinSeqCollection;
use crate::reference::index::KmerIndex;

/// 比对结果。
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    /// 读段索引。
    pub read_idx: u32,
    /// 命中列表。
    pub hits: Vec<GHit>,
    /// 是否为唯一比对。
    pub is_unique: bool,
    /// 最佳 mismatch 数。
    pub best_snp: u8,
}

impl AlignmentResult {
    /// 创建新的比对结果。
    pub fn new(read_idx: u32, hits: Vec<GHit>, is_unique: bool, best_snp: u8) -> Self {
        Self {
            read_idx,
            hits,
            is_unique,
            best_snp,
        }
    }

    /// 检查是否有命中。
    pub fn has_hits(&self) -> bool {
        !self.hits.is_empty()
    }
}

/// 单端比对引擎。
///
/// 对应 C++ 中的比对状态管理。维护命中列表和统计信息，
/// 提供完整的单端比对流程。
pub struct SingleAlign {
    /// 命中列表，按 snp_level 组织。
    pub hits: Vec<Vec<GHit>>,
    /// 已比对读段数。
    pub n_aligned: u32,
    /// 唯一比对读段数。
    pub n_unique: u32,
    /// 多重比对读段数。
    pub n_multiple: u32,
    /// C++ hitset: 非 gap 命中去重集。Rust `chr` 已等价于 C++ `chr >> 1`。
    dedup_no_gap: HashSet<(u32, u32)>,
    /// C++ ghitset: gap 命中去重集。Rust `chr` 已等价于 C++ `chr >> 1`。
    dedup_gap: HashSet<(u32, u32)>,
    /// 本 read 内已经做过 no-gap mismatch 的候选。
    tested_no_gap: HashSet<(u32, u32, u8)>,
    /// C++ `x_cur_n_hit[read_chain][snp_level]`，跨 segment 累计。
    level_counts: [[usize; MAXSNPS as usize + 1]; 2],
    /// Worker-local seed hash scratch，跨 read 复用容量。
    seed_chains: [Vec<u32>; 2],
    seed_reg_masks: [Vec<u32>; 2],
    /// Worker-local segment scratch，跨 read 复用容量。
    seed_segments: [Vec<SeedSegment>; 2],
}

impl SingleAlign {
    /// 创建新的比对引擎实例。
    pub fn new() -> Self {
        let mut hits = Vec::with_capacity(MAXSNPS as usize + 1);
        for _ in 0..=MAXSNPS as usize {
            hits.push(Vec::new());
        }

        Self {
            hits,
            n_aligned: 0,
            n_unique: 0,
            n_multiple: 0,
            dedup_no_gap: HashSet::new(),
            dedup_gap: HashSet::new(),
            tested_no_gap: HashSet::new(),
            level_counts: [[0; MAXSNPS as usize + 1]; 2],
            seed_chains: std::array::from_fn(|_| Vec::with_capacity(crate::param::FIXSIZE)),
            seed_reg_masks: std::array::from_fn(|_| Vec::with_capacity(crate::param::FIXSIZE)),
            seed_segments: std::array::from_fn(|_| {
                Vec::with_capacity(MAXSNPS as usize + 1)
            }),
        }
    }

    /// 清空命中集合（含去重集和分层计数）。
    ///
    /// 对应 C++ `ClearHits()` 函数。在比对新读段前调用。
    pub fn clear(&mut self) {
        clear_hits(&mut self.hits);
        self.dedup_no_gap.clear();
        self.dedup_gap.clear();
        self.tested_no_gap.clear();
        self.level_counts.fill([0; MAXSNPS as usize + 1]);
    }

    /// 运行比对（逐 segment 共享两条 read-chain 状态）。
    ///
    /// 对应 C++ `RunAlign()` 函数。执行完整的比对流程：
    /// 1. 提取种子（两条链）
    /// 2. 对每条链重排序种子
    /// 3. 对每条链独立扩展比对
    /// 4. 合并结果
    ///
    /// # 参数
    /// - `encoded`: 编码后的读段
    /// - `index`: k-mer 索引
    /// - `coll`: 二进制参考序列集合
    /// - `config`: 比对配置
    ///
    /// # 返回值
    /// 如果有命中返回 true
    pub fn run_align(
        &mut self,
        encoded: &EncodedRead,
        index: &KmerIndex,
        coll: &BinSeqCollection,
        config: &AlignConfig,
    ) -> bool {
        // 清空之前的命中
        self.clear();

        // 获取读段长度
        let read_len = encoded.read_len();

        // 如果读段太短，跳过
        if read_len < config.min_read_size {
            return false;
        }

        // C++ xflag_chain: 控制处理哪些链
        //   xflag_chain[0] = chains || (read_set < 2)   → 单端总是 true
        //   xflag_chain[1] = chains || (read_set == 2)  → 单端非 chains 模式为 false
        let xflag_chain0 = config.chains || encoded.read_set < 2;
        let xflag_chain1 = config.chains || encoded.read_set == 2;

        // 提取种子（两条链）
        extract_seeds_and_masks_into(
            encoded,
            config.seed_size,
            &mut self.seed_chains,
            &mut self.seed_reg_masks,
        );

        // 计算最大允许的 mismatch 数（C++ 动态阈值：在处理过程中会被降低）
        let max_snp = if config.max_snp_num >= 100 {
            // 百分比模式：(val - 100)% of read length
            ((config.max_snp_num - 100) as f64 / 100.0 * read_len as f64) as u32
        } else {
            config.max_snp_num
        };

        // 预计算两条链的 segments（C++ 逐 segment 处理架构）
        // 仅对启用的链（xflag_chain）进行重排序
        let cross_chain_enabled = config.paired_end || config.chains;
        self.seed_segments[0].clear();
        if xflag_chain0 && !self.seed_chains[0].is_empty() {
            reorder_seeds_for_chain_with_masks_into(
                &self.seed_chains[0],
                &self.seed_reg_masks[0],
                index,
                config.seed_size,
                config.index_interval,
                &config.profile,
                read_len,
                config.rrbs_flag,
                0,
                cross_chain_enabled,
                &mut self.seed_segments[0],
            );
        }
        self.seed_segments[1].clear();
        if xflag_chain1 && !self.seed_chains[1].is_empty() {
            reorder_seeds_for_chain_with_masks_into(
                &self.seed_chains[1],
                &self.seed_reg_masks[1],
                index,
                config.seed_size,
                config.index_interval,
                &config.profile,
                read_len,
                config.rrbs_flag,
                1,
                cross_chain_enabled,
                &mut self.seed_segments[1],
            );
        }

        let num_segments = self.seed_segments[0]
            .len()
            .max(self.seed_segments[1].len());

        // C++ 两条 read-chain 共享一个动态 mismatch 阈值。
        let mut snp_thres = max_snp;

        // C++ 逐 segment 处理：segment → chain，找到命中即早停
        for seg_idx in 0..num_segments {
            let mut early_stop = false;
            for read_chain in 0..2u8 {
                // C++ xflag_chain 过滤
                if !match read_chain {
                    0 => xflag_chain0,
                    1 => xflag_chain1,
                    _ => false,
                } {
                    continue;
                }
                let chain_segs = match read_chain {
                    0 => &self.seed_segments[0],
                    1 => &self.seed_segments[1],
                    _ => unreachable!(),
                };
                if seg_idx >= chain_segs.len() {
                    continue;
                }
                let segment = &chain_segs[seg_idx];

                let query = if read_chain == 0 {
                    encoded.fwd_words()
                } else {
                    encoded.rev_words()
                };
                let mask = if read_chain == 0 {
                    encoded.fwd_mask()
                } else {
                    encoded.rev_mask()
                };
                let n_count = crate::align::extend::count_n_in_mask(mask, read_len);

                let mut collector = HitCollector::new(
                    &mut self.hits,
                    &mut self.level_counts,
                    &mut snp_thres,
                    config.max_num_hits as usize,
                    &coll.chr_lengths,
                    read_len,
                    &mut self.dedup_no_gap,
                    &mut self.dedup_gap,
                    &mut self.tested_no_gap,
                );
                early_stop = snp_align_segment(
                    encoded,
                    index,
                    coll,
                    segment,
                    read_chain,
                    config.gap,
                    config.nt3,
                    config.randseed,
                    cross_chain_enabled,
                    query,
                    mask,
                    n_count,
                    &mut collector,
                );
                if early_stop {
                    break;
                }
            }

            if early_stop {
                break;
            }

            // C++ RunAlign: segment i 后检查累计的 mismatch level 0..=i。
            if !config.nt3 {
                let check_levels = (seg_idx + 1).min(MAXSNPS as usize + 1);
                let has_hits = (0..check_levels).any(|level| {
                    self.level_counts[0][level] > 0 || self.level_counts[1][level] > 0
                });
                if has_hits {
                    break;
                }
            }
        }

        // 更新统计（对应 C++ StringAlign: 找最佳层，统计命中数）
        let mut best_count = 0usize;
        for level in &self.hits {
            if !level.is_empty() {
                best_count = level.len();
                break;
            }
        }

        if best_count > 0 {
            self.n_aligned += 1;
            if best_count == 1 {
                self.n_unique += 1;
            } else {
                self.n_multiple += 1;
            }
        }

        best_count > 0
    }

    /// 过滤读段。
    ///
    /// 对应 C++ `FilterReads()` 函数。基于质量、长度等标准
    /// 判断读段是否应被过滤。
    ///
    /// # 参数
    /// - `encoded`: 编码后的读段
    /// - `config`: 比对配置
    ///
    /// # 返回值
    /// 如果读段应被过滤返回 true
    pub fn filter_read(encoded: &EncodedRead, config: &AlignConfig) -> bool {
        let read_len = encoded.read_len();

        // 检查长度
        if read_len < config.min_read_size {
            return true;
        }

        if read_len > config.max_read_len {
            return true;
        }

        // 检查 N 碱基数
        let n_count = count_n_bases(encoded);
        if n_count > config.max_ns {
            return true;
        }

        // 检查质量（如果配置了质量阈值）
        if config.qual_threshold > 0 {
            if encoded.low_qual_count as u32 > read_len / 2 {
                return true;
            }
        }

        false
    }

    /// 处理一批读段。
    ///
    /// 对应 C++ `Do_Batch()` 函数。批量处理多个读段，
    /// 返回每个读段的比对结果。
    ///
    /// # 参数
    /// - `reads`: 编码后的读段数组
    /// - `index`: k-mer 索引
    /// - `coll`: 二进制参考序列集合
    /// - `config`: 比对配置
    ///
    /// # 返回值
    /// 比对结果数组
    pub fn do_batch(
        &mut self,
        reads: &[EncodedRead],
        index: &KmerIndex,
        coll: &BinSeqCollection,
        config: &AlignConfig,
    ) -> Vec<AlignmentResult> {
        reads
            .par_iter()
            .enumerate()
            .map_init(SingleAlign::new, |aligner, (idx, encoded)| {
                if Self::filter_read(encoded, config) {
                    return AlignmentResult::new(idx as u32, Vec::new(), false, 0);
                }

                let has_hits = aligner.run_align(encoded, index, coll, config);
                let (best_hits, best_snp) = if has_hits {
                    select_best_hits(&aligner.hits)
                } else {
                    (Vec::new(), 0)
                };
                let is_unique = is_unique_hit(&aligner.hits);

                AlignmentResult::new(idx as u32, best_hits, is_unique, best_snp)
            })
            .collect()
    }

    /// 获取最佳层命中数（对应 C++：第一个非空层的命中数）。
    pub fn hit_count(&self) -> usize {
        for level in &self.hits {
            if !level.is_empty() {
                return level.len();
            }
        }
        0
    }

    /// 获取最佳命中。
    pub fn get_best_hits(&self) -> (Vec<GHit>, u8) {
        select_best_hits(&self.hits)
    }

    /// 重置统计信息。
    pub fn reset_stats(&mut self) {
        self.n_aligned = 0;
        self.n_unique = 0;
        self.n_multiple = 0;
    }
}

impl Default for SingleAlign {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算读段中 N 碱基的数量。
fn count_n_bases(encoded: &EncodedRead) -> u32 {
    let mut count: u32 = 0;
    let total_bases = encoded.read_len() as usize;

    let mut bases_checked = 0usize;
    for &mask_word in encoded.fwd_mask() {
        // 统计掩码中为 0 的位（表示 N）
        let inverted = !mask_word;
        for i in 0..32 {
            if bases_checked >= total_bases {
                break;
            }
            let bits = (inverted >> (62 - i * 2)) & 0b11;
            if bits == 0b11 {
                count += 1;
            }
            bases_checked += 1;
        }
    }

    count
}

/// 比对配置构建器。
///
/// 提供流式 API 构建比对配置。
pub struct AlignConfigBuilder {
    config: AlignConfig,
}

impl AlignConfigBuilder {
    /// 创建新的构建器。
    pub fn new() -> Self {
        Self {
            config: AlignConfig::default(),
        }
    }

    /// 设置种子大小。
    pub fn seed_size(mut self, size: u32) -> Self {
        self.config.set_seed_size(size);
        self
    }

    /// 设置最大 mismatch 数。
    pub fn max_mismatch(mut self, max: u32) -> Self {
        self.config.max_snp_num = max;
        self
    }

    /// 设置最大命中数。
    pub fn max_hits(mut self, max: u32) -> Self {
        self.config.max_num_hits = max;
        self
    }

    /// 设置 gap 大小。
    pub fn gap_size(mut self, size: u32) -> Self {
        self.config.gap = size;
        self
    }

    /// 设置索引间隔。
    pub fn index_interval(mut self, interval: u32) -> Self {
        self.config.index_interval = interval;
        self.config.init_profile();
        self
    }

    /// 设置 RRBS 模式。
    pub fn rrbs(mut self, enabled: bool) -> Self {
        self.config.rrbs_flag = enabled;
        self
    }

    /// 设置 3-核苷酸模式。
    pub fn nt3(mut self, enabled: bool) -> Self {
        self.config.nt3 = enabled;
        self
    }

    /// 构建配置。
    pub fn build(self) -> AlignConfig {
        self.config
    }
}

impl Default for AlignConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::ReadInf;
    use crate::reads::encode_read;
    use crate::reference::fasta::Reference;

    fn make_test_read(seq: &[u8]) -> EncodedRead {
        let read = ReadInf {
            index: 0,
            read_set: 0,
            name: "test".to_string(),
            seq: seq.to_vec(),
            qual: vec![33u8; seq.len()],
        };

        encode_read(&read)
    }

    fn make_test_index() -> (KmerIndex, BinSeqCollection) {
        let refs = vec![Reference {
            name: "chr1".to_string(),
            seq: b"ACGTACGTACGTACGTACGTACGTACGTACGT".repeat(100).to_vec(),
            len: 3200,
        }];

        let coll = BinSeqCollection::from_references(&refs);
        let index = KmerIndex::build_wgbs(&coll, 8, 4, 0.01);

        (index, coll)
    }

    #[test]
    fn test_single_align_new() {
        let aligner = SingleAlign::new();
        assert_eq!(aligner.hits.len(), MAXSNPS as usize + 1);
        assert_eq!(aligner.hit_count(), 0);
    }

    #[test]
    fn test_single_align_clear() {
        let mut aligner = SingleAlign::new();

        // 添加一些命中
        aligner.hits[0].push(GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        });

        assert_eq!(aligner.hit_count(), 1);

        aligner.clear();
        assert_eq!(aligner.hit_count(), 0);
    }

    #[test]
    fn test_filter_read_too_short() {
        let config = AlignConfigBuilder::new()
            .seed_size(16)
            .build();

        let short_read = make_test_read(b"ACGT");
        assert!(SingleAlign::filter_read(&short_read, &config));

        let long_read = make_test_read(b"ACGTACGTACGTACGTACGTACGTACGTACGT");
        assert!(!SingleAlign::filter_read(&long_read, &config));
    }

    #[test]
    fn test_alignment_result() {
        let hits = vec![GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        }];

        let result = AlignmentResult::new(0, hits.clone(), true, 0);

        assert_eq!(result.read_idx, 0);
        assert_eq!(result.hits.len(), 1);
        assert!(result.is_unique);
        assert_eq!(result.best_snp, 0);
        assert!(result.has_hits());
    }

    #[test]
    fn test_alignment_result_no_hits() {
        let result = AlignmentResult::new(0, Vec::new(), false, 0);
        assert!(!result.has_hits());
    }

    #[test]
    fn test_align_config_builder() {
        let config = AlignConfigBuilder::new()
            .seed_size(12)
            .max_mismatch(5)
            .max_hits(50)
            .gap_size(2)
            .index_interval(2)
            .rrbs(true)
            .nt3(true)
            .build();

        assert_eq!(config.seed_size, 12);
        assert_eq!(config.max_snp_num, 5);
        assert_eq!(config.max_num_hits, 50);
        assert_eq!(config.gap, 2);
        assert_eq!(config.index_interval, 2);
        assert!(config.rrbs_flag);
        assert!(config.nt3);
    }

    #[test]
    fn test_reset_stats() {
        let mut aligner = SingleAlign::new();
        aligner.n_aligned = 10;
        aligner.n_unique = 5;
        aligner.n_multiple = 5;

        aligner.reset_stats();

        assert_eq!(aligner.n_aligned, 0);
        assert_eq!(aligner.n_unique, 0);
        assert_eq!(aligner.n_multiple, 0);
    }

    #[test]
    fn test_get_best_hits() {
        let mut aligner = SingleAlign::new();

        aligner.hits[2].push(GHit {
            loc: 200,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 2,
        });

        aligner.hits[0].push(GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        });

        let (best_hits, best_snp) = aligner.get_best_hits();

        assert_eq!(best_snp, 0);
        assert_eq!(best_hits.len(), 1);
        assert_eq!(best_hits[0].loc, 100);
    }
}
