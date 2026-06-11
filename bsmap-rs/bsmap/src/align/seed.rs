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
//!
//! ## 架构说明
//!
//! C++ BSMAP 采用**逐链独立**架构：
//! - 每条链（chain=0 正向, chain=1 反向）独立调用 `ReorderSeed()`
//! - 每条链独立选择 `xseed_start_offset[chain]`
//! - 每条链独立执行 `AdjustSeedStartArray()`
//! - 每条链独立排序 segment
//! - 最后 `SnpAlign()` 也是逐链独立执行
//!
//! Rust 版本已重构为与 C++ 一致的逐链独立架构。

use crate::alphabet::xt3;
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
    /// 每个种子在读段中的碱基位置（0-based）。与 seeds 一一对应。
    pub seed_positions: Vec<u32>,
}

impl SeedSegment {
    /// 创建新的 SeedSegment。
    fn new(index: usize, start_offset: u32, seeds: Vec<u32>, reg_masks: Vec<u32>, seed_positions: Vec<u32>) -> Self {
        Self {
            index,
            start_offset,
            candidates: 0,
            seeds,
            reg_masks,
            seed_positions,
        }
    }
}

/// 提取读段的所有种子。
///
/// 对应 C++ `ConvertBinaySeq()` + `ReorderSeed()` 中的种子提取部分。
/// 从编码读段中提取所有 k-mer 种子，返回按链和位置组织的种子数组。
///
/// 关键算法：C++ BSMAP 提取所有位置（0 到 read_len - seed_size）的种子，
/// 然后在 ReorderSeed 中根据 profile 和 index_interval 选择使用哪些种子。
pub fn extract_seeds(
    encoded: &EncodedRead,
    seed_size: u32,
    _index_interval: u32,
    _profile: &[[u32; 16]],
) -> Vec<Vec<u32>> {
    let read_len = encoded.info.seq.len() as u32;
    let num_words = encoded.fwd_words.len();
    
    let mut all_seeds: Vec<Vec<u32>> = vec![Vec::new(); 2];

    for chain in 0..2u32 {
        let words = if chain == 0 {
            &encoded.fwd_words
        } else {
            &encoded.rev_words
        };

        let mut seeds = Vec::new();

        // 提取所有位置的种子（每个位置都提取）
        // 对应 C++ 的 xseed_array[chain][pos]
        let mut pos = 0u32;
        while pos + seed_size <= read_len {
            let seed = extract_seed_at_pos(words, pos, seed_size, num_words);
            seeds.push(seed);
            pos += 1;
        }

        all_seeds[chain as usize] = seeds;
    }

    all_seeds
}

/// 从指定位置提取种子哈希。
#[inline]
fn extract_seed_at_pos(words: &[u64], pos: u32, seed_size: u32, _num_words: usize) -> u32 {
    let bit_pos = pos * 2;
    let seed_bits_lz = (32 - seed_size) * 2;

    let word_idx = (bit_pos / 64) as usize;
    let bit_offset = (bit_pos % 64) as u32;

    if word_idx >= words.len() {
        return 0;
    }

    let straddle: u64 = if bit_offset == 0 {
        words[word_idx]
    } else if word_idx + 1 < words.len() {
        (words[word_idx] << bit_offset)
            | (words[word_idx + 1] >> (64 - bit_offset))
    } else {
        words[word_idx] << bit_offset
    };

    let result = xt3((straddle >> seed_bits_lz) as u32);
    result
}

/// 计算最佳 seed_start_offset（逐链独立）。
///
/// 对应 C++ `GetTotalSeedLoc()`。通过尝试不同的起始偏移，
/// 找到使总候选数**最小**的偏移量。
///
/// 注意：C++ 选择候选数最少的偏移以优化比对速度。
pub fn find_best_start_offset(
    chain_seeds: &[u32],
    index: &KmerIndex,
    map_readlen: u32,
    seed_size: u32,
    index_interval: u32,
    is_rrbs: bool,
) -> u32 {
    // C++ 搜索范围：0 到 (map_readlen - index_interval + 1) % seed_size - 1
    let num_offsets = ((map_readlen - index_interval + 1) % seed_size).max(1).min(index_interval);

    let mut best_offset = 0u32;
    let mut best_candidates = u32::MAX;

    for start_offset in 0..num_offsets {
        let mut total_candidates: u32 = 0;

        let mut pos = start_offset;
        let mut seed_idx = start_offset as usize;

        while pos + seed_size <= map_readlen && seed_idx < chain_seeds.len() {
            let seed_hash = chain_seeds[seed_idx];
            if is_rrbs {
                if let Some(ref rrbs_idx) = index.rrbs_index {
                    if (seed_hash as usize) < rrbs_idx.len() {
                        total_candidates += rrbs_idx[seed_hash as usize].n1;
                    }
                }
            } else {
                let (fwd, rev) = index.lookup_separated(seed_hash);
                total_candidates += (fwd.len() + rev.len()) as u32;
            }

            pos += index_interval;
            seed_idx += index_interval as usize;
        }

        if total_candidates > 0 && total_candidates < best_candidates {
            best_candidates = total_candidates;
            best_offset = start_offset;
        }
    }

    best_offset
}

/// 为单条链重排序 seed segments。
///
/// 对应 C++ `ReorderSeed()` + `AdjustSeedStartArray()`。
/// 逐链独立执行：选择最佳偏移、创建 segments、调整起始位置、排序。
pub fn reorder_seeds_for_chain(
    chain_seeds: &[u32],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    map_readlen: u32,
    is_rrbs: bool,
    read_chain: u8,
) -> Vec<SeedSegment> {
    // C++ RRBS: cseed_offset = map_readlen % seed_size
    // 用于 read_chain=1 时偏移种子位置，补偿 RC 编码的相位差
    let cseed_offset = if is_rrbs { map_readlen % seed_size } else { 0 };

    // 1. 找到最佳起始偏移（对应 C++ 的 xseed_start_offset[chain]）
    let best_start_offset = find_best_start_offset(
        chain_seeds,
        index,
        map_readlen,
        seed_size,
        index_interval,
        is_rrbs,
    );

    // 2. 计算 segment 数量
    let num_segments = calculate_num_segments(map_readlen, seed_size, index_interval, profile);

    // 3. 为每个 segment 创建 SeedSegment
    let mut segments: Vec<SeedSegment> = Vec::with_capacity(num_segments);

    for seg_idx in 0..num_segments {
        let mut seg_seeds: Vec<u32> = Vec::with_capacity(index_interval as usize);
        let mut seg_masks: Vec<u32> = Vec::with_capacity(index_interval as usize);
        let mut seg_positions: Vec<u32> = Vec::with_capacity(index_interval as usize);

        // C++ RRBS: xseeds[chain][seg][0] = xseed_array[chain][profile[seg][0] + cseed_offset * read_chain]
        // C++ WGBS: xseeds[chain][seg][ii] = xseed_array[chain][profile[seg][ii] + start - ii]
        for ii in 0..index_interval as usize {
            let profile_val = if seg_idx < profile.len() && ii < profile[seg_idx].len() {
                profile[seg_idx][ii]
            } else {
                continue;
            };

            let mut seed_pos = (profile_val + best_start_offset).saturating_sub(ii as u32);
            if is_rrbs && read_chain == 1 {
                seed_pos = seed_pos.saturating_add(cseed_offset);
            }

            if seed_pos < chain_seeds.len() as u32 && seed_pos + seed_size <= map_readlen {
                seg_seeds.push(chain_seeds[seed_pos as usize]);
                seg_masks.push(1);
                seg_positions.push(seed_pos);
            }
        }

        let mut segment = SeedSegment::new(seg_idx, best_start_offset, seg_seeds, seg_masks, seg_positions);

        // 统计该 segment 的候选数
        segment.candidates = count_seeds_for_chain(&segment.seeds, &segment.reg_masks, index, is_rrbs);

        segments.push(segment);
    }

    // 4. 调整每个 segment 的起始位置（AdjustSeedStartArray）
    adjust_seed_starts_for_chain(&mut segments, index, seed_size, index_interval, map_readlen, chain_seeds, profile, is_rrbs);

    // C++ cmodeindex 反转：chain 1 将 segment 索引反转，
    // 使两端第一批种子都从读段起始位置附近开始采样
    if read_chain == 1 {
        let n = segments.len();
        let orig_data: Vec<_> = segments.iter().map(|s| {
            (s.seeds.clone(), s.reg_masks.clone(), s.seed_positions.clone(), s.start_offset)
        }).collect();
        for s in segments.iter_mut() {
            let rev_idx = n - 1 - s.index;
            s.seeds.clone_from(&orig_data[rev_idx].0);
            s.reg_masks.clone_from(&orig_data[rev_idx].1);
            s.seed_positions.clone_from(&orig_data[rev_idx].2);
            s.start_offset = orig_data[rev_idx].3;
        }
    }

    // 5. 按候选数升序排序
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
    let max_seg = profile.len() as u32;

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
    let profile_val = profile.get(seg_idx).map(|p| p[0]).unwrap_or(0);
    (profile_val / index_interval) * index_interval
}

/// 调整 seed segment 起始位置（逐链独立）。
///
/// 对应 C++ `AdjustSeedStartArray()`。核心算法：
/// 1. 从中间向两端交替扩展遍历 segment
/// 2. 每个 segment 的 start 必须在相邻 segment 的 start 之间（保持顺序）
/// 3. 在允许范围内找使该 segment 候选数最小的 start
fn adjust_seed_starts_for_chain(
    segments: &mut [SeedSegment],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    map_readlen: u32,
    chain_seeds: &[u32],
    profile: &[[u32; 16]],
    is_rrbs: bool,
) {
    if segments.is_empty() {
        return;
    }

    let num_segments = segments.len();
    let max_offset = (map_readlen - index_interval + 1) % seed_size;

    // 创建临时数组存储调整后的起始位置
    let mut start_array: Vec<u32> = segments.iter().map(|s| s.start_offset).collect();

    // 从中间向两端交替扩展遍历
    // C++: for(i=0; i<seedseg_num; i++) { if(i%2==0) ptr = i/2; else ptr = seedseg_num - 1 - i/2; }
    for i in 0..num_segments {
        let ptr = if i % 2 == 0 {
            i / 2
        } else {
            num_segments - 1 - i / 2
        };

        let mut min_candidates = u32::MAX;
        let mut best_start = start_array[ptr];

        // 确定搜索边界：不能超过相邻 segment 的 start
        let start_bound = if ptr == 0 {
            0
        } else {
            start_array[ptr - 1]
        };

        let end_bound = if ptr == num_segments - 1 {
            max_offset
        } else {
            start_array[ptr + 1]
        };

        // 在 [start_bound, end_bound] 范围内找使该 segment 候选数最小的 start
        for start in start_bound..=end_bound {
            let candidates = count_seeds_at_offset(ptr, start, chain_seeds, index, seed_size, index_interval, map_readlen, profile, is_rrbs);
            if candidates < min_candidates {
                min_candidates = candidates;
                best_start = start;
            }
        }

        start_array[ptr] = best_start;
    }

    // 应用调整后的起始位置并重新提取种子
    for (i, segment) in segments.iter_mut().enumerate() {
        let new_start = start_array[i];
        if new_start == segment.start_offset {
            continue;
        }
        segment.start_offset = new_start;

        // Re-extract seeds at the new start offset
        segment.seeds.clear();
        segment.reg_masks.clear();
        segment.seed_positions.clear();

        for ii in 0..index_interval as usize {
            let profile_val = if i < profile.len() && ii < profile[i].len() {
                profile[i][ii]
            } else {
                continue;
            };

            let seed_pos = (profile_val + new_start).saturating_sub(ii as u32);

            if seed_pos < chain_seeds.len() as u32 && seed_pos + seed_size <= map_readlen {
                segment.seeds.push(chain_seeds[seed_pos as usize]);
                segment.reg_masks.push(1);
                segment.seed_positions.push(seed_pos);
            }
        }

        // Update candidate count
        segment.candidates = count_seeds_at_offset(i, new_start, chain_seeds, index, seed_size, index_interval, map_readlen, profile, is_rrbs);
    }
}

/// 统计某个 segment 在指定起始偏移下的候选数。
///
/// 根据新的 start_offset 和 profile 重新选择种子位置，计算候选数。
fn count_seeds_at_offset(
    seg_idx: usize,
    start_offset: u32,
    chain_seeds: &[u32],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    map_readlen: u32,
    profile: &[[u32; 16]],
    is_rrbs: bool,
) -> u32 {
    let mut total: u32 = 0;

    for ii in 0..index_interval as usize {
        let profile_val = if seg_idx < profile.len() && ii < profile[seg_idx].len() {
            profile[seg_idx][ii]
        } else {
            continue;
        };

        let seed_pos = (profile_val + start_offset).saturating_sub(ii as u32);

        if seed_pos < chain_seeds.len() as u32 && seed_pos + seed_size <= map_readlen {
            let seed_hash = chain_seeds[seed_pos as usize];
            if is_rrbs {
                if let Some(ref rrbs_idx) = index.rrbs_index {
                    if (seed_hash as usize) < rrbs_idx.len() {
                        total += rrbs_idx[seed_hash as usize].n1;
                    }
                }
            } else {
                let (fwd, rev) = index.lookup_separated(seed_hash);
                total += (fwd.len() + rev.len()) as u32;
            }
        }
    }

    // Match C++: if(total==0) total=9999999
    // Without this, 0 candidates beats positive candidates, causing wrong pick
    if total == 0 {
        u32::MAX
    } else {
        total
    }
}

/// 统计某个 segment 的总候选数（逐链独立）。
pub fn count_seeds_for_chain(seeds: &[u32], reg_masks: &[u32], index: &KmerIndex, is_rrbs: bool) -> u32 {
    let mut total: u32 = 0;

    if is_rrbs {
        if let Some(ref rrbs_idx) = index.rrbs_index {
            for (i, &seed) in seeds.iter().enumerate() {
                if i < reg_masks.len() && reg_masks[i] == 0 {
                    continue;
                }
                if (seed as usize) < rrbs_idx.len() {
                    total += rrbs_idx[seed as usize].n1;
                }
            }
        }
    } else {
        // Match C++: CountSeeds uses ref.index2[s].n[0] which in C++ is total of both chains
        for (i, &seed) in seeds.iter().enumerate() {
            if i < reg_masks.len() && reg_masks[i] == 0 {
                continue;
            }
            let (fwd, rev) = index.lookup_separated(seed);
            total += (fwd.len() + rev.len()) as u32;
        }
    }

    total
}

/// 获取指定位置的种子哈希。
pub fn get_seed_at_position(words: &[u64], pos: u32, seed_size: u32) -> u32 {
    extract_seed_at_pos(words, pos, seed_size, words.len())
}

/// 计算读段中 N 碱基的数量。
pub fn count_n_bases(encoded: &EncodedRead) -> u32 {
    let mut count: u32 = 0;

    for &mask_word in &encoded.fwd_mask {
        let inverted = !mask_word;
        let mut n_in_word: u32 = 0;
        for i in 0..32 {
            let bits = (inverted >> (62 - i * 2)) & 0b11;
            if bits == 0b11 {
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
    use crate::param::{ReadInf, MAXSNPS, SEGLEN};

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
        let rev_words = pack_forward(seq, num_words);

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
        for j in 0..=MAXSNPS as usize {
            for i in 0..16 {
                profile[j][i] = (j as u32 * 16 + i as u32) / 4 * 4;
            }
        }
        profile
    }

    #[test]
    fn test_extract_seed_at_pos() {
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGT";
        let words = pack_forward(seq, 1);

        let seed = extract_seed_at_pos(&words, 0, 8, 1);
        assert!(seed > 0, "应该提取到有效的种子哈希");

        let seed2 = extract_seed_at_pos(&words, 8, 8, 1);
        assert!(seed2 > 0, "应该提取到有效的种子哈希");

        assert_eq!(seed, seed2, "相同序列应该产生相同的种子哈希");
    }

    #[test]
    fn test_extract_seeds_basic() {
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT";
        let encoded = make_test_read(seq);
        let profile = make_test_profile();

        let seeds = extract_seeds(&encoded, 8, 4, &profile);

        assert_eq!(seeds.len(), 2);
        assert!(!seeds[0].is_empty(), "正向链应该有种子");
        assert!(!seeds[1].is_empty(), "反向链应该有种子");
    }

    #[test]
    fn test_find_best_start_offset() {
        let seeds: Vec<u32> = vec![0, 1, 2, 3, 4, 5, 6, 7];

        let index = KmerIndex {
            total_kmers: 100,
            max_kmer_num: u32::MAX,
            index2: vec![],
            positions: vec![],
            start_offsets: vec![],
            rrbs_index: None,
            seed_size: 16,
        };

        let best_offset = find_best_start_offset(&seeds, &index, 48, 8, 4, false);
        assert!(best_offset < 4, "最佳偏移应该在有效范围内");
    }
}
