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
use crate::param::GHit;
use crate::reads::encode::EncodedRead;
use crate::reference::binseq::BinSeqCollection;
use crate::reference::index::KmerIndex;

/// 扩展后的命中记录（内部使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExtHit {
    /// 染色体 ID。
    chr: u32,
    /// 位置。
    loc: u32,
    /// Mismatch 数。
    snps: u8,
    /// Strand 编码 (ref_chain << 1 | read_chain)。
    strand: u8,
    /// Gap 大小。
    gap_size: i8,
    /// Gap 位置。
    gap_pos: u8,
}

impl ExtHit {
    /// 转换为 GHit。
    pub fn to_ghit(self) -> GHit {
        GHit {
            loc: self.loc,
            chr: self.chr,
            strand: self.strand,
            gap_size: self.gap_size as i16,
            gap_pos: self.gap_pos as u16,
            snps: self.snps,
        }
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
    let mut all_hits: Vec<ExtHit> = Vec::new();
    let read_len = encoded.info.seq.len() as u32;
    let query = if read_chain == 0 { &encoded.fwd_words } else { &encoded.rev_words };
    let mask = if read_chain == 0 { &encoded.fwd_mask } else { &encoded.rev_mask };
    let n_count = count_n_in_mask(mask, read_len);

    for segment in segments.iter() {
        if snp_align_segment(
            encoded, index, coll, segment, read_chain, snp_thres,
            gap_size, nt3, max_hits, level_counts,
            query, mask, n_count, &mut all_hits,
        ) {
            break;
        }
    }

    dedup_hits(&mut all_hits);
    all_hits.into_iter().map(|h| h.to_ghit()).collect()
}

/// 对单个 segment 执行种子扩展比对。
///
/// 对应 C++ `SnpAlign()` 的单 segment 处理体。
/// 将命中追加到 `all_hits` 中。
///
/// 返回 `true` 表示应停止继续处理（对应 C++ AddHit 返回 1 —
/// MM=0 命中达到 max_hits 上限）。
#[allow(clippy::too_many_arguments)]
pub fn snp_align_segment(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segment: &SeedSegment,
    read_chain: u8,
    snp_thres: &mut u32,
    gap_size: u32,
    nt3: bool,
    max_hits: usize,
    level_counts: &mut [usize],
    query: &[u64],
    mask: &[u64],
    n_count: u32,
    all_hits: &mut Vec<ExtHit>,
) -> bool {
    let read_len = encoded.info.seq.len() as u32;

    for (seed_idx, &seed_hash) in segment.seeds.iter().enumerate() {
        if seed_idx < segment.reg_masks.len() && segment.reg_masks[seed_idx] == 0 {
            continue;
        }

        let seed_pos_in_read = if seed_idx < segment.seed_positions.len() {
            segment.seed_positions[seed_idx]
        } else {
            segment.start_offset + seed_idx as u32 * 4
        };

        let (fwd_positions, rev_positions) = index.lookup_separated(seed_hash);

        for ref_chain in 0..2u8 {
            let positions = if ref_chain == 0 { fwd_positions } else { rev_positions };
            let ref_seq = if ref_chain == 0 { coll.refcat.as_slice() } else { coll.crefcat.as_slice() };

            if positions.is_empty() {
                continue;
            }

            let strand = (ref_chain << 1) | read_chain;

            for &flat_pos in positions {
                let alignment_start = flat_pos.wrapping_sub(seed_pos_in_read);
                let ref_offset = alignment_start * 2;
                let (chr, mut loc) = coll.int2hit(alignment_start);

                // C++ int2hit 坐标转换：反向链位置转为正向坐标
                // 对应 C++: if(ref_chain_index) gh.loc = rc_offset - map_readlen - gh.loc
                // 使用 get_chromosome_length（真实长度），而非 total_len_for_chr（含 BINSEQPAD padding）
                if ref_chain == 1 {
                    let chr_len = crate::align::output::get_chromosome_length(chr, coll);
                    loc = chr_len.saturating_sub(read_len).saturating_sub(loc);
                }

                if (ref_offset / 64) as usize + query.len() > ref_seq.len() {
                    continue;
                }

                let mm_count = count_mismatch(
                    query, ref_offset, ref_seq, mask,
                    *snp_thres, n_count, nt3,
                );

                if mm_count <= *snp_thres {
                    let snp_level = mm_count as usize;
                    level_counts[snp_level] += 1;
                    all_hits.push(ExtHit {
                        chr: chr / 2,
                        loc,
                        snps: mm_count as u8,
                        strand,
                        gap_size: 0,
                        gap_pos: 0,
                    });

                    // C++ AddHit: if w==0 and max_hits reached, return 1 (stop immediately)
                    // Without this, Rust collects all hits before checking stop, producing extra hits
                    if mm_count == 0 && level_counts[0] >= max_hits {
                        return true;
                    }
                    if mm_count > 0 && level_counts[snp_level] >= max_hits {
                        *snp_thres = mm_count - 1;
                    }
                }

                if gap_size > 0 && mm_count > *snp_thres && mm_count <= *snp_thres + 2 {
                    if let Some(gap_result) = gap_align(
                        query, ref_seq, alignment_start, seed_pos_in_read,
                        8, *snp_thres, gap_size, nt3, read_len, 3,
                    ) {
                        let gap_snps = gap_result.snp_count as usize;
                        level_counts[gap_snps] += 1;
                        all_hits.push(ExtHit {
                            chr: chr / 2,
                            loc,
                            snps: gap_result.snp_count as u8,
                            strand,
                            gap_size: gap_result.gap_size,
                            gap_pos: gap_result.gap_pos,
                        });
                        // C++: GapAlign returns AddHit result, which stops if w==0 and max_hits reached
                        if gap_snps == 0 && level_counts[0] >= max_hits {
                            return true;
                        }
                        if gap_snps > 0 && level_counts[gap_snps] >= max_hits {
                            *snp_thres = gap_snps as u32 - 1;
                        }
                    }
                }
            }
        }
    }
    false
}

/// 命中去重。
pub fn dedup_hits(hits: &mut Vec<ExtHit>) {
    if hits.len() <= 1 {
        return;
    }

    hits.sort_unstable_by(|a, b| {
        a.chr.cmp(&b.chr)
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

/// 添加命中到列表。
///
/// 对应 C++ `AddHit()` 函数。将 segment 的命中添加到总命中列表。
///
/// # 返回值
/// 如果达到最大命中数限制，返回 true（应停止）
pub fn add_hits(new_hits: Vec<GHit>, all_hits: &mut [Vec<GHit>], max_hits: usize, dedup_no_gap: &mut HashSet<(u32, u32)>, dedup_gap: &mut HashSet<(u32, u32)>) -> bool {
    for hit in new_hits {
        let snp_level = hit.snps as usize;
        if snp_level >= all_hits.len() {
            continue;
        }

        // C++ 去重：同一 (chr>>1, loc) 位置只保留首次出现的命中
        // 对应 C++ if(!hitset[_ghit.chr>>1].insert(_ghit.loc).second) return 0
        let key = (hit.chr >> 1, hit.loc);
        if hit.gap_size != 0 {
            if !dedup_gap.insert(key) {
                continue;
            }
        } else {
            if !dedup_no_gap.insert(key) {
                continue;
            }
        }

        all_hits[snp_level].push(hit);
    }

    // C++ 行为：仅当最佳 mismatch 层 (w=0) 的命中数达到上限时才停止
    if let Some(level0) = all_hits.first() {
        if level0.len() >= max_hits {
            return true;
        }
    }
    false
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
