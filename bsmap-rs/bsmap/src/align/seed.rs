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
#[cfg(test)]
use crate::reference::index::RRBS_BSC_FLAG;

const NO_SEED_CANDIDATES: u32 = 9_999_999;
const MAX_SEGMENT_SEEDS: usize = 16;
const MAX_SEED_SEGMENTS: usize = crate::param::MAXSNPS as usize + 1;

/// Seed segment 信息。
///
/// 一个读段被划分为多个 seed segments，每个 segment 包含多个种子位置。
/// 通过重排序 segments，可以优先处理候选数少的 segment，提高比对效率。
#[derive(Debug, Clone, Copy)]
pub struct SeedSegment {
    /// Segment 索引（0-based）。
    pub index: usize,
    /// 起始偏移（碱基位置）。
    pub start_offset: u32,
    /// 该 segment 的候选数（用于排序）。
    pub candidates: u32,
    /// 该 segment 的所有种子哈希值。
    seeds: [u32; MAX_SEGMENT_SEEDS],
    /// 该 segment 的有效种子掩码（标记哪些种子可用）。
    reg_masks: [u32; MAX_SEGMENT_SEEDS],
    /// 每个种子在读段中的碱基位置（0-based）。与 seeds 一一对应。
    seed_positions: [u32; MAX_SEGMENT_SEEDS],
    seed_count: u8,
}

impl SeedSegment {
    /// 创建新的 SeedSegment。
    const fn new(index: usize, start_offset: u32) -> Self {
        Self {
            index,
            start_offset,
            candidates: 0,
            seeds: [0; MAX_SEGMENT_SEEDS],
            reg_masks: [0; MAX_SEGMENT_SEEDS],
            seed_positions: [0; MAX_SEGMENT_SEEDS],
            seed_count: 0,
        }
    }

    #[inline]
    fn push_seed(&mut self, seed: u32, reg_mask: u32, position: u32) {
        let index = self.seed_count as usize;
        debug_assert!(index < MAX_SEGMENT_SEEDS);
        if index >= MAX_SEGMENT_SEEDS {
            return;
        }
        self.seeds[index] = seed;
        self.reg_masks[index] = reg_mask;
        self.seed_positions[index] = position;
        self.seed_count += 1;
    }

    #[inline]
    fn clear_seeds(&mut self) {
        self.seed_count = 0;
    }

    #[inline]
    pub(crate) fn seeds(&self) -> &[u32] {
        &self.seeds[..self.seed_count as usize]
    }

    #[inline]
    pub(crate) fn reg_masks(&self) -> &[u32] {
        &self.reg_masks[..self.seed_count as usize]
    }

    #[inline]
    pub(crate) fn seed_positions(&self) -> &[u32] {
        &self.seed_positions[..self.seed_count as usize]
    }

    #[inline]
    fn copy_seed_data_from(&mut self, source: &Self) {
        self.start_offset = source.start_offset;
        self.seeds = source.seeds;
        self.reg_masks = source.reg_masks;
        self.seed_positions = source.seed_positions;
        self.seed_count = source.seed_count;
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
    let mut all_seeds = [Vec::new(), Vec::new()];
    extract_seeds_into(encoded, seed_size, &mut all_seeds);
    Vec::from(all_seeds)
}

/// 将两条 read-chain 的所有 seed 写入可复用 worker scratch。
pub(crate) fn extract_seeds_into(
    encoded: &EncodedRead,
    seed_size: u32,
    all_seeds: &mut [Vec<u32>; 2],
) {
    let mut unused_masks = [Vec::new(), Vec::new()];
    extract_seeds_and_masks_into(encoded, seed_size, all_seeds, &mut unused_masks);
}

pub(crate) fn extract_seeds_and_masks_into(
    encoded: &EncodedRead,
    seed_size: u32,
    all_seeds: &mut [Vec<u32>; 2],
    all_reg_masks: &mut [Vec<u32>; 2],
) {
    let read_len = encoded.read_len();
    let num_words = encoded.num_words();
    let seed_count = if read_len >= seed_size {
        (read_len - seed_size + 1) as usize
    } else {
        0
    };

    for chain in 0..2u32 {
        let words = if chain == 0 {
            encoded.fwd_words()
        } else {
            encoded.rev_words()
        };
        let mask_words = if chain == 0 {
            encoded.fwd_mask()
        } else {
            encoded.rev_mask()
        };

        let seeds = &mut all_seeds[chain as usize];
        let reg_masks = &mut all_reg_masks[chain as usize];
        seeds.clear();
        reg_masks.clear();
        if seeds.capacity() < seed_count {
            seeds.reserve(seed_count);
        }
        if reg_masks.capacity() < seed_count {
            reg_masks.reserve(seed_count);
        }

        // 提取所有位置的种子（每个位置都提取）
        // 对应 C++ 的 xseed_array[chain][pos]
        let mut pos = 0u32;
        while pos + seed_size <= read_len {
            let seed = extract_seed_at_pos(words, pos, seed_size, num_words);
            seeds.push(seed);
            reg_masks.push(extract_seed_reg_mask_at_pos(mask_words, pos, seed_size));
            pos += 1;
        }

    }
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

#[inline]
fn extract_seed_reg_mask_at_pos(mask_words: &[u64], pos: u32, seed_size: u32) -> u32 {
    let bit_pos = pos * 2;
    let seed_bits_lz = (32 - seed_size) * 2;

    let word_idx = (bit_pos / 64) as usize;
    let bit_offset = (bit_pos % 64) as u32;

    if word_idx >= mask_words.len() {
        return 0;
    }

    let straddle = if bit_offset == 0 {
        mask_words[word_idx]
    } else if word_idx + 1 < mask_words.len() {
        (mask_words[word_idx] << bit_offset) | (mask_words[word_idx + 1] >> (64 - bit_offset))
    } else {
        mask_words[word_idx] << bit_offset
    };
    let mask_bits = (straddle >> seed_bits_lz) as u32;
    let seed_bits = if seed_size >= 16 {
        u32::MAX
    } else {
        (1u32 << (seed_size * 2)) - 1
    };
    (!mask_bits) & seed_bits
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
    profile: &[[u32; 16]],
    num_segments: usize,
    is_rrbs: bool,
) -> u32 {
    let zero_masks = vec![0u32; chain_seeds.len()];
    find_best_start_offset_with_masks(
        chain_seeds,
        &zero_masks,
        index,
        map_readlen,
        seed_size,
        index_interval,
        profile,
        num_segments,
        is_rrbs,
    )
}

fn find_best_start_offset_with_masks(
    chain_seeds: &[u32],
    chain_reg_masks: &[u32],
    index: &KmerIndex,
    map_readlen: u32,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    num_segments: usize,
    is_rrbs: bool,
) -> u32 {
    // C++ 搜索范围：0 到 (map_readlen - index_interval + 1) % seed_size - 1
    let num_offsets = (map_readlen - index_interval + 1) % seed_size;

    let mut best_offset = 0u32;
    let mut best_candidates = u32::MAX;

    for start_offset in 0..num_offsets {
        let total_candidates = (0..num_segments).fold(0u32, |total, segment| {
            total.saturating_add(count_seeds_at_offset(
                segment,
                start_offset,
                chain_seeds,
                chain_reg_masks,
                index,
                seed_size,
                index_interval,
                map_readlen,
                profile,
                is_rrbs,
            ))
        });

        if total_candidates < best_candidates {
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
///
/// 此兼容入口保持历史行为并计入全部 RRBS hit；能取得运行参数的调用方应使用
/// [`reorder_seeds_for_chain_with_cross_chain`] 显式传入 `paired_end || chains`。
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
    reorder_seeds_for_chain_with_cross_chain(
        chain_seeds,
        index,
        seed_size,
        index_interval,
        profile,
        map_readlen,
        is_rrbs,
        read_chain,
        true,
    )
}

/// 为单条 read chain 重排 seed segment，并显式控制 RRBS cross-chain 候选计数。
///
/// C++ 仅在 `pairend || chains` 时把带 `RRBS_BSC_FLAG` 的 hit 加入 RRBS 索引。
/// Rust 索引保存两类 hit 的超集，因此在 seed 计数阶段按同一条件过滤。
pub fn reorder_seeds_for_chain_with_cross_chain(
    chain_seeds: &[u32],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    map_readlen: u32,
    is_rrbs: bool,
    read_chain: u8,
    cross_chain_enabled: bool,
) -> Vec<SeedSegment> {
    let mut segments = Vec::new();
    reorder_seeds_for_chain_with_cross_chain_into(
        chain_seeds,
        index,
        seed_size,
        index_interval,
        profile,
        map_readlen,
        is_rrbs,
        read_chain,
        cross_chain_enabled,
        &mut segments,
    );
    segments
}

/// 将重排后的 seed segments 写入可复用 worker scratch。
#[allow(clippy::too_many_arguments)]
pub(crate) fn reorder_seeds_for_chain_with_cross_chain_into(
    chain_seeds: &[u32],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    map_readlen: u32,
    is_rrbs: bool,
    read_chain: u8,
    cross_chain_enabled: bool,
    segments: &mut Vec<SeedSegment>,
) {
    let zero_masks = vec![0u32; chain_seeds.len()];
    reorder_seeds_for_chain_with_masks_into(
        chain_seeds,
        &zero_masks,
        index,
        seed_size,
        index_interval,
        profile,
        map_readlen,
        is_rrbs,
        read_chain,
        cross_chain_enabled,
        segments,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reorder_seeds_for_chain_with_masks_into(
    chain_seeds: &[u32],
    chain_reg_masks: &[u32],
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
    map_readlen: u32,
    is_rrbs: bool,
    read_chain: u8,
    cross_chain_enabled: bool,
    segments: &mut Vec<SeedSegment>,
) {
    // C++ RRBS: cseed_offset = map_readlen % seed_size
    // 用于 read_chain=1 时偏移种子位置，补偿 RC 编码的相位差
    let cseed_offset = if is_rrbs { map_readlen % seed_size } else { 0 };
    let num_segments = calculate_num_segments(map_readlen, seed_size, index_interval, profile);

    // 1. 找到最佳起始偏移（对应 C++ 的 xseed_start_offset[chain]）
    let best_start_offset = if is_rrbs {
        0
    } else {
        find_best_start_offset_with_masks(
            chain_seeds,
            chain_reg_masks,
            index,
            map_readlen,
            seed_size,
            index_interval,
            profile,
            num_segments,
            is_rrbs,
        )
    };

    segments.clear();
    if segments.capacity() < num_segments {
        segments.reserve(num_segments);
    }

    if is_rrbs {
        for seg_idx in 0..num_segments {
            let profile_val = if seg_idx < profile.len() {
                profile[seg_idx][0]
            } else {
                continue;
            };
            let seed_pos = profile_val + cseed_offset * read_chain as u32;
            let mut segment = SeedSegment::new(seg_idx, 0);
            if seed_pos < chain_seeds.len() as u32 && seed_pos + seed_size <= map_readlen {
                let seed_index = seed_pos as usize;
                let reg_mask = chain_reg_masks.get(seed_index).copied().unwrap_or(0);
                segment.push_seed(chain_seeds[seed_index], reg_mask, seed_pos);
            }
            segment.candidates = count_seeds_for_chain_with_cross_chain(
                segment.seeds(),
                segment.reg_masks(),
                index,
                true,
                cross_chain_enabled,
            );
            segments.push(segment);
        }
        segments.sort_by_key(|s| (s.candidates, s.index));
        return;
    }

    // 3. 为每个 segment 创建 SeedSegment
    for seg_idx in 0..num_segments {
        let mut segment = SeedSegment::new(seg_idx, best_start_offset);

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
                let seed_index = seed_pos as usize;
                let reg_mask = chain_reg_masks.get(seed_index).copied().unwrap_or(0);
                segment.push_seed(chain_seeds[seed_index], reg_mask, seed_pos);
            }
        }

        // 统计该 segment 的候选数
        segment.candidates =
            count_seeds_for_chain(segment.seeds(), segment.reg_masks(), index, is_rrbs);

        segments.push(segment);
    }

    // 4. 调整每个 segment 的起始位置（AdjustSeedStartArray）
    adjust_seed_starts_for_chain(
        segments,
        index,
        seed_size,
        index_interval,
        map_readlen,
        chain_seeds,
        chain_reg_masks,
        profile,
        is_rrbs,
    );

    // C++ cmodeindex 反转：chain 1 将 segment 索引反转，
    // 使两端第一批种子都从读段起始位置附近开始采样
    if read_chain == 1 {
        let n = segments.len();
        let mut original = [SeedSegment::new(0, 0); MAX_SEED_SEGMENTS];
        original[..n].copy_from_slice(segments);
        for s in segments.iter_mut() {
            let rev_idx = n - 1 - s.index;
            s.copy_seed_data_from(&original[rev_idx]);
        }
    }

    // 5. 按候选数升序排序
    segments.sort_by_key(|s| s.candidates);
}

/// 计算 segment 数量。
fn calculate_num_segments(
    map_readlen: u32,
    seed_size: u32,
    index_interval: u32,
    profile: &[[u32; 16]],
) -> usize {
    (map_readlen
        .saturating_sub(index_interval - 1)
        .checked_div(seed_size)
        .unwrap_or(0)
        .max(1)
        .min(profile.len() as u32) as usize)
        .min(MAX_SEED_SEGMENTS)
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
    chain_reg_masks: &[u32],
    profile: &[[u32; 16]],
    is_rrbs: bool,
) {
    if segments.is_empty() {
        return;
    }

    let num_segments = segments.len();
    let max_offset = (map_readlen - index_interval + 1) % seed_size;

    // 创建栈上临时数组存储调整后的起始位置
    let mut start_array = [0u32; MAX_SEED_SEGMENTS];
    for (target, segment) in start_array.iter_mut().zip(segments.iter()) {
        *target = segment.start_offset;
    }

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
            let candidates = count_seeds_at_offset(ptr, start, chain_seeds, chain_reg_masks, index, seed_size, index_interval, map_readlen, profile, is_rrbs);
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
        segment.clear_seeds();

        for ii in 0..index_interval as usize {
            let profile_val = if i < profile.len() && ii < profile[i].len() {
                profile[i][ii]
            } else {
                continue;
            };

            let seed_pos = (profile_val + new_start).saturating_sub(ii as u32);

            if seed_pos < chain_seeds.len() as u32 && seed_pos + seed_size <= map_readlen {
                let seed_index = seed_pos as usize;
                let reg_mask = chain_reg_masks.get(seed_index).copied().unwrap_or(0);
                segment.push_seed(chain_seeds[seed_index], reg_mask, seed_pos);
            }
        }

        // Update candidate count
        segment.candidates = count_seeds_at_offset(i, new_start, chain_seeds, chain_reg_masks, index, seed_size, index_interval, map_readlen, profile, is_rrbs);
    }
}

/// 统计某个 segment 在指定起始偏移下的候选数。
///
/// 根据新的 start_offset 和 profile 重新选择种子位置，计算候选数。
fn count_seeds_at_offset(
    seg_idx: usize,
    start_offset: u32,
    chain_seeds: &[u32],
    chain_reg_masks: &[u32],
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
            let seed_index = seed_pos as usize;
            let seed_hash = chain_seeds[seed_index];
            let reg_mask = chain_reg_masks.get(seed_index).copied().unwrap_or(0);
            let count = if is_rrbs {
                index.rrbs_candidate_count(seed_hash, true)
            } else {
                index.wgbs_candidate_count(seed_hash)
            };
            total = total.saturating_add(weight_seed_count(count, reg_mask));
        }
    }

    // Match C++: if(total==0) total=9999999
    // Without this, 0 candidates beats positive candidates, causing wrong pick
    if total == 0 {
        NO_SEED_CANDIDATES
    } else {
        total
    }
}

/// 统计某个 segment 的总候选数（逐链独立）。
///
/// 此兼容入口保持历史行为并计入全部 RRBS hit。
pub fn count_seeds_for_chain(
    seeds: &[u32],
    reg_masks: &[u32],
    index: &KmerIndex,
    is_rrbs: bool,
) -> u32 {
    count_seeds_for_chain_with_cross_chain(seeds, reg_masks, index, is_rrbs, true)
}

/// 统计单条 read chain 的 seed 候选数，并按 C++ `pairend || chains` 语义过滤
/// RRBS cross-chain hit。WGBS 不使用该标记，行为不受 `cross_chain_enabled` 影响。
pub fn count_seeds_for_chain_with_cross_chain(
    seeds: &[u32],
    reg_masks: &[u32],
    index: &KmerIndex,
    is_rrbs: bool,
    cross_chain_enabled: bool,
) -> u32 {
    let mut total: u32 = 0;

    if is_rrbs {
        for (i, &seed) in seeds.iter().enumerate() {
            let reg_mask = reg_masks.get(i).copied().unwrap_or(0);
            let count = index.rrbs_candidate_count(seed, cross_chain_enabled);
            total = total.saturating_add(weight_seed_count(count, reg_mask));
        }
    } else {
        // Match C++: CountSeeds uses ref.index2[s].n[0] which in C++ is total of both chains
        for (i, &seed) in seeds.iter().enumerate() {
            let reg_mask = reg_masks.get(i).copied().unwrap_or(0);
            let count = index.wgbs_candidate_count(seed);
            total = total.saturating_add(weight_seed_count(count, reg_mask));
        }
    }

    if total == 0 {
        NO_SEED_CANDIDATES
    } else {
        total
    }
}

#[inline]
fn weight_seed_count(count: u32, reg_mask: u32) -> u32 {
    if reg_mask == 0 {
        count
    } else {
        count.saturating_mul(1 << 12)
    }
}

/// 获取指定位置的种子哈希。
pub fn get_seed_at_position(words: &[u64], pos: u32, seed_size: u32) -> u32 {
    extract_seed_at_pos(words, pos, seed_size, words.len())
}

/// 计算读段中 N 碱基的数量。
pub fn count_n_bases(encoded: &EncodedRead) -> u32 {
    let mut count: u32 = 0;

    for &mask_word in encoded.fwd_mask() {
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
    use crate::param::{Hit, KmerLoc2, ReadInf, MAXSNPS};
    use crate::reads::encode::encode_read;
    use std::sync::OnceLock;

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
    fn extract_seeds_into_reuses_worker_buffers() {
        let encoded = make_test_read(b"ACGTACGTACGTACGTACGTACGTACGTACGT");
        let mut scratch = [Vec::new(), Vec::new()];
        extract_seeds_into(&encoded, 8, &mut scratch);
        let pointers = [scratch[0].as_ptr(), scratch[1].as_ptr()];
        let capacities = [scratch[0].capacity(), scratch[1].capacity()];
        let expected = scratch.clone();

        extract_seeds_into(&encoded, 8, &mut scratch);

        assert_eq!(scratch, expected);
        assert_eq!([scratch[0].as_ptr(), scratch[1].as_ptr()], pointers);
        assert_eq!([scratch[0].capacity(), scratch[1].capacity()], capacities);
    }

    fn make_rrbs_count_index(buckets: Vec<Vec<Hit>>) -> KmerIndex {
        let mut rrbs_offsets = Vec::with_capacity(buckets.len() + 1);
        let mut rrbs_hits = Vec::new();
        rrbs_offsets.push(0);
        for bucket in buckets {
            rrbs_hits.extend(bucket);
            rrbs_offsets.push(rrbs_hits.len() as u32);
        }
        KmerIndex {
            total_kmers: rrbs_offsets.len().saturating_sub(1) as u32,
            max_kmer_num: u32::MAX,
            index2: Vec::new(),
            positions: Vec::new(),
            start_offsets: Vec::new(),
            rrbs_offsets,
            rrbs_hits,
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            wgbs_occupancy: Vec::new(),
            wgbs_rank: Vec::new(),
            wgbs_buckets: Vec::new(),
            wgbs_overflow: Vec::new(),
            seed_size: 2,
            mapped: None,
            rrbs_normal_counts: OnceLock::new(),
            rrbs_mode_ranges: OnceLock::new(),
        }
    }

    #[test]
    fn test_rrbs_count_seeds_filters_cross_chain_hits_when_disabled() {
        let bucket = vec![
                Hit { chr: 0, loc: 10 },
                Hit { chr: 2, loc: 20 },
                Hit {
                    chr: RRBS_BSC_FLAG,
                    loc: 30,
                },
                Hit {
                    chr: RRBS_BSC_FLAG | 2,
                    loc: 40,
                },
            ];
        let index = make_rrbs_count_index(vec![Vec::new(), bucket]);

        assert_eq!(
            count_seeds_for_chain_with_cross_chain(&[1], &[0], &index, true, false),
            2,
            "SE 默认模式只统计 normal RRBS hit"
        );
        assert_eq!(
            count_seeds_for_chain_with_cross_chain(&[1], &[0], &index, true, true),
            4,
            "PE 或 -n 1 模式统计 normal 与 BSC hit"
        );
    }

    #[test]
    fn rrbs_count_seeds_penalizes_n_seed_like_cpp() {
        let index = make_rrbs_count_index(vec![Vec::new(), vec![Hit { chr: 0, loc: 10 }]]);
        assert_eq!(
            count_seeds_for_chain_with_cross_chain(&[1], &[1], &index, true, false),
            1 << 12
        );
    }

    #[test]
    fn test_rrbs_cross_chain_filter_changes_segment_order() {
        let empty = Vec::new();
        let mostly_cross_chain = vec![
                Hit { chr: 0, loc: 10 },
                Hit {
                    chr: RRBS_BSC_FLAG,
                    loc: 20,
                },
                Hit {
                    chr: RRBS_BSC_FLAG,
                    loc: 30,
                },
                Hit {
                    chr: RRBS_BSC_FLAG,
                    loc: 40,
                },
                Hit {
                    chr: RRBS_BSC_FLAG,
                    loc: 50,
                },
            ];
        let normal_only = vec![
                Hit {
                    chr: 1 << 16,
                    loc: 60,
                },
                Hit {
                    chr: 1 << 16,
                    loc: 70,
                },
            ];
        let index = make_rrbs_count_index(vec![empty, mostly_cross_chain, normal_only]);
        let chain_seeds = vec![1, 0, 2];
        let mut profile = [[4u32; 16]; MAXSNPS as usize + 1];
        profile[0][0] = 0;
        profile[1][0] = 2;

        let se_segments = reorder_seeds_for_chain_with_cross_chain(
            &chain_seeds,
            &index,
            2,
            1,
            &profile,
            4,
            true,
            0,
            false,
        );
        assert_eq!(
            se_segments
                .iter()
                .map(|segment| (segment.index, segment.candidates))
                .collect::<Vec<_>>(),
            vec![(0, 1), (1, 2)]
        );

        let cross_chain_segments = reorder_seeds_for_chain_with_cross_chain(
            &chain_seeds,
            &index,
            2,
            1,
            &profile,
            4,
            true,
            0,
            true,
        );
        assert_eq!(
            cross_chain_segments
                .iter()
                .map(|segment| (segment.index, segment.candidates))
                .collect::<Vec<_>>(),
            vec![(1, 2), (0, 5)]
        );

        let mut scratch = Vec::new();
        reorder_seeds_for_chain_with_cross_chain_into(
            &chain_seeds,
            &index,
            2,
            1,
            &profile,
            4,
            true,
            0,
            true,
            &mut scratch,
        );
        let pointer = scratch.as_ptr();
        let expected: Vec<_> = scratch
            .iter()
            .map(|segment| (segment.index, segment.candidates))
            .collect();
        reorder_seeds_for_chain_with_cross_chain_into(
            &chain_seeds,
            &index,
            2,
            1,
            &profile,
            4,
            true,
            0,
            true,
            &mut scratch,
        );
        assert_eq!(scratch.as_ptr(), pointer);
        assert_eq!(
            scratch
                .iter()
                .map(|segment| (segment.index, segment.candidates))
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_cross_chain_setting_does_not_change_wgbs_count() {
        let index = KmerIndex {
            total_kmers: 2,
            max_kmer_num: u32::MAX,
            index2: vec![KmerLoc2 { n: [0, 0] }, KmerLoc2 { n: [2, 1] }],
            positions: vec![10, 20, 30],
            start_offsets: vec![0, 0],
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            wgbs_occupancy: Vec::new(),
            wgbs_rank: Vec::new(),
            wgbs_buckets: Vec::new(),
            wgbs_overflow: Vec::new(),
            seed_size: 2,
            mapped: None,
            rrbs_normal_counts: OnceLock::new(),
            rrbs_mode_ranges: OnceLock::new(),
        };

        let disabled =
            count_seeds_for_chain_with_cross_chain(&[1], &[0], &index, false, false);
        let enabled =
            count_seeds_for_chain_with_cross_chain(&[1], &[0], &index, false, true);
        assert_eq!(disabled, 3);
        assert_eq!(enabled, disabled);
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
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            wgbs_occupancy: Vec::new(),
            wgbs_rank: Vec::new(),
            wgbs_buckets: Vec::new(),
            wgbs_overflow: Vec::new(),
            seed_size: 16,
            mapped: None,
            rrbs_normal_counts: OnceLock::new(),
            rrbs_mode_ranges: OnceLock::new(),
        };

        let mut profile = [[0u32; 16]; MAXSNPS as usize + 1];
        for (segment, values) in profile.iter_mut().enumerate() {
            for (interval, value) in values.iter_mut().enumerate().take(4) {
                *value = (((segment as u32 * 8 + interval as u32) + 3) / 4) * 4;
            }
        }
        let best_offset = find_best_start_offset(&seeds, &index, 48, 8, 4, &profile, 5, false);
        assert!(best_offset < 5, "最佳偏移应该在有效范围内");
    }

    #[test]
    fn wgbs_count_seeds_keeps_filtered_bucket_frequency() {
        let index = KmerIndex {
            total_kmers: 1,
            max_kmer_num: 2,
            index2: vec![KmerLoc2 { n: [2, 2] }],
            positions: Vec::new(),
            start_offsets: vec![0],
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            wgbs_occupancy: Vec::new(),
            wgbs_rank: Vec::new(),
            wgbs_buckets: Vec::new(),
            wgbs_overflow: Vec::new(),
            seed_size: 2,
            mapped: None,
            rrbs_normal_counts: OnceLock::new(),
            rrbs_mode_ranges: OnceLock::new(),
        };

        assert_eq!(index.wgbs_candidate_count(0), 4);
        assert_eq!(index.lookup_separated(0), (&[][..], &[][..]));
        assert_eq!(count_seeds_for_chain(&[0], &[0], &index, false), 4);
    }
}
