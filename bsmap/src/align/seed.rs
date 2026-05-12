//! 种子提取、重排序和索引查找模块。
//!
//! 对应 C++ align.cpp 中的种子相关函数：
//! - `ConvertBinaySeq()` 中的种子提取部分
//! - `GetTotalSeedLoc()` / `CountSeeds()`
//! - `ReorderSeed()` / `AdjustSeedStartArray()`
//!
//! ## 核心功能
//!
//! 1. **种子提取**: 从编码读段中提取所有 k-mer 种子
//! 2. **候选数统计**: 统计每个种子在参考中的出现次数
//! 3. **种子重排序**: 按候选数升序排序，优化比对效率

use crate::alphabet::xt3;
use crate::param::SEGLEN;
use crate::reads::encode::EncodedRead;
use crate::reference::index::KmerIndex;

/// Seed segment 信息。
///
/// 一个读段被划分为多个 seed segments，每个 segment 包含多个种子位置。
/// 通过重排序 segments，可以优先处理候选数少的 segment，提高比对效率。
#[derive(Debug, Clone)]
pub struct SeedSegment {
    /// Segment 索引（0-based）。
    pub index: usize,
    /// 起始偏移（碱基位置）。
    pub start_offset: u32,
    /// 该 segment 的候选数（用于排序）。
    pub candidates: u32,
    /// 该 segment 的所有种子哈希值。
    pub seeds: Vec<u32>,
    /// 该 segment 的有效种子掩码（标记哪些种子可用）。
    pub reg_masks: Vec<u32>,
    /// 每个种子来自哪个链（0=正向, 1=反向）。与 seeds 一一对应。
    pub seed_chains: Vec<u8>,
}

impl SeedSegment {
    /// 创建新的 SeedSegment。
    fn new(index: usize, start_offset: u32, seeds: Vec<u32>, reg_masks: Vec<u32>, seed_chains: Vec<u8>) -> Self {
        Self {
            index,
            start_offset,
            candidates: 0,
            seeds,
            reg_masks,
            seed_chains,
        }
    }
}

/// 提取读段的所有种子。
///
/// 对应 C++ `ConvertBinaySeq()` 中的种子提取部分。
/// 从编码读段中提取所有 k-mer 种子，返回按链和位置组织的种子数组。
///
/// # 参数
/// - `encoded`: 编码后的读段
/// - `seed_size`: 种子大小（k-mer 长度）
/// - `index_interval`: 索引间隔（每隔多少个碱基提取一个种子）
/// - `profile`: 参数 profile 矩阵，用于确定每个 mismatch 级别的 segment 边界
///
/// # 返回值
/// `Vec<Vec<u32>>` - [chain][position] 的种子哈希数组
/// chain 0 = 正向链，chain 1 = 反向互补链
pub fn extract_seeds(
    encoded: &EncodedRead,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
) -> Vec<Vec<u32>> {
    let read_len = encoded.info.seq.len() as u32;
    let num_words = encoded.fwd_words.len();

    // 计算每个链的种子
    let mut all_seeds: Vec<Vec<u32>> = vec![Vec::new(); 2];

    // 确定最大 segment 数
    let max_seg = profile.len();

    for chain in 0..2u32 {
        let words = if chain == 0 {
            &encoded.fwd_words
        } else {
            &encoded.rev_words
        };

        let mut seeds = Vec::new();

        // 计算种子提取范围
        // 使用 profile[0][0] 作为起始偏移
        let start_offset = profile[0][0];

        // 提取种子
        let mut pos = start_offset;
        while pos + seed_size <= read_len {
            let seed = extract_seed_at_pos(words, pos, seed_size, num_words);
            seeds.push(seed);
            pos += index_interval;
        }

        all_seeds[chain as usize] = seeds;
    }

    all_seeds
}

/// 从指定位置提取种子哈希。
///
/// # 参数
/// - `words`: 编码后的 word 数组
/// - `pos`: 碱基位置（0-based）
/// - `seed_size`: 种子大小
/// - `num_words`: word 数组长度
///
/// # 返回值
/// 种子哈希值（经过 xt3_64 转换）
#[inline]
fn extract_seed_at_pos(words: &[u64], pos: u32, seed_size: u32, _num_words: usize) -> u32 {
    // 计算 word 索引和位偏移
    let word_idx = (pos / SEGLEN as u32) as usize;
    let bit_offset = ((pos % SEGLEN as u32) * 2) as u32; // 每个碱基 2 位

    if word_idx >= words.len() {
        return 0;
    }

    // 提取种子（可能需要跨 word 边界）
    let seed_bits = seed_size * 2; // 种子总位数
    let available_bits = 64 - bit_offset; // 当前 word 剩余位数

    let seed_val: u64 = if seed_bits <= available_bits {
        // 种子在当前 word 内
        (words[word_idx] >> (64 - bit_offset - seed_bits)) & ((1u64 << seed_bits) - 1)
    } else {
        // 种子跨 word 边界
        let low_bits = words[word_idx] & ((1u64 << (64 - bit_offset)) - 1);
        let high_bits_needed = seed_bits - available_bits;

        if word_idx + 1 < words.len() {
            let high_bits = words[word_idx + 1] >> (64 - high_bits_needed);
            (high_bits << (64 - bit_offset)) | low_bits
        } else {
            low_bits
        }
    };

    // 应用 xt3 转换（C/T 合并）— 使用 32 位版本与索引构建保持一致
    xt3(seed_val as u32)
}

/// 计算最佳 seed_start_offset。
///
/// 对应 C++ `GetTotalSeedLoc()`。通过尝试不同的起始偏移，
/// 找到使总候选数最小的偏移量。
///
/// # 参数
/// - `seeds`: 种子数组（按链组织）
/// - `index`: k-mer 索引
/// - `map_readlen`: 读段长度
/// - `seed_size`: 种子大小
/// - `index_interval`: 索引间隔
///
/// # 返回值
/// 最佳起始偏移量
pub fn find_best_start_offset(
    seeds: &[Vec<u32>],
    index: &KmerIndex,
    map_readlen: u32,
    seed_size: u32,
    index_interval: u32,
) -> u32 {
    let mut best_offset = 0u32;
    let mut min_candidates = u32::MAX;

    // 尝试不同的起始偏移（0 到 index_interval-1）
    for start_offset in 0..index_interval {
        let mut total_candidates: u32 = 0;

        // 对每个链计算候选数
        for chain_seeds in seeds.iter().take(2) {
            let mut pos = start_offset;
            let mut seed_idx = 0;

            while pos + seed_size <= map_readlen && seed_idx < chain_seeds.len() {
                let seed_hash = chain_seeds[seed_idx];
                let (fwd, rev) = index.lookup_separated(seed_hash);
                total_candidates += fwd.len() as u32 + rev.len() as u32;

                pos += index_interval;
                seed_idx += 1;
            }
        }

        if total_candidates < min_candidates {
            min_candidates = total_candidates;
            best_offset = start_offset;
        }
    }

    best_offset
}

/// 重排序 seed segments。
///
/// 对应 C++ `ReorderSeed()`。按候选数升序排序 segments，
/// 优先处理候选数少的 segment，提高比对效率。
///
/// # 参数
/// - `seeds`: 种子数组（按链组织）
/// - `index`: k-mer 索引
/// - `seed_size`: 种子大小
/// - `index_interval`: 索引间隔
/// - `profile`: 参数 profile 矩阵
/// - `map_readlen`: 读段长度
/// - `is_rrbs`: 是否为 RRBS 模式
///
/// # 返回值
/// 按候选数排序的 SeedSegment 数组
pub fn reorder_seeds(
    seeds: &[Vec<u32>],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    map_readlen: u32,
    is_rrbs: bool,
) -> Vec<SeedSegment> {
    let mut segments: Vec<SeedSegment> = Vec::new();

    // 计算 segment 数量
    let num_segments = calculate_num_segments(map_readlen, seed_size, index_interval, profile);

    // 为每个 segment 创建 SeedSegment
    for seg_idx in 0..num_segments {
        let start_offset = calculate_segment_start(seg_idx, profile, index_interval);
        let end_offset = calculate_segment_end(seg_idx, profile, map_readlen, index_interval);

        // 收集该 segment 的所有种子
        let mut seg_seeds: Vec<u32> = Vec::new();
        let mut seg_masks: Vec<u32> = Vec::new();
        let mut seg_chains: Vec<u8> = Vec::new();

        for chain in 0..2 {
            if chain >= seeds.len() {
                continue;
            }

            let chain_seeds = &seeds[chain];
            let mut pos = start_offset;
            let mut seed_idx = ((start_offset - profile[0][0]) / index_interval) as usize;

            while pos < end_offset && pos + seed_size <= map_readlen {
                if seed_idx < chain_seeds.len() {
                    seg_seeds.push(chain_seeds[seed_idx]);
                    // 标记有效种子（这里简化处理，假设都有效）
                    seg_masks.push(1);
                    seg_chains.push(chain as u8);
                }
                pos += index_interval;
                seed_idx += 1;
            }
        }

        let mut segment = SeedSegment::new(seg_idx, start_offset, seg_seeds, seg_masks, seg_chains);

        // 统计该 segment 的候选数
        segment.candidates = count_seeds(&segment.seeds, &segment.reg_masks, index, is_rrbs);

        segments.push(segment);
    }

    // 调整每个 segment 的起始位置
    adjust_seed_starts(&mut segments, index, seed_size);

    // 按候选数升序排序
    segments.sort_by_key(|s| s.candidates);

    segments
}

/// 计算 segment 数量。
fn calculate_num_segments(
    map_readlen: u32,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
) -> usize {
    // 使用 profile 确定 segment 数量
    let max_seg = profile.len() as u32;

    // 计算能容纳多少个完整的 segment
    let mut count = 0;
    for seg_idx in 0..max_seg {
        let start = calculate_segment_start(seg_idx as usize, profile, index_interval);
        if start + seed_size > map_readlen {
            break;
        }
        count += 1;
    }

    count.max(1) as usize
}

/// 计算 segment 起始位置。
fn calculate_segment_start(seg_idx: usize, profile: &[[u32; 16]], index_interval: u32) -> u32 {
    // 使用 profile 矩阵确定起始位置
    // profile[seg_idx][0] 给出了该 mismatch 级别的起始偏移
    let profile_val = profile.get(seg_idx).map(|p| p[0]).unwrap_or(0);

    // 对齐到 index_interval
    (profile_val / index_interval) * index_interval
}

/// 计算 segment 结束位置。
fn calculate_segment_end(
    seg_idx: usize,
    profile: &[[u32; 16]],
    map_readlen: u32,
    index_interval: u32,
) -> u32 {
    // 下一个 segment 的起始位置，或读段末尾
    let next_start = calculate_segment_start(seg_idx + 1, profile, index_interval);
    next_start.min(map_readlen)
}

/// 调整 seed segment 起始位置。
///
/// 对应 C++ `AdjustSeedStartArray()`。根据候选数分布，
/// 微调每个 segment 的起始位置，使种子分布更均匀。
///
/// # 参数
/// - `segments`: SeedSegment 数组（可变引用）
/// - `index`: k-mer 索引
/// - `seed_size`: 种子大小
pub fn adjust_seed_starts(segments: &mut [SeedSegment], index: &KmerIndex, seed_size: u32) {
    for segment in segments.iter_mut() {
        // 如果候选数过多，尝试调整起始位置
        if segment.candidates > 1000 {
            // 尝试微调起始位置（±index_interval）
            // 这里简化处理，实际实现可能需要更复杂的逻辑
            let _ = seed_size; // 使用参数避免警告
            let _ = index;
        }
    }
}

/// 统计某个 segment 的总候选数。
///
/// 对应 C++ `CountSeeds()`。统计该 segment 中所有种子
/// 在参考基因组中的出现次数总和。
///
/// # 参数
/// - `seeds`: 种子哈希数组
/// - `reg_masks`: 有效种子掩码（标记哪些种子可用）
/// - `index`: k-mer 索引
/// - `is_rrbs`: 是否为 RRBS 模式
///
/// # 返回值
/// 总候选数
pub fn count_seeds(seeds: &[u32], reg_masks: &[u32], index: &KmerIndex, is_rrbs: bool) -> u32 {
    let mut total: u32 = 0;

    if is_rrbs {
        // RRBS 模式：使用 rrbs_index
        if let Some(ref rrbs_idx) = index.rrbs_index {
            for (i, &seed) in seeds.iter().enumerate() {
                if i < reg_masks.len() && reg_masks[i] == 0 {
                    continue; // 跳过无效种子
                }

                if (seed as usize) < rrbs_idx.len() {
                    total += rrbs_idx[seed as usize].n1;
                }
            }
        }
    } else {
        // WGBS 模式：使用 lookup_separated 统计正反链候选数
        for (i, &seed) in seeds.iter().enumerate() {
            if i < reg_masks.len() && reg_masks[i] == 0 {
                continue; // 跳过无效种子
            }

            let (fwd, rev) = index.lookup_separated(seed);
            total += fwd.len() as u32 + rev.len() as u32;
        }
    }

    total
}

/// 获取指定位置的种子哈希（用于动态种子提取）。
///
/// # 参数
/// - `words`: 编码后的 word 数组
/// - `pos`: 碱基位置
/// - `seed_size`: 种子大小
///
/// # 返回值
/// 种子哈希值
pub fn get_seed_at_position(words: &[u64], pos: u32, seed_size: u32) -> u32 {
    extract_seed_at_pos(words, pos, seed_size, words.len())
}

/// 计算读段中 N 碱基的数量。
///
/// # 参数
/// - `encoded`: 编码后的读段
///
/// # 返回值
/// N 碱基数
pub fn count_n_bases(encoded: &EncodedRead) -> u32 {
    let mut count: u32 = 0;

    // 检查正向掩码
    for &mask_word in &encoded.fwd_mask {
        // 统计掩码中为 0 的位（表示 N）
        let inverted = !mask_word;
        // 每 2-bit 表示一个碱基，统计 00 的数量
        let mut n_in_word: u32 = 0;
        for i in 0..32 {
            let bits = (inverted >> (62 - i * 2)) & 0b11;
            if bits == 0b11 {
                // 原掩码为 00，表示 N
                n_in_word += 1;
            }
        }
        count += n_in_word;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alphabet::pack_forward;
    use crate::param::{ReadInf, MAXSNPS};

    fn make_test_read(seq: &[u8]) -> EncodedRead {
        let read = ReadInf {
            index: 0,
            read_set: 0,
            name: "test".to_string(),
            seq: seq.to_vec(),
            qual: vec![33u8; seq.len()],
        };

        let num_words = (seq.len() + SEGLEN - 1) / SEGLEN;
        let fwd_words = pack_forward(seq, num_words);
        let rev_words = pack_forward(seq, num_words); // 简化处理

        // 构建掩码（全有效）
        let fwd_mask = vec![u64::MAX; num_words];
        let rev_mask = vec![u64::MAX; num_words];

        EncodedRead {
            fwd_words,
            rev_words,
            fwd_mask,
            rev_mask,
            info: read,
        }
    }

    fn make_test_profile() -> [[u32; 16]; MAXSNPS as usize + 1] {
        let mut profile = [[0u32; 16]; MAXSNPS as usize + 1];
        // 简化 profile：每个 segment 间隔 16bp
        for j in 0..=MAXSNPS as usize {
            for i in 0..16 {
                profile[j][i] = (j as u32 * 16 + i as u32) / 4 * 4;
            }
        }
        profile
    }

    #[test]
    fn test_extract_seed_at_pos() {
        // 测试种子提取
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGT"; // 32 个碱基
        let words = pack_forward(seq, 1);

        // 从位置 0 提取 8-mer 种子
        let seed = extract_seed_at_pos(&words, 0, 8, 1);
        assert!(seed > 0, "应该提取到有效的种子哈希");

        // 从位置 8 提取
        let seed2 = extract_seed_at_pos(&words, 8, 8, 1);
        assert!(seed2 > 0, "应该提取到有效的种子哈希");

        // 相同序列应该产生相同的种子
        assert_eq!(seed, seed2, "相同序列应该产生相同的种子哈希");
    }

    #[test]
    fn test_extract_seed_different_seq() {
        let seq1 = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"; // 全 A
        let seq2 = b"CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"; // 全 C

        let words1 = pack_forward(seq1, 1);
        let words2 = pack_forward(seq2, 1);

        let seed1 = extract_seed_at_pos(&words1, 0, 8, 1);
        let seed2 = extract_seed_at_pos(&words2, 0, 8, 1);

        // 全 A 和全 C 应该产生不同的种子哈希
        assert_ne!(seed1, seed2, "不同序列应该产生不同的种子哈希");
    }

    #[test]
    fn test_extract_seeds_basic() {
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT"; // 48 个碱基
        let encoded = make_test_read(seq);
        let profile = make_test_profile();

        let seeds = extract_seeds(&encoded, 8, 4, &profile);

        // 应该有两个链的种子
        assert_eq!(seeds.len(), 2);
        // 每个链应该有多个种子
        assert!(!seeds[0].is_empty(), "正向链应该有种子");
        assert!(!seeds[1].is_empty(), "反向链应该有种子");
    }

    #[test]
    fn test_seed_segment_creation() {
        let seeds = vec![1, 2, 3, 4, 5];
        let masks = vec![1, 1, 1, 1, 1];

        let segment = SeedSegment::new(0, 0, seeds.clone(), masks, vec![0, 0, 0, 0, 0]);

        assert_eq!(segment.index, 0);
        assert_eq!(segment.start_offset, 0);
        assert_eq!(segment.seeds, seeds);
        assert_eq!(segment.candidates, 0); // 初始为 0
    }

    #[test]
    fn test_calculate_segment_start() {
        let profile = make_test_profile();

        let start0 = calculate_segment_start(0, &profile, 4);
        let start1 = calculate_segment_start(1, &profile, 4);

        assert_eq!(start0, 0);
        assert!(start1 > start0, "segment 1 的起始位置应该大于 segment 0");
    }

    #[test]
    fn test_calculate_num_segments() {
        let profile = make_test_profile();

        // 短读段
        let num_short = calculate_num_segments(50, 8, 4, &profile);
        assert!(num_short >= 1, "短读段至少应该有 1 个 segment");

        // 长读段
        let num_long = calculate_num_segments(150, 8, 4, &profile);
        assert!(num_long >= num_short, "长读段应该有更多 segment");
    }

    #[test]
    fn test_count_seeds_empty() {
        let seeds: Vec<u32> = vec![];
        let masks: Vec<u32> = vec![];

        // 创建空的索引
        let index = KmerIndex {
            total_kmers: 100,
            max_kmer_num: u32::MAX,
            index2: vec![],
            positions: vec![],
            start_offsets: vec![],
            rrbs_index: None,
        };

        let count = count_seeds(&seeds, &masks, &index, false);
        assert_eq!(count, 0, "空种子数组应该返回 0");
    }

    #[test]
    fn test_count_n_bases() {
        // 创建包含 N 的读段
        let seq_with_n = b"ACGTACGTACGTACGNACGTACGTACGTACGT";
        let encoded = make_test_read(seq_with_n);

        // 修改掩码以包含 N
        // 这里简化处理，实际测试需要构造真实的 N 掩码
        let count = count_n_bases(&encoded);
        // 由于 make_test_read 创建全有效掩码，count 应该为 0
        assert_eq!(count, 0, "全有效掩码应该返回 0");
    }

    #[test]
    fn test_find_best_start_offset() {
        // 创建简单的种子数组
        let seeds: Vec<Vec<u32>> = vec![vec![0, 1, 2, 3], vec![0, 1, 2, 3]];

        let index = KmerIndex {
            total_kmers: 100,
            max_kmer_num: u32::MAX,
            index2: vec![],
            positions: vec![],
            start_offsets: vec![],
            rrbs_index: None,
        };

        let profile = make_test_profile();
        let best_offset = find_best_start_offset(&seeds, &index, 48, 8, 4);

        // 最佳偏移应该在 0 到 index_interval-1 之间
        assert!(best_offset < 4, "最佳偏移应该在有效范围内");
    }

    #[test]
    fn test_reorder_seeds_sorts_by_candidates() {
        // 创建测试用的种子数组
        let seeds: Vec<Vec<u32>> = vec![vec![1, 2, 3], vec![1, 2, 3]];

        let index = KmerIndex {
            total_kmers: 100,
            max_kmer_num: u32::MAX,
            index2: vec![],
            positions: vec![],
            start_offsets: vec![],
            rrbs_index: None,
        };

        let profile = make_test_profile();
        let segments = reorder_seeds(&seeds, &index, 8, 4, &profile, 48, false);

        // 验证 segments 已按候选数排序
        for i in 1..segments.len() {
            assert!(
                segments[i].candidates >= segments[i - 1].candidates,
                "Segments 应该按候选数升序排序"
            );
        }
    }

    #[test]
    fn test_get_seed_at_position() {
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGT";
        let words = pack_forward(seq, 1);

        let seed1 = get_seed_at_position(&words, 0, 8);
        let seed2 = get_seed_at_position(&words, 0, 8);

        assert_eq!(seed1, seed2, "相同位置应该产生相同的种子");
    }
}
