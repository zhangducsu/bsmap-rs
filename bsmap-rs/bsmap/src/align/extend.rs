//! 种子扩展和命中收集模块。
//!
//! 对应 C++ align.cpp 中的 `SnpAlign()` 和 `AddHit()` 函数。
//! 实现种子扩展比对、命中去重和结果收集。
//!
//! ## 架构说明
//!
//! C++ BSMAP 采用**逐链独立**架构：
//! - `SnpAlign()` 对每条链（read_chain）独立执行
//! - segment 只包含该链的种子（无需 `seed_chains` 过滤）
//! - 最后合并两条链的 hits

use std::collections::HashSet;

use crate::align::gap::gap_align;
use crate::align::mismatch::count_mismatch;
use crate::align::seed::SeedSegment;
use crate::param::{GHit, MAXSNPS};
use crate::reads::encode::EncodedRead;
use crate::reference::binseq::BinSeqCollection;
use crate::reference::index::{KmerIndex, RRBS_CHR_MASK};
use crate::utils::myrand;

const HIT_LEVELS: usize = MAXSNPS as usize + 1;

fn circular_bucket_indices(
    bucket_len: usize,
    read_index: u32,
    randseed: u32,
) -> impl Iterator<Item = usize> {
    let start = if bucket_len == 0 {
        0
    } else {
        myrand(read_index, randseed, 0) as usize % bucket_len
    };
    (0..bucket_len).map(move |offset| (start + offset) % bucket_len)
}

/// C++ `AddHit()` 等价状态，由两条 read-chain 和全部 segment 共享。
pub(crate) struct HitCollector<'a> {
    hits: &'a mut [Vec<GHit>],
    level_counts: &'a mut [[usize; HIT_LEVELS]; 2],
    snp_thres: &'a mut u32,
    max_hits: usize,
    chr_lengths: &'a [u32],
    read_len: u32,
    dedup_no_gap: &'a mut HashSet<(u32, u32)>,
    dedup_gap: &'a mut HashSet<(u32, u32)>,
}

impl<'a> HitCollector<'a> {
    pub(crate) fn new(
        hits: &'a mut [Vec<GHit>],
        level_counts: &'a mut [[usize; HIT_LEVELS]; 2],
        snp_thres: &'a mut u32,
        max_hits: usize,
        chr_lengths: &'a [u32],
        read_len: u32,
        dedup_no_gap: &'a mut HashSet<(u32, u32)>,
        dedup_gap: &'a mut HashSet<(u32, u32)>,
    ) -> Self {
        Self {
            hits,
            level_counts,
            snp_thres,
            max_hits,
            chr_lengths,
            read_len,
            dedup_no_gap,
            dedup_gap,
        }
    }

    fn snp_thres(&self) -> u32 {
        *self.snp_thres
    }

    /// 按 C++ `AddHit()` 顺序接收一个已完成边界检查的命中。
    fn try_add_hit(&mut self, hit: GHit, read_chain: u8) -> bool {
        let snp_level = hit.snps as usize;
        let read_chain = read_chain as usize;
        if read_chain >= self.level_counts.len() || snp_level >= self.hits.len() {
            return false;
        }

        let Some(&chr_len) = self.chr_lengths.get(hit.chr as usize) else {
            return false;
        };
        let Some(hit_end) = hit.loc.checked_add(self.read_len) else {
            return false;
        };
        if hit_end > chr_len {
            return false;
        }

        let key = (hit.chr, hit.loc);
        let is_new = if hit.gap_size != 0 {
            self.dedup_gap.insert(key)
        } else {
            self.dedup_no_gap.insert(key)
        };
        if !is_new {
            return false;
        }

        self.hits[snp_level].push(hit);
        self.level_counts[read_chain][snp_level] += 1;

        let combined = self.level_counts[0][snp_level] + self.level_counts[1][snp_level];
        if combined >= self.max_hits {
            if snp_level == 0 {
                return true;
            }
            *self.snp_thres = snp_level as u32 - 1;
        }
        false
    }
}

/// 种子扩展比对（逐链独立）。
///
/// 对应 C++ `SnpAlign()` 函数。对单条链的所有 segment 进行比对。
pub fn snp_align_for_chain(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segments: &[SeedSegment],
    read_chain: u8,
    snp_thres: &mut u32,
    gap_size: u32,
    nt3: bool,
    _is_rrbs: bool,
    max_hits: usize,
    level_counts: &mut [usize],
) -> Vec<GHit> {
    let mut hits_by_level = vec![Vec::new(); HIT_LEVELS];
    let mut counts_by_chain = [[0usize; HIT_LEVELS]; 2];
    let copy_len = level_counts.len().min(HIT_LEVELS);
    counts_by_chain[read_chain as usize][..copy_len].copy_from_slice(&level_counts[..copy_len]);
    let mut dedup_no_gap = HashSet::new();
    let mut dedup_gap = HashSet::new();
    let read_len = encoded.info.seq.len() as u32;
    let query = if read_chain == 0 {
        &encoded.fwd_words
    } else {
        &encoded.rev_words
    };
    let mask = if read_chain == 0 {
        &encoded.fwd_mask
    } else {
        &encoded.rev_mask
    };
    let n_count = count_n_in_mask(mask, read_len);

    {
        let mut collector = HitCollector::new(
            &mut hits_by_level,
            &mut counts_by_chain,
            snp_thres,
            max_hits,
            &coll.chr_lengths,
            read_len,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );
        for segment in segments.iter() {
            if snp_align_segment(
                encoded,
                index,
                coll,
                segment,
                read_chain,
                gap_size,
                nt3,
                0,
                query,
                mask,
                n_count,
                &mut collector,
            ) {
                break;
            }
        }
    }

    level_counts[..copy_len].copy_from_slice(&counts_by_chain[read_chain as usize][..copy_len]);
    hits_by_level.into_iter().flatten().collect()
}

/// 对单个 segment 执行种子扩展比对。
///
/// 对应 C++ `SnpAlign()` 的单 segment 处理体。
/// 将命中追加到 `all_hits` 中。
///
/// 返回 `true` 表示应停止继续处理（对应 C++ AddHit 返回 1 —
/// MM=0 命中达到 max_hits 上限）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn snp_align_segment(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segment: &SeedSegment,
    read_chain: u8,
    gap_size: u32,
    nt3: bool,
    randseed: u32,
    query: &[u64],
    mask: &[u64],
    n_count: u32,
    collector: &mut HitCollector<'_>,
) -> bool {
    let read_len = encoded.info.seq.len() as u32;
    let fwd_slice = coll.refcat.as_slice();
    let rev_slice = coll.crefcat.as_slice();

    for (seed_idx, &seed_hash) in segment.seeds.iter().enumerate() {
        if seed_idx < segment.reg_masks.len() && segment.reg_masks[seed_idx] == 0 {
            continue;
        }

        let seed_pos_in_read = if seed_idx < segment.seed_positions.len() {
            segment.seed_positions[seed_idx]
        } else {
            segment.start_offset + seed_idx as u32 * 4
        };

        // ── RRBS mode: positions from rrbs_index ──
        if let Some(ref rrbs_idx) = index.rrbs_index {
            if (seed_hash as usize) < rrbs_idx.len() && rrbs_idx[seed_hash as usize].n1 > 0 {
                let modeindex = segment.index as u32;
                let max_mode = read_len / index.seed_size;
                if max_mode == 0 || modeindex >= max_mode {
                    continue;
                }
                let cmodeindex = if read_chain == 0 {
                    modeindex
                } else {
                    max_mode - 1 - modeindex
                };
                let read_chain_mask = (read_chain as u32) << 24;
                let hits = &rrbs_idx[seed_hash as usize].loc1;

                for bucket_idx in
                    circular_bucket_indices(hits.len(), encoded.info.index, randseed)
                {
                    let hit = &hits[bucket_idx];
                    if ((hit.chr ^ read_chain_mask) >> 16) != cmodeindex {
                        continue;
                    }

                    let block_id = hit.chr & RRBS_CHR_MASK;
                    let ref_chain = (block_id & 1) as u8;
                    let chr_idx = (block_id / 2) as usize;
                    let anchor = if chr_idx < coll.ref_anchor.len() {
                        coll.ref_anchor[chr_idx]
                    } else {
                        continue;
                    };
                    let rc_offset = if chr_idx + 1 < coll.ref_anchor.len() {
                        coll.ref_anchor[chr_idx + 1] - coll.ref_anchor[chr_idx]
                    } else {
                        continue;
                    };

                    let Some(local_start) = hit.loc.checked_sub(seed_pos_in_read) else {
                        continue;
                    };

                    let Some(alignment_start) = anchor.checked_add(local_start) else {
                        continue;
                    };
                    let ref_offset = alignment_start as u64 * 2;
                    let ref_seq = if ref_chain == 0 { fwd_slice } else { rev_slice };
                    if (ref_offset as u64 / 64) as usize + query.len() > ref_seq.len() {
                        continue;
                    }

                    let strand = (ref_chain << 1) | read_chain;
                    let loc = if ref_chain == 1 {
                        let Some(loc) = rc_offset
                            .checked_sub(read_len)
                            .and_then(|end| end.checked_sub(local_start))
                        else {
                            continue;
                        };
                        loc
                    } else {
                        local_start
                    };
                    let chr = chr_idx as u32;

                    let snp_thres = collector.snp_thres();
                    let mm_count =
                        count_mismatch(query, ref_offset, ref_seq, mask, snp_thres, n_count, nt3);

                    if mm_count <= snp_thres {
                        let hit = GHit {
                            chr,
                            loc,
                            snps: mm_count as u8,
                            strand,
                            gap_size: 0,
                            gap_pos: 0,
                        };
                        if collector.try_add_hit(hit, read_chain) {
                            return true;
                        }
                    }

                    let snp_thres = collector.snp_thres();
                    if gap_size > 0 && mm_count > snp_thres && mm_count <= snp_thres + 2 {
                        if let Some(gap_result) = gap_align(
                            query,
                            ref_seq,
                            alignment_start,
                            seed_pos_in_read,
                            8,
                            snp_thres,
                            gap_size,
                            nt3,
                            read_len,
                            3,
                        ) {
                            let hit = GHit {
                                chr,
                                loc,
                                snps: gap_result.snp_count as u8,
                                strand,
                                gap_size: gap_result.gap_size as i16,
                                gap_pos: gap_result.gap_pos as u16,
                            };
                            if collector.try_add_hit(hit, read_chain) {
                                return true;
                            }
                        }
                    }
                }
            }
        } else {
            // ── WGBS mode: existing lookup_separated logic ──
            let (fwd_positions, rev_positions) = index.lookup_separated(seed_hash);

            let bucket_len = fwd_positions.len() + rev_positions.len();
            for bucket_idx in
                circular_bucket_indices(bucket_len, encoded.info.index, randseed)
            {
                let (ref_chain, flat_pos) = if bucket_idx < fwd_positions.len() {
                    (0u8, fwd_positions[bucket_idx])
                } else {
                    (1u8, rev_positions[bucket_idx - fwd_positions.len()])
                };
                let ref_seq = if ref_chain == 0 { fwd_slice } else { rev_slice };
                let strand = (ref_chain << 1) | read_chain;

                let Some(alignment_start) = flat_pos.checked_sub(seed_pos_in_read) else {
                    continue;
                };
                let ref_offset = alignment_start as u64 * 2;
                let (chr, mut loc) = coll.int2hit(alignment_start);

                if ref_chain == 1 {
                    // C++ int2hit() flips against title[chr].rc_offset, which includes padding.
                    // AddHit() then validates the converted location against the true length.
                    let rc_offset = coll.total_len_for_chr(chr as usize);
                    let Some(reverse_loc) = rc_offset
                        .checked_sub(read_len)
                        .and_then(|end| end.checked_sub(loc))
                    else {
                        continue;
                    };
                    loc = reverse_loc;
                }

                if (ref_offset as u64 / 64) as usize + query.len() > ref_seq.len() {
                    continue;
                }

                let snp_thres = collector.snp_thres();
                let mm_count =
                    count_mismatch(query, ref_offset, ref_seq, mask, snp_thres, n_count, nt3);

                if mm_count <= snp_thres {
                    let hit = GHit {
                        chr,
                        loc,
                        snps: mm_count as u8,
                        strand,
                        gap_size: 0,
                        gap_pos: 0,
                    };
                    if collector.try_add_hit(hit, read_chain) {
                        return true;
                    }
                }

                let snp_thres = collector.snp_thres();
                if gap_size > 0 && mm_count > snp_thres && mm_count <= snp_thres + 2 {
                    if let Some(gap_result) = gap_align(
                        query,
                        ref_seq,
                        alignment_start,
                        seed_pos_in_read,
                        8,
                        snp_thres,
                        gap_size,
                        nt3,
                        read_len,
                        3,
                    ) {
                        let hit = GHit {
                            chr,
                            loc,
                            snps: gap_result.snp_count as u8,
                            strand,
                            gap_size: gap_result.gap_size as i16,
                            gap_pos: gap_result.gap_pos as u16,
                        };
                        if collector.try_add_hit(hit, read_chain) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// 命中去重。
pub fn dedup_hits(hits: &mut Vec<GHit>) {
    if hits.len() <= 1 {
        return;
    }

    hits.sort_unstable_by(|a, b| {
        a.chr
            .cmp(&b.chr)
            .then_with(|| a.loc.cmp(&b.loc))
            .then_with(|| a.strand.cmp(&b.strand))
            .then_with(|| a.snps.cmp(&b.snps))
    });

    hits.dedup_by(|a, b| a.chr == b.chr && a.loc == b.loc && a.strand == b.strand);
}

/// 计算掩码中 N 碱基的数量（仅统计 read 长度范围内）。
pub fn count_n_in_mask(mask: &[u64], read_len: u32) -> u32 {
    let mut count = 0u32;
    let total_bits = read_len * 2;
    let mut bits_processed = 0u32;
    for &word in mask {
        if bits_processed >= total_bits {
            break;
        }
        let inverted = !word;
        let remaining = ((total_bits - bits_processed) / 2).min(32);
        for i in 0..remaining {
            let bits = (inverted >> (62 - i * 2)) & 0b11;
            if bits == 0b11 {
                count += 1;
            }
        }
        bits_processed += 64;
    }
    count
}

/// 清空命中列表。
pub fn clear_hits(hits: &mut [Vec<GHit>]) {
    for level in hits.iter_mut() {
        level.clear();
    }
}

/// 统计唯一命中数。
pub fn count_unique_hits(hits: &[Vec<GHit>]) -> usize {
    use std::collections::HashSet;

    let mut unique = HashSet::new();
    for level in hits.iter() {
        for hit in level.iter() {
            unique.insert((hit.chr, hit.loc, hit.strand));
        }
    }
    unique.len()
}

/// 检查是否有唯一比对（对应 C++：仅看最佳 mismatch 层的命中数）。
///
/// C++ 行为：找到第一个非空层（snp 最少），检查该层命中数是否为 1。
/// 这比跨所有层去重更宽松——如果最佳层只有 1 个命中，即使有其他命中
/// 在更高的 mismatch 层，也算作 unique。
pub fn is_unique_hit(hits: &[Vec<GHit>]) -> bool {
    for level in hits.iter() {
        if !level.is_empty() {
            return level.len() == 1;
        }
    }
    false
}

/// 选择最佳命中。
///
/// # 返回值
/// (最佳命中列表, 最佳 mismatch 数)
pub fn select_best_hits(hits: &[Vec<GHit>]) -> (Vec<GHit>, u8) {
    let mut result = Vec::new();
    let mut best_snp = 0u8;

    // 优先选择 mismatch 数少的
    for (snp_level, level) in hits.iter().enumerate() {
        if !level.is_empty() {
            // 去重
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            for hit in level.iter() {
                let key = (hit.chr, hit.loc, hit.strand);
                if seen.insert(key) {
                    result.push(*hit);
                }
            }
            if !result.is_empty() {
                best_snp = snp_level as u8;
                break;
            }
        }
    }

    (result, best_snp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(loc: u32, snps: u8) -> GHit {
        GHit {
            chr: 0,
            loc,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps,
        }
    }

    #[test]
    fn bucket_uses_nonzero_start_and_visits_every_entry_once() {
        let order: Vec<usize> = circular_bucket_indices(7, 100, 42).collect();
        assert_eq!(order, vec![1, 2, 3, 4, 5, 6, 0]);

        let mut sorted = order;
        sorted.sort_unstable();
        assert_eq!(sorted, (0..7).collect::<Vec<_>>());
    }

    #[test]
    fn true_chromosome_length_rejects_padding_but_accepts_exact_end() {
        let mut hits = vec![Vec::new(); HIT_LEVELS];
        let mut counts = [[0usize; HIT_LEVELS]; 2];
        let mut snp_thres = 0;
        let chr_lengths = [100];
        let mut dedup_no_gap = HashSet::new();
        let mut dedup_gap = HashSet::new();
        let mut collector = HitCollector::new(
            &mut hits,
            &mut counts,
            &mut snp_thres,
            10,
            &chr_lengths,
            10,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );

        assert!(!collector.try_add_hit(hit(91, 0), 0));
        assert!(!collector.try_add_hit(hit(90, 0), 0));
        drop(collector);

        assert_eq!(hits[0].len(), 1);
        assert_eq!(hits[0][0].loc, 90);
        assert_eq!(counts[0][0], 1);
        assert_eq!(dedup_no_gap.len(), 1);
    }

    #[test]
    fn bsw_bsc_and_both_read_chains_keep_strand_encoding() {
        let mut hits = vec![Vec::new(); HIT_LEVELS];
        let mut counts = [[0usize; HIT_LEVELS]; 2];
        let mut snp_thres = 0;
        let chr_lengths = [1_000];
        let mut dedup_no_gap = HashSet::new();
        let mut dedup_gap = HashSet::new();
        let mut collector = HitCollector::new(
            &mut hits,
            &mut counts,
            &mut snp_thres,
            10,
            &chr_lengths,
            10,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );

        for (loc, read_chain, ref_chain) in
            [(100, 0u8, 0u8), (200, 0, 1), (300, 1, 0), (400, 1, 1)]
        {
            let mut candidate = hit(loc, 0);
            candidate.strand = (ref_chain << 1) | read_chain;
            assert!(!collector.try_add_hit(candidate, read_chain));
        }
        drop(collector);

        assert_eq!(counts[0][0], 2);
        assert_eq!(counts[1][0], 2);
        assert_eq!(
            hits[0].iter().map(|candidate| candidate.strand).collect::<Vec<_>>(),
            vec![0, 2, 1, 3],
        );
    }

    #[test]
    fn duplicate_hit_is_not_counted_twice() {
        let mut hits = vec![Vec::new(); HIT_LEVELS];
        let mut counts = [[0usize; HIT_LEVELS]; 2];
        let mut snp_thres = 2;
        let chr_lengths = [1_000];
        let mut dedup_no_gap = HashSet::new();
        let mut dedup_gap = HashSet::new();
        let mut collector = HitCollector::new(
            &mut hits,
            &mut counts,
            &mut snp_thres,
            10,
            &chr_lengths,
            10,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );

        assert!(!collector.try_add_hit(hit(100, 1), 0));
        assert!(!collector.try_add_hit(hit(100, 1), 1));
        drop(collector);

        assert_eq!(hits[1].len(), 1);
        assert_eq!(counts[0][1], 1);
        assert_eq!(counts[1][1], 0);
    }

    #[test]
    fn accepted_hit_counts_accumulate_across_segment_calls() {
        let mut hits = vec![Vec::new(); HIT_LEVELS];
        let mut counts = [[0usize; HIT_LEVELS]; 2];
        let mut snp_thres = 2;
        let chr_lengths = [1_000];
        let mut dedup_no_gap = HashSet::new();
        let mut dedup_gap = HashSet::new();
        let mut collector = HitCollector::new(
            &mut hits,
            &mut counts,
            &mut snp_thres,
            10,
            &chr_lengths,
            10,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );

        assert!(!collector.try_add_hit(hit(100, 1), 0));
        assert!(!collector.try_add_hit(hit(200, 1), 0));
        drop(collector);

        assert_eq!(counts[0][1], 2);
        assert_eq!(hits[1].len(), 2);
    }

    #[test]
    fn read_chains_share_threshold_and_level_zero_early_stop() {
        let mut hits = vec![Vec::new(); HIT_LEVELS];
        let mut counts = [[0usize; HIT_LEVELS]; 2];
        let mut snp_thres = 2;
        let chr_lengths = [1_000];
        let mut dedup_no_gap = HashSet::new();
        let mut dedup_gap = HashSet::new();
        let mut collector = HitCollector::new(
            &mut hits,
            &mut counts,
            &mut snp_thres,
            2,
            &chr_lengths,
            10,
            &mut dedup_no_gap,
            &mut dedup_gap,
        );

        assert!(!collector.try_add_hit(hit(100, 2), 0));
        assert!(!collector.try_add_hit(hit(200, 2), 1));
        assert_eq!(collector.snp_thres(), 1);
        assert!(!collector.try_add_hit(hit(300, 0), 0));
        assert!(collector.try_add_hit(hit(400, 0), 1));
        drop(collector);

        assert_eq!(counts[0][2] + counts[1][2], 2);
        assert_eq!(counts[0][0] + counts[1][0], 2);
        assert_eq!(snp_thres, 1);
    }
}
