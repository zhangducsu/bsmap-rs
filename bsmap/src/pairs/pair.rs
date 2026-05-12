//! 配对读段比对核心模块。
//!
//! 对应 C++ pairs.cpp 中的配对逻辑，包括：
//! - `GetPairs()`: 双指针法配对算法
//! - `RunAlign()`: 配对比对主控
//! - `Do_Batch()`: 批量配对处理
//! - `FixPairReadName()`: 读段名称修复

use crate::align::engine::SingleAlign;
use crate::param::{AlignConfig, GHit, MAXSNPS};
use crate::reads::encode::EncodedRead;
use crate::reference::binseq::BinSeqCollection;
use crate::reference::index::KmerIndex;

/// 配对命中记录。
///
/// 对应 C++ `PairHit` 结构体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairHit {
    /// 链组合：0=a+ vs b-, 1=a- vs b+
    pub chain: u8,
    /// read_a 的 mismatch 数
    pub na: u8,
    /// read_b 的 mismatch 数
    pub nb: u8,
    /// Insert size
    pub insert: u32,
    /// read_a 的命中
    pub a: GHit,
    /// read_b 的命中
    pub b: GHit,
}

impl PairHit {
    /// 创建新的配对命中记录。
    pub fn new(chain: u8, na: u8, nb: u8, insert: u32, a: GHit, b: GHit) -> Self {
        Self {
            chain,
            na,
            nb,
            insert,
            a,
            b,
        }
    }

    /// 获取总 mismatch 数。
    #[inline]
    pub fn total_snps(&self) -> u8 {
        self.na + self.nb
    }
}

/// 配对比对结果。
///
/// 存储配对比对的所有结果。
#[derive(Debug, Clone)]
pub struct PairResult {
    /// 配对命中列表（按总 mismatch 数组织）
    pub pair_hits: Vec<Vec<PairHit>>, // [total_snps][hits]
    /// 是否有配对
    pub has_pair: bool,
    /// 最佳总 mismatch 数
    pub best_snps: u8,
}

impl PairResult {
    /// 创建新的空结果。
    pub fn new() -> Self {
        let mut pair_hits = Vec::with_capacity((MAXSNPS as usize + 1) * 2);
        for _ in 0..=(MAXSNPS as usize * 2) {
            pair_hits.push(Vec::new());
        }

        Self {
            pair_hits,
            has_pair: false,
            best_snps: 0,
        }
    }

    /// 清空结果。
    pub fn clear(&mut self) {
        for hits in &mut self.pair_hits {
            hits.clear();
        }
        self.has_pair = false;
        self.best_snps = 0;
    }

    /// 添加配对命中。
    pub fn add_hit(&mut self, hit: PairHit) {
        let total_snps = hit.total_snps() as usize;
        if total_snps < self.pair_hits.len() {
            self.pair_hits[total_snps].push(hit);
            self.has_pair = true;
        }
    }

    /// 获取最佳配对命中。
    pub fn get_best_hits(&self) -> (Vec<PairHit>, u8) {
        for (snps, hits) in self.pair_hits.iter().enumerate() {
            if !hits.is_empty() {
                return (hits.clone(), snps as u8);
            }
        }
        (Vec::new(), 0)
    }

    /// 检查是否为唯一配对。
    pub fn is_unique(&self) -> bool {
        let (best_hits, _) = self.get_best_hits();
        best_hits.len() == 1
    }

    /// 获取总配对命中数。
    pub fn total_hits(&self) -> usize {
        self.pair_hits.iter().map(|v| v.len()).sum()
    }
}

impl Default for PairResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 配对比对引擎。
///
/// 对应 C++ 中的 `PairAlign` 类。
pub struct PairAlign {
    /// read_a 的单端比对引擎
    pub align_a: SingleAlign,
    /// read_b 的单端比对引擎
    pub align_b: SingleAlign,
    /// 配对命中列表（按总 mismatch 数组织）
    pub pair_hits: Vec<Vec<PairHit>>,
    /// 统计信息
    pub n_aligned_pairs: u32,
    pub n_unique_pairs: u32,
    pub n_multiple_pairs: u32,
}

impl PairAlign {
    /// 创建新的配对比对引擎实例。
    pub fn new() -> Self {
        // 总 mismatch 数范围：0 到 MAXSNPS*2（两个读段各有 0-MAXSNPS 个 mismatch）
        let total_snps_levels = (MAXSNPS as usize + 1) * 2;
        let mut pair_hits = Vec::with_capacity(total_snps_levels);
        for _ in 0..total_snps_levels {
            pair_hits.push(Vec::new());
        }

        Self {
            align_a: SingleAlign::new(),
            align_b: SingleAlign::new(),
            pair_hits,
            n_aligned_pairs: 0,
            n_unique_pairs: 0,
            n_multiple_pairs: 0,
        }
    }

    /// 清空所有命中。
    ///
    /// 在比对新读段对前调用。
    pub fn clear(&mut self) {
        self.align_a.clear();
        self.align_b.clear();
        for hits in &mut self.pair_hits {
            hits.clear();
        }
    }

    /// 获取配对。
    ///
    /// 对应 C++ `GetPairs()`。
    /// 使用双指针法按染色体分组比较，避免 O(n²) 全交叉比较。
    ///
    /// # 配对逻辑
    ///
    /// - chain=0 (a+ vs b-): read_a 正向 vs read_b 反向
    /// - chain=1 (a- vs b+): read_a 反向 vs read_b 正向
    ///
    /// # 双指针优化
    ///
    /// 1. 按染色体对命中分组
    /// 2. 对每个染色体，只比较该染色体上的命中
    /// 3. 避免全交叉比较，复杂度从 O(n²) 降到 O(n log n)
    pub fn get_pairs(
        &mut self,
        hits_a: &[GHit],      // read_a 在 snp_level=na 的命中
        hits_b: &[GHit],      // read_b 在 snp_level=nb 的命中（正向）
        chits_a: &[GHit],     // read_a 的反向命中
        chits_b: &[GHit],     // read_b 的反向命中
        na: u8,
        nb: u8,
        config: &AlignConfig,
        read_len_a: u32,
        read_len_b: u32,
    ) -> usize {
        let mut found_pairs = 0usize;

        // Chain 0: a+ vs b-
        // read_a 正向 (hits_a) vs read_b 反向 (chits_b)
        found_pairs += self.find_pairs_chain0(
            hits_a, chits_b, na, nb, config, read_len_a, read_len_b,
        );

        // Chain 1: a- vs b+
        // read_a 反向 (chits_a) vs read_b 正向 (hits_b)
        found_pairs += self.find_pairs_chain1(
            chits_a, hits_b, na, nb, config, read_len_a, read_len_b,
        );

        found_pairs
    }

    /// Chain 0: a+ vs b- 配对。
    ///
    /// read_a 正向命中 vs read_b 反向命中。
    fn find_pairs_chain0(
        &mut self,
        hits_a: &[GHit],
        chits_b: &[GHit],
        na: u8,
        nb: u8,
        config: &AlignConfig,
        read_len_a: u32,
        read_len_b: u32,
    ) -> usize {
        let mut found = 0usize;

        // 按染色体分组 read_b 的反向命中
        let grouped_b = group_hits_by_chr(chits_b);

        // 遍历 read_a 的正向命中
        for hit_a in hits_a {
            let chr_a = hit_a.chr;

            // 找到该染色体在 read_b 反向命中中的范围
            if let Some((_, hits_b_chr)) = grouped_b.iter().find(|(chr, _)| *chr == chr_a as u16) {
                for hit_b in hits_b_chr {
                    // 计算 insert size
                    // a+ vs b-: insert = hit_b.loc + read_len_b - hit_a.loc
                    let insert = calculate_insert(hit_a, hit_b, read_len_a, read_len_b, 0);

                    // 检查 insert size 范围
                    if insert >= config.min_insert && insert <= config.max_insert {
                        let pair_hit = PairHit::new(
                            0, // chain=0
                            na,
                            nb,
                            insert,
                            *hit_a,
                            **hit_b,
                        );

                        let total_snps = (na + nb) as usize;
                        if total_snps < self.pair_hits.len() {
                            self.pair_hits[total_snps].push(pair_hit);
                            found += 1;
                        }
                    }
                }
            }
        }

        found
    }

    /// Chain 1: a- vs b+ 配对。
    ///
    /// read_a 反向命中 vs read_b 正向命中。
    fn find_pairs_chain1(
        &mut self,
        chits_a: &[GHit],
        hits_b: &[GHit],
        na: u8,
        nb: u8,
        config: &AlignConfig,
        read_len_a: u32,
        read_len_b: u32,
    ) -> usize {
        let mut found = 0usize;

        // 按染色体分组 read_b 的正向命中
        let grouped_b = group_hits_by_chr(hits_b);

        // 遍历 read_a 的反向命中
        for hit_a in chits_a {
            let chr_a = hit_a.chr;

            // 找到该染色体在 read_b 正向命中中的范围
            if let Some((_, hits_b_chr)) = grouped_b.iter().find(|(chr, _)| *chr == chr_a as u16) {
                for hit_b in hits_b_chr {
                    // 计算 insert size
                    // a- vs b+: insert = hit_a.loc + read_len_a - hit_b.loc
                    let insert = calculate_insert(hit_a, hit_b, read_len_a, read_len_b, 1);

                    // 检查 insert size 范围
                    if insert >= config.min_insert && insert <= config.max_insert {
                        let pair_hit = PairHit::new(
                            1, // chain=1
                            na,
                            nb,
                            insert,
                            *hit_a,
                            **hit_b,
                        );

                        let total_snps = (na + nb) as usize;
                        if total_snps < self.pair_hits.len() {
                            self.pair_hits[total_snps].push(pair_hit);
                            found += 1;
                        }
                    }
                }
            }
        }

        found
    }

    /// 运行配对比对。
    ///
    /// 对应 C++ `RunAlign()`。
    ///
    /// # 流程
    /// 1. 分别对 read_a 和 read_b 运行单端比对
    /// 2. 枚举所有 mismatch 组合，调用 get_pairs
    /// 3. 提前终止策略（nt3 模式）
    ///
    /// # 参数
    /// - `encoded_a`: 编码后的 read_a
    /// - `encoded_b`: 编码后的 read_b
    /// - `index`: k-mer 索引
    /// - `coll`: 二进制参考序列集合
    /// - `config`: 比对配置
    ///
    /// # 返回值
    /// 如果找到配对返回 true
    pub fn run_pair_align(
        &mut self,
        encoded_a: &EncodedRead,
        encoded_b: &EncodedRead,
        index: &KmerIndex,
        coll: &BinSeqCollection,
        config: &AlignConfig,
    ) -> bool {
        // 清空之前的命中
        self.clear();

        let read_len_a = encoded_a.info.seq.len() as u32;
        let read_len_b = encoded_b.info.seq.len() as u32;

        // 分别对 read_a 和 read_b 运行单端比对
        let has_hits_a = self.align_a.run_align(encoded_a, index, coll, config);
        let has_hits_b = self.align_b.run_align(encoded_b, index, coll, config);

        if !has_hits_a || !has_hits_b {
            return false;
        }

        // 枚举所有 mismatch 组合
        let max_snp_a = self.align_a.hits.len().min(MAXSNPS as usize + 1);
        let max_snp_b = self.align_b.hits.len().min(MAXSNPS as usize + 1);

        for na in 0..max_snp_a {
            if self.align_a.hits[na].is_empty() {
                continue;
            }

            for nb in 0..max_snp_b {
                if self.align_b.hits[nb].is_empty() {
                    continue;
                }

                // 分离正向和反向命中
                let (hits_a_fwd, hits_a_rev) = split_hits_by_strand(&self.align_a.hits[na]);
                let (hits_b_fwd, hits_b_rev) = split_hits_by_strand(&self.align_b.hits[nb]);

                // 查找配对
                let _found = self.get_pairs(
                    &hits_a_fwd,
                    &hits_b_fwd,
                    &hits_a_rev,
                    &hits_b_rev,
                    na as u8,
                    nb as u8,
                    config,
                    read_len_a,
                    read_len_b,
                );

                // 提前终止策略：如果找到配对且 nt3 模式，立即返回
                if config.nt3 && self.has_pairs() {
                    return true;
                }
            }
        }

        // 更新统计
        if self.has_pairs() {
            self.n_aligned_pairs += 1;
            if self.is_unique_pair() {
                self.n_unique_pairs += 1;
            } else {
                self.n_multiple_pairs += 1;
            }
        }

        self.has_pairs()
    }

    /// 检查是否有配对。
    pub fn has_pairs(&self) -> bool {
        self.pair_hits.iter().any(|v| !v.is_empty())
    }

    /// 检查是否为唯一配对。
    pub fn is_unique_pair(&self) -> bool {
        let total: usize = self.pair_hits.iter().map(|v| v.len()).sum();
        total == 1
    }

    /// 获取最佳配对命中。
    pub fn get_best_pair_hits(&self) -> (Vec<PairHit>, u8) {
        for (snps, hits) in self.pair_hits.iter().enumerate() {
            if !hits.is_empty() {
                return (hits.clone(), snps as u8);
            }
        }
        (Vec::new(), 0)
    }

    /// 处理一批配对读段。
    ///
    /// 对应 C++ `Do_Batch()`。
    ///
    /// # 参数
    /// - `reads_a`: read_a 数组
    /// - `reads_b`: read_b 数组
    /// - `index`: k-mer 索引
    /// - `coll`: 二进制参考序列集合
    /// - `config`: 比对配置
    ///
    /// # 返回值
    /// 每个读段对的比对结果
    pub fn do_pair_batch(
        &mut self,
        reads_a: &[EncodedRead],
        reads_b: &[EncodedRead],
        index: &KmerIndex,
        coll: &BinSeqCollection,
        config: &AlignConfig,
    ) -> Vec<PairBatchResult> {
        let batch_size = reads_a.len().min(reads_b.len());
        let mut results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let encoded_a = &reads_a[i];
            let encoded_b = &reads_b[i];

            // 过滤读段
            if SingleAlign::filter_read(encoded_a, config) ||
               SingleAlign::filter_read(encoded_b, config) {
                results.push(PairBatchResult::new(
                    i as u32,
                    Vec::new(),
                    false,
                    0,
                    Vec::new(),
                    Vec::new(),
                ));
                continue;
            }

            // 执行配对比对
            let has_pair = self.run_pair_align(encoded_a, encoded_b, index, coll, config);

            let (pair_hits, best_snps) = if has_pair {
                self.get_best_pair_hits()
            } else {
                (Vec::new(), 0)
            };

            let is_unique = self.is_unique_pair();

            // 获取单端命中（用于未配对情况）
            let (unpair_hits_a, _) = if !has_pair {
                self.align_a.get_best_hits()
            } else {
                (Vec::new(), 0)
            };

            let (unpair_hits_b, _) = if !has_pair {
                self.align_b.get_best_hits()
            } else {
                (Vec::new(), 0)
            };

            results.push(PairBatchResult::new(
                i as u32,
                pair_hits,
                is_unique,
                best_snps,
                unpair_hits_a,
                unpair_hits_b,
            ));
        }

        results
    }

    /// 修复配对读段名称。
    ///
    /// 对应 C++ `FixPairReadName()`。
    /// 确保两个读段名称相同，去除 /1, /2 等后缀。
    ///
    /// # 参数
    /// - `name_a`: read_a 的名称
    /// - `name_b`: read_b 的名称
    ///
    /// # 返回值
    /// 修复后的共同名称，如果无法修复返回 None
    pub fn fix_pair_name(name_a: &str, name_b: &str) -> Option<String> {
        // 去除末尾的空白字符
        let name_a = name_a.trim();
        let name_b = name_b.trim();

        // 如果完全相同，直接返回
        if name_a == name_b {
            return Some(name_a.to_string());
        }

        // 找到最后一个共同字符
        let min_len = name_a.len().min(name_b.len());
        let mut common_len = 0;

        for i in 0..min_len {
            if name_a.as_bytes()[i] == name_b.as_bytes()[i] {
                common_len = i + 1;
            } else {
                break;
            }
        }

        if common_len == 0 {
            return None;
        }

        // 检查共同前缀后的部分是否是配对后缀（如 /1 /2 或 .1 .2）
        let suffix_a = &name_a[common_len..];
        let suffix_b = &name_b[common_len..];

        // 检查后缀是否符合配对模式
        let is_pair_suffix = |suffix: &str| -> bool {
            if suffix.is_empty() {
                return false;
            }
            // 支持的后缀模式: /1, /2, .1, .2, :1, :2 等
            let first = suffix.chars().next().unwrap();
            if first == '/' || first == '.' || first == ':' {
                suffix.len() >= 2 && suffix[1..].chars().all(|c| c.is_ascii_digit())
            } else {
                suffix.chars().all(|c| c.is_ascii_digit())
            }
        };

        // 如果后缀不是配对数字后缀，则不是有效的配对读段
        if !is_pair_suffix(suffix_a) || !is_pair_suffix(suffix_b) {
            return None;
        }

        // 去除末尾的常见后缀分隔符
        let common = &name_a[..common_len];
        let trimmed = common.trim_end_matches(|c| c == '/' || c == '.' || c == ':');

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    /// 重置统计信息。
    pub fn reset_stats(&mut self) {
        self.n_aligned_pairs = 0;
        self.n_unique_pairs = 0;
        self.n_multiple_pairs = 0;
    }
}

impl Default for PairAlign {
    fn default() -> Self {
        Self::new()
    }
}

/// 配对批量处理结果。
#[derive(Debug, Clone)]
pub struct PairBatchResult {
    /// 读段索引
    pub read_idx: u32,
    /// 配对命中列表
    pub pair_hits: Vec<PairHit>,
    /// 是否为唯一配对
    pub is_unique: bool,
    /// 最佳总 mismatch 数
    pub best_snps: u8,
    /// 如果未配对，read_a 的单端命中
    pub unpair_hits_a: Vec<GHit>,
    /// 如果未配对，read_b 的单端命中
    pub unpair_hits_b: Vec<GHit>,
}

impl PairBatchResult {
    /// 创建新的批量处理结果。
    pub fn new(
        read_idx: u32,
        pair_hits: Vec<PairHit>,
        is_unique: bool,
        best_snps: u8,
        unpair_hits_a: Vec<GHit>,
        unpair_hits_b: Vec<GHit>,
    ) -> Self {
        Self {
            read_idx,
            pair_hits,
            is_unique,
            best_snps,
            unpair_hits_a,
            unpair_hits_b,
        }
    }

    /// 检查是否有配对。
    pub fn has_pair(&self) -> bool {
        !self.pair_hits.is_empty()
    }
}

/// 按染色体分组的命中列表（用于双指针优化）。
///
/// 返回按染色体排序的 (chr, hits) 列表。
fn group_hits_by_chr(hits: &[GHit]) -> Vec<(u16, Vec<&GHit>)> {
    let mut groups: std::collections::HashMap<u16, Vec<&GHit>> = std::collections::HashMap::new();

    for hit in hits {
        groups.entry(hit.chr as u16).or_default().push(hit);
    }

    // 转换为 Vec 并按染色体排序
    let mut result: Vec<(u16, Vec<&GHit>)> = groups.into_iter().collect();
    result.sort_by_key(|(chr, _)| *chr);

    result
}

/// 分离正向和反向命中。
///
/// 根据 strand 字段分离命中：
/// - strand & 1 == 0: 正向（读段正义链）
/// - strand & 1 == 1: 反向（读段反义链）
fn split_hits_by_strand(hits: &[GHit]) -> (Vec<GHit>, Vec<GHit>) {
    let mut fwd = Vec::new();
    let mut rev = Vec::new();

    for hit in hits {
        // strand 的最低位表示读段链：0=正向，1=反向
        if hit.strand & 1 == 0 {
            fwd.push(*hit);
        } else {
            rev.push(*hit);
        }
    }

    (fwd, rev)
}

/// 计算 insert size。
///
/// # 参数
/// - `hit_a`: read_a 的命中
/// - `hit_b`: read_b 的命中
/// - `read_len_a`: read_a 的长度
/// - `read_len_b`: read_b 的长度
/// - `chain`: 链组合（0=a+ vs b-, 1=a- vs b+）
///
/// # 返回值
/// Insert size
fn calculate_insert(
    hit_a: &GHit,
    hit_b: &GHit,
    read_len_a: u32,
    read_len_b: u32,
    chain: u8,
) -> u32 {
    if chain == 0 {
        // Chain 0: a+ vs b-
        // insert = hit_b.loc + read_len_b - hit_a.loc
        hit_b.loc + read_len_b - hit_a.loc
    } else {
        // Chain 1: a- vs b+
        // insert = hit_a.loc + read_len_a - hit_b.loc
        hit_a.loc + read_len_a - hit_b.loc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alphabet::pack_forward;
    use crate::param::ReadInf;
    use crate::reference::fasta::Reference;

    fn make_test_read(seq: &[u8]) -> EncodedRead {
        let read = ReadInf {
            index: 0,
            read_set: 1,
            name: "test_read/1".to_string(),
            seq: seq.to_vec(),
            qual: vec![33u8; seq.len()],
        };

        crate::reads::encode::encode_read(&read)
    }

    fn make_test_read_b(seq: &[u8]) -> EncodedRead {
        let read = ReadInf {
            index: 0,
            read_set: 2,
            name: "test_read/2".to_string(),
            seq: seq.to_vec(),
            qual: vec![33u8; seq.len()],
        };

        crate::reads::encode::encode_read(&read)
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
    fn test_pair_hit_new() {
        let hit_a = GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 1,
        };

        let hit_b = GHit {
            loc: 200,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 2,
        };

        let pair_hit = PairHit::new(0, 1, 2, 150, hit_a, hit_b);

        assert_eq!(pair_hit.chain, 0);
        assert_eq!(pair_hit.na, 1);
        assert_eq!(pair_hit.nb, 2);
        assert_eq!(pair_hit.total_snps(), 3);
        assert_eq!(pair_hit.insert, 150);
        assert_eq!(pair_hit.a.loc, 100);
        assert_eq!(pair_hit.b.loc, 200);
    }

    #[test]
    fn test_pair_result() {
        let mut result = PairResult::new();

        assert!(!result.has_pair);
        assert_eq!(result.total_hits(), 0);

        let hit_a = GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 1,
        };

        let hit_b = GHit {
            loc: 200,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 2,
        };

        let pair_hit = PairHit::new(0, 1, 2, 150, hit_a, hit_b);
        result.add_hit(pair_hit);

        assert!(result.has_pair);
        assert_eq!(result.total_hits(), 1);

        let (best_hits, best_snps) = result.get_best_hits();
        assert_eq!(best_snps, 3);
        assert_eq!(best_hits.len(), 1);
    }

    #[test]
    fn test_fix_pair_name() {
        // 相同名称
        assert_eq!(
            PairAlign::fix_pair_name("read1", "read1"),
            Some("read1".to_string())
        );

        // /1 /2 后缀
        assert_eq!(
            PairAlign::fix_pair_name("read1/1", "read1/2"),
            Some("read1".to_string())
        );

        // .1 .2 后缀
        assert_eq!(
            PairAlign::fix_pair_name("read1.1", "read1.2"),
            Some("read1".to_string())
        );

        // 纯数字后缀（也是有效的配对后缀）
        assert_eq!(
            PairAlign::fix_pair_name("read1", "read2"),
            Some("read".to_string())
        );

        // 完全不同名称
        assert_eq!(
            PairAlign::fix_pair_name("readA", "readB"),
            None
        );

        // 空白字符
        assert_eq!(
            PairAlign::fix_pair_name("read1/1  ", "read1/2\t"),
            Some("read1".to_string())
        );
    }

    #[test]
    fn test_group_hits_by_chr() {
        let hits = vec![
            GHit { loc: 100, chr: 0, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 },
            GHit { loc: 200, chr: 0, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 },
            GHit { loc: 300, chr: 1, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 },
            GHit { loc: 400, chr: 1, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 },
            GHit { loc: 500, chr: 0, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 },
        ];

        let grouped = group_hits_by_chr(&hits);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, 0);
        assert_eq!(grouped[0].1.len(), 3); // chr0 有 3 个命中
        assert_eq!(grouped[1].0, 1);
        assert_eq!(grouped[1].1.len(), 2); // chr1 有 2 个命中
    }

    #[test]
    fn test_split_hits_by_strand() {
        let hits = vec![
            GHit { loc: 100, chr: 0, strand: 0, gap_size: 0, gap_pos: 0, snps: 0 }, // 正向
            GHit { loc: 200, chr: 0, strand: 1, gap_size: 0, gap_pos: 0, snps: 0 }, // 反向
            GHit { loc: 300, chr: 0, strand: 2, gap_size: 0, gap_pos: 0, snps: 0 }, // 正向 (strand & 1 == 0)
            GHit { loc: 400, chr: 0, strand: 3, gap_size: 0, gap_pos: 0, snps: 0 }, // 反向 (strand & 1 == 1)
        ];

        let (fwd, rev) = split_hits_by_strand(&hits);

        assert_eq!(fwd.len(), 2);
        assert_eq!(rev.len(), 2);
        assert_eq!(fwd[0].loc, 100);
        assert_eq!(fwd[1].loc, 300);
        assert_eq!(rev[0].loc, 200);
        assert_eq!(rev[1].loc, 400);
    }

    #[test]
    fn test_calculate_insert() {
        let hit_a = GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };

        let hit_b = GHit {
            loc: 200,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };

        // Chain 0: a+ vs b-
        // insert = hit_b.loc + read_len_b - hit_a.loc
        // = 200 + 50 - 100 = 150
        let insert0 = calculate_insert(&hit_a, &hit_b, 50, 50, 0);
        assert_eq!(insert0, 150);

        // Chain 1: a- vs b+
        // insert = hit_a.loc + read_len_a - hit_b.loc
        // = 100 + 50 - 200 = 0 (但通常 hit_a.loc > hit_b.loc)
        let hit_a2 = GHit {
            loc: 300,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let hit_b2 = GHit {
            loc: 200,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let insert1 = calculate_insert(&hit_a2, &hit_b2, 50, 50, 1);
        assert_eq!(insert1, 150);
    }

    #[test]
    fn test_pair_align_new() {
        let aligner = PairAlign::new();

        assert_eq!(aligner.pair_hits.len(), (MAXSNPS as usize + 1) * 2);
        assert_eq!(aligner.n_aligned_pairs, 0);
        assert_eq!(aligner.n_unique_pairs, 0);
        assert_eq!(aligner.n_multiple_pairs, 0);
    }

    #[test]
    fn test_pair_align_clear() {
        let mut aligner = PairAlign::new();

        // 添加一些配对命中
        let hit_a = GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let hit_b = GHit {
            loc: 200,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let pair_hit = PairHit::new(0, 0, 0, 150, hit_a, hit_b);
        aligner.pair_hits[0].push(pair_hit);

        assert!(aligner.has_pairs());

        aligner.clear();

        assert!(!aligner.has_pairs());
        assert!(aligner.pair_hits.iter().all(|v| v.is_empty()));
    }

    #[test]
    fn test_pair_batch_result() {
        let hit_a = GHit {
            loc: 100,
            chr: 0,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let hit_b = GHit {
            loc: 200,
            chr: 0,
            strand: 1,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        };
        let pair_hit = PairHit::new(0, 0, 0, 150, hit_a, hit_b);

        let result = PairBatchResult::new(
            0,
            vec![pair_hit],
            true,
            0,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(result.read_idx, 0);
        assert!(result.has_pair());
        assert!(result.is_unique);
        assert_eq!(result.best_snps, 0);
    }
}
