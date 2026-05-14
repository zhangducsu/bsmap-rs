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

use crate::align::gap::gap_align;
use crate::align::mismatch::count_mismatch;
use crate::align::seed::SeedSegment;
use crate::param::GHit;
use crate::reads::encode::EncodedRead;
use crate::reference::binseq::BinSeqCollection;
use crate::reference::index::KmerIndex;

/// 扩展后的命中记录（内部使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ExtHit {
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
    fn to_ghit(self) -> GHit {
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
///
/// # 参数
/// - `encoded`: 编码后的读段
/// - `index`: k-mer 索引
/// - `coll`: 二进制参考序列集合
/// - `segments`: 该链的所有 seed segments（已排序）
/// - `read_chain`: 读段链（0=正向, 1=反向）
/// - `snp_thres`: mismatch 阈值
/// - `gap_size`: 最大 gap 大小
/// - `nt3`: 3-核苷酸模式
/// - `is_rrbs`: 是否为 RRBS 模式
///
/// # 返回值
/// 命中列表（GHit 数组）
pub fn snp_align_for_chain(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segments: &[SeedSegment],
    read_chain: u8,
    snp_thres: u32,
    gap_size: u32,
    nt3: bool,
    _is_rrbs: bool,
) -> Vec<GHit> {
    let mut all_hits: Vec<ExtHit> = Vec::new();
    let read_len = encoded.info.seq.len() as u32;

    // 获取该链的查询序列和掩码
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

    // 处理每个 segment
    for (seg_idx, segment) in segments.iter().enumerate() {

        // 如果已经找到足够好的命中，提前终止
        if should_stop_early(seg_idx, &all_hits, snp_thres) {
            break;
        }

        // 对该 segment 的每个种子进行比对
        for (seed_idx, &seed_hash) in segment.seeds.iter().enumerate() {
            if seed_idx < segment.reg_masks.len() && segment.reg_masks[seed_idx] == 0 {
                continue;
            }

            // 获取种子在读段中的位置
            let seed_pos_in_read = if seed_idx < segment.seed_positions.len() {
                segment.seed_positions[seed_idx]
            } else {
                // 回退计算
                segment.start_offset + seed_idx as u32 * 4
            };

            // 查找种子在参考中的位置
            let (fwd_positions, rev_positions) = index.lookup_separated(seed_hash);

            // 对每条参考链进行比对
            for ref_chain in 0..2u8 {
                let positions = if ref_chain == 0 { fwd_positions } else { rev_positions };
                let ref_seq = if ref_chain == 0 { &coll.refcat } else { &coll.crefcat };

                if positions.is_empty() {
                    continue;
                }

                let strand = (ref_chain << 1) | read_chain;

                for &flat_pos in positions {
                    let alignment_start = flat_pos.wrapping_sub(seed_pos_in_read);
                    let ref_offset = alignment_start * 2;
                    let (chr, loc) = coll.int2hit(alignment_start);

                    if (ref_offset / 64) as usize + query.len() > ref_seq.len() {
                        continue;
                    }

                    let mm_count = count_mismatch(
                        query,
                        ref_offset,
                        ref_seq,
                        mask,
                        snp_thres,
                        n_count,
                        nt3,
                    );

                    if mm_count <= snp_thres {
                        all_hits.push(ExtHit {
                            chr: chr / 2,
                            loc,
                            snps: mm_count as u8,
                            strand,
                            gap_size: 0,
                            gap_pos: 0,
                        });
                    }

                    // Gap 检测
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
                            all_hits.push(ExtHit {
                                chr: chr / 2,
                                loc,
                                snps: gap_result.snp_count as u8,
                                strand,
                                gap_size: gap_result.gap_size,
                                gap_pos: gap_result.gap_pos,
                            });
                        }
                    }
                }
            }
        }
    }

    // 去重
    dedup_hits(&mut all_hits);

    // 转换为 GHit
    all_hits.into_iter().map(|h| h.to_ghit()).collect()
}

/// 判断是否应提前终止。
fn should_stop_early(seg_idx: usize, hits: &[ExtHit], _snp_thres: u32) -> bool {
    // C++ 逻辑：如果已经找到唯一比对，提前终止
    // 简化：如果已经有 hit 且处理了几个 segment，就终止
    if seg_idx > 0 && !hits.is_empty() {
        // 进一步检查是否有唯一比对
        // 这里简化处理
        false
    } else {
        false
    }
}

/// 命中去重。
fn dedup_hits(hits: &mut Vec<ExtHit>) {
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
fn count_n_in_mask(mask: &[u64], read_len: u32) -> u32 {
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
pub fn add_hits(new_hits: Vec<GHit>, all_hits: &mut [Vec<GHit>], max_hits: usize) -> bool {
    for hit in new_hits {
        let snp_level = hit.snps as usize;
        if snp_level < all_hits.len() {
            all_hits[snp_level].push(hit);
        }
    }

    // 检查是否达到最大命中数限制
    let total: usize = all_hits.iter().map(|v| v.len()).sum();
    total >= max_hits
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

/// 检查是否有唯一比对。
pub fn is_unique_hit(hits: &[Vec<GHit>]) -> bool {
    let total = count_unique_hits(hits);
    total == 1
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
