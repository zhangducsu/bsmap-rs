//! 种子扩展和命中收集模块。
//!
//! 对应 C++ align.cpp 中的 `SnpAlign()` 和 `AddHit()` 函数。
//! 实现种子扩展比对、命中去重和结果收集。
//!
//! ## 核心功能
//!
//! 1. **种子扩展比对**: 从种子位置向两端扩展，验证完整比对
//! 2. **Gap 检测**: 在扩展过程中检测可能的 gap
//! 3. **命中去重**: 使用 Vec + sort_unstable + dedup 优化去重
//! 4. **结果收集**: 将命中转换为 GHit 结构

use crate::align::gap::{gap_align, GapResult};
use crate::align::mismatch::count_mismatch;
use crate::align::seed::SeedSegment;
use crate::align::Chain;
use crate::param::{GHit, Hit, MAXHITS, MAXSNPS};
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
    /// 链（0=正向, 1=反向互补）。
    chain: u8,
    /// Gap 大小。
    gap_size: i8,
    /// Gap 位置。
    gap_pos: u8,
}

impl ExtHit {
    /// 转换为 GHit。
    fn to_ghit(self, strand: u8) -> GHit {
        GHit {
            loc: self.loc,
            chr: self.chr,
            strand,
            gap_size: self.gap_size as i16,
            gap_pos: self.gap_pos as u16,
            snps: self.snps,
        }
    }
}

/// 种子扩展比对。
///
/// 对应 C++ `SnpAlign()` 函数。对每个候选位置进行完整比对，
/// 包括 mismatch 计数和 gap 检测。
///
/// # 参数
/// - `encoded`: 编码后的读段
/// - `index`: k-mer 索引
/// - `coll`: 二进制参考序列集合
/// - `segment`: 当前处理的 seed segment
/// - `mode`: mismatch 级别（0, 1, 2, ...）
/// - `snp_thres`: mismatch 阈值
/// - `gap_size`: 最大 gap 大小
/// - `nt3`: 3-核苷酸模式
/// - `is_rrbs`: 是否为 RRBS 模式
/// - `profile`: 参数 profile 矩阵
///
/// # 返回值
/// 命中列表（GHit 数组）
pub fn snp_align(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segment: &SeedSegment,
    mode: usize,
    snp_thres: u32,
    gap_size: u32,
    nt3: bool,
    is_rrbs: bool,
    profile: &[[u32; 16]],
) -> Vec<GHit> {
    let mut hits: Vec<ExtHit> = Vec::new();
    let read_len = encoded.info.seq.len() as u32;

    // 获取该 segment 的 mismatch 阈值
    let seg_snp_thres = if mode < profile.len() {
        mode as u32
    } else {
        snp_thres
    };

    if is_rrbs {
        // RRBS 模式：保持原有的 chain 循环逻辑
        for chain in 0..2u8 {
            let query = if chain == 0 {
                &encoded.fwd_words
            } else {
                &encoded.rev_words
            };
            let mask = if chain == 0 {
                &encoded.fwd_mask
            } else {
                &encoded.rev_mask
            };

            let n_count = count_n_in_mask(mask);
            let ref_seq = if chain == 0 {
                &coll.refcat
            } else {
                &coll.crefcat
            };

            for (seed_idx, &seed_hash) in segment.seeds.iter().enumerate() {
                if seed_idx < segment.reg_masks.len() && segment.reg_masks[seed_idx] == 0 {
                    continue;
                }

                let positions = if let Some(ref rrbs_idx) = index.rrbs_index {
                    if (seed_hash as usize) < rrbs_idx.len() {
                        rrbs_idx[seed_hash as usize]
                            .loc1
                            .iter()
                            .map(|h| coll.hit2int(h.chr, h.loc))
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                for &flat_pos in &positions {
                    let (chr, loc) = coll.int2hit(flat_pos);
                    let ref_offset = flat_pos * 2;

                    if (ref_offset / 64) as usize + query.len() > ref_seq.len() {
                        continue;
                    }

                    let mm_count = count_mismatch(
                        query,
                        ref_offset,
                        ref_seq,
                        mask,
                        seg_snp_thres,
                        n_count,
                        nt3,
                    );

                    if mm_count <= seg_snp_thres {
                        hits.push(ExtHit {
                            chr: chr / 2,
                            loc,
                            snps: mm_count as u8,
                            chain,
                            gap_size: 0,
                            gap_pos: 0,
                        });
                    }

                    if gap_size > 0 && mm_count > seg_snp_thres && mm_count <= seg_snp_thres + 2 {
                        if let Some(gap_result) = gap_align(
                            query,
                            ref_seq,
                            flat_pos,
                            segment.start_offset,
                            8,
                            seg_snp_thres,
                            gap_size,
                            nt3,
                            read_len,
                            3,
                        ) {
                            hits.push(ExtHit {
                                chr: chr / 2,
                                loc,
                                snps: gap_result.snp_count as u8,
                                chain,
                                gap_size: gap_result.gap_size,
                                gap_pos: gap_result.gap_pos,
                            });
                        }
                    }
                }
            }
        }
    } else {
        // WGBS 模式：使用 lookup_separated 获取分离的正反链位置
        // 每个种子带有 seed_chains 标记其来源链，
        // 正向读段种子(chain=0) → 查正向参考位置(refcat)
        // 反向读段种子(chain=1) → 查反向参考位置(crefcat)

        for read_chain in 0..2u8 {
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
            let ref_seq = if read_chain == 0 {
                &coll.refcat
            } else {
                &coll.crefcat
            };
            let n_count = count_n_in_mask(mask);

            let mut total_positions_checked = 0usize;
            let mut total_mm_ok = 0usize;
            let mut candidates_with_pos = 0usize;

            for (seed_idx, &seed_hash) in segment.seeds.iter().enumerate() {
                // 只处理属于当前 read_chain 的种子
                if seed_idx < segment.seed_chains.len() && segment.seed_chains[seed_idx] != read_chain {
                    continue;
                }

                if seed_idx < segment.reg_masks.len() && segment.reg_masks[seed_idx] == 0 {
                    continue;
                }

                let (fwd_positions, rev_positions) = index.lookup_separated(seed_hash);

                // 根据读段链选择对应参考链的位置
                let positions: &[u32] = if read_chain == 0 {
                    fwd_positions // 正向读段 → 正向参考位置
                } else {
                    rev_positions // 反向读段 → 反向参考位置
                };

                if positions.is_empty() {
                    continue;
                }
                candidates_with_pos += 1;
                total_positions_checked += positions.len();

                for &flat_pos in positions {
                    let (chr, loc) = coll.int2hit(flat_pos);
                    let ref_offset = flat_pos * 2;

                    if (ref_offset / 64) as usize + query.len() > ref_seq.len() {
                        continue;
                    }

                    let mm_count = count_mismatch(
                        query,
                        ref_offset,
                        ref_seq,
                        mask,
                        seg_snp_thres,
                        n_count,
                        nt3,
                    );

                    if mm_count <= seg_snp_thres {
                        total_mm_ok += 1;
                        hits.push(ExtHit {
                            chr: chr / 2,
                            loc,
                            snps: mm_count as u8,
                            chain: read_chain,
                            gap_size: 0,
                            gap_pos: 0,
                        });
                    }

                    if gap_size > 0 && mm_count > seg_snp_thres && mm_count <= seg_snp_thres + 2 {
                        if let Some(gap_result) = gap_align(
                            query,
                            ref_seq,
                            flat_pos,
                            segment.start_offset,
                            8,
                            seg_snp_thres,
                            gap_size,
                            nt3,
                            read_len,
                            3,
                        ) {
                            hits.push(ExtHit {
                                chr: chr / 2,
                                loc,
                                snps: gap_result.snp_count as u8,
                                chain: read_chain,
                                gap_size: gap_result.gap_size,
                                gap_pos: gap_result.gap_pos,
                            });
                        }
                    }
                }
            }

            // 调试日志（只在第一个 segment 输出）
            if segment.index == 0 {
                log::info!(
                    "WGBS read_chain={}: {} candidates with positions, {} total positions checked, {} passed mismatch check",
                    read_chain, candidates_with_pos, total_positions_checked, total_mm_ok
                );
            }
        }
    }

    // 去重
    dedup_hits(&mut hits);

    // 转换为 GHit
    hits.into_iter()
        .map(|h| {
            let strand = calculate_strand(h.chain, 0); // 简化处理
            h.to_ghit(strand)
        })
        .collect()
}

/// 计算 strand 编码。
///
/// strand 编码：`ref_chain << 1 | read_chain`
fn calculate_strand(read_chain: u8, ref_chain: u8) -> u8 {
    (ref_chain << 1) | read_chain
}

/// 命中去重。
///
/// 使用 Vec + sort_unstable + dedup 替代 HashSet，内存效率更高。
/// 去重键：(chr, loc, chain, gap_size, gap_pos)
fn dedup_hits(hits: &mut Vec<ExtHit>) {
    // 按去重键排序
    hits.sort_unstable_by(|a, b| {
        a.chr
            .cmp(&b.chr)
            .then_with(|| a.loc.cmp(&b.loc))
            .then_with(|| a.chain.cmp(&b.chain))
            .then_with(|| a.gap_size.cmp(&b.gap_size))
            .then_with(|| a.gap_pos.cmp(&b.gap_pos))
    });

    // 去重
    hits.dedup_by(|a, b| {
        a.chr == b.chr
            && a.loc == b.loc
            && a.chain == b.chain
            && a.gap_size == b.gap_size
            && a.gap_pos == b.gap_pos
    });
}

/// 添加命中（带容量检查）。
///
/// 对应 C++ `AddHit()` 函数。将新命中添加到列表，
/// 如果达到最大容量返回 true 表示需要提前终止。
///
/// # 参数
/// - `new_hits`: 新命中的 GHit 数组
/// - `hits`: 现有命中列表（按 snp_level 组织）
/// - `max_hits`: 最大命中数
///
/// # 返回值
/// 如果达到 max_hits 返回 true
pub fn add_hits(
    new_hits: Vec<GHit>,
    hits: &mut Vec<Vec<GHit>>,
    max_hits: usize,
) -> bool {
    // 确保 hits 有足够层级
    while hits.len() <= MAXSNPS as usize {
        hits.push(Vec::new());
    }

    // 按 snp 数分类添加
    for hit in new_hits {
        let snp_level = hit.snps as usize;
        if snp_level < hits.len() {
            hits[snp_level].push(hit);
        }
    }

    // 检查总命中数
    let total_hits: usize = hits.iter().map(|v| v.len()).sum();
    total_hits >= max_hits
}

/// 将扁平位置转换为 GHit。
///
/// 对应 C++ `int2hit()` 函数。
///
/// # 参数
/// - `pos`: 扁平位置（u32）
/// - `coll`: 二进制参考序列集合
/// - `gap_size`: gap 大小
/// - `gap_pos`: gap 位置
/// - `read_chain`: 读段链（0=正向, 1=反向）
/// - `snps`: mismatch 数
/// - `map_readlen`: 读段长度
///
/// # 返回值
/// GHit 结构
pub fn int2ghit(
    pos: u32,
    coll: &BinSeqCollection,
    gap_size: i8,
    gap_pos: u8,
    read_chain: u8,
    snps: u8,
    map_readlen: u32,
) -> GHit {
    let (chr, loc) = coll.int2hit(pos);

    // 计算 strand: ref_chain is determined by the caller, read_chain is passed in
    let strand = (0 << 1) | read_chain; // ref_chain=0 since int2hit returns chr index

    // 根据 gap 调整位置
    let adjusted_loc = if gap_size < 0 {
        // 缺失：参考比读段长
        loc
    } else if gap_size > 0 {
        // 插入：读段比参考长
        loc
    } else {
        loc
    };

    let _ = map_readlen; // 使用参数避免警告

    GHit {
        loc: adjusted_loc,
        chr: chr / 2,
        strand,
        gap_size: gap_size as i16,
        gap_pos: gap_pos as u16,
        snps,
    }
}

/// 计算掩码中的 N 碱基数。
fn count_n_in_mask(mask: &[u64]) -> u32 {
    let mut count: u32 = 0;

    for &mask_word in mask {
        // 统计掩码中为 0 的位（表示 N）
        // 每 2-bit 表示一个碱基
        let inverted = !mask_word;
        for i in 0..32 {
            let bits = (inverted >> (62 - i * 2)) & 0b11;
            if bits == 0b11 {
                count += 1;
            }
        }
    }

    count
}

/// 选择最佳命中。
///
/// 从多层命中列表中选择最佳结果（mismatch 数最少）。
///
/// # 参数
/// - `hits`: 按 snp_level 组织的命中列表
///
/// # 返回值
/// 最佳命中列表和对应的 snp 数
pub fn select_best_hits(hits: &[Vec<GHit>]) -> (Vec<GHit>, u8) {
    for (snp_level, level_hits) in hits.iter().enumerate() {
        if !level_hits.is_empty() {
            return (level_hits.clone(), snp_level as u8);
        }
    }
    (Vec::new(), 0)
}

/// 统计唯一命中数。
///
/// 计算所有层级的唯一命中总数。
pub fn count_unique_hits(hits: &[Vec<GHit>]) -> usize {
    hits.iter().map(|v| v.len()).sum()
}

/// 清空命中列表。
pub fn clear_hits(hits: &mut [Vec<GHit>]) {
    for level in hits.iter_mut() {
        level.clear();
    }
}

/// 检查是否达到唯一比对。
///
/// 如果在最低 mismatch 级别只有一个命中，返回 true。
pub fn is_unique_hit(hits: &[Vec<GHit>]) -> bool {
    for level in hits {
        if level.len() == 1 {
            return true;
        }
        if level.len() > 1 {
            return false;
        }
    }
    false
}

/// 合并多个 segment 的命中结果。
///
/// 将多个 segment 的命中合并到一个列表中。
pub fn merge_segment_hits(segment_hits: Vec<Vec<GHit>>) -> Vec<Vec<GHit>> {
    let mut merged: Vec<Vec<GHit>> = vec![Vec::new(); MAXSNPS as usize + 1];

    for hits in segment_hits {
        for hit in hits {
            let level = hit.snps as usize;
            if level < merged.len() {
                merged[level].push(hit);
            }
        }
    }

    // 去重
    for level in merged.iter_mut() {
        level.sort_by(|a, b| {
            a.chr
                .cmp(&b.chr)
                .then_with(|| a.loc.cmp(&b.loc))
                .then_with(|| a.strand.cmp(&b.strand))
        });
        level.dedup_by(|a, b| a.chr == b.chr && a.loc == b.loc && a.strand == b.strand);
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_ghit(chr: u32, loc: u32, snps: u8) -> GHit {
        GHit {
            loc,
            chr,
            strand: 0,
            gap_size: 0,
            gap_pos: 0,
            snps,
        }
    }

    #[test]
    fn test_dedup_hits() {
        let mut hits = vec![
            ExtHit {
                chr: 0,
                loc: 100,
                snps: 0,
                chain: 0,
                gap_size: 0,
                gap_pos: 0,
            },
            ExtHit {
                chr: 0,
                loc: 100,
                snps: 0,
                chain: 0,
                gap_size: 0,
                gap_pos: 0,
            }, // 重复
            ExtHit {
                chr: 0,
                loc: 200,
                snps: 1,
                chain: 0,
                gap_size: 0,
                gap_pos: 0,
            },
        ];

        dedup_hits(&mut hits);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].loc, 100);
        assert_eq!(hits[1].loc, 200);
    }

    #[test]
    fn test_add_hits() {
        let mut hits: Vec<Vec<GHit>> = vec![Vec::new(); MAXSNPS as usize + 1];

        let new_hits = vec![
            make_test_ghit(0, 100, 0),
            make_test_ghit(0, 200, 1),
            make_test_ghit(0, 300, 1),
        ];

        let should_stop = add_hits(new_hits, &mut hits, 100);

        assert!(!should_stop);
        assert_eq!(hits[0].len(), 1);
        assert_eq!(hits[1].len(), 2);
    }

    #[test]
    fn test_add_hits_max_reached() {
        let mut hits: Vec<Vec<GHit>> = vec![Vec::new(); MAXSNPS as usize + 1];

        let new_hits: Vec<GHit> = (0..10).map(|i| make_test_ghit(0, i * 100, 0)).collect();

        let should_stop = add_hits(new_hits, &mut hits, 5);

        assert!(should_stop);
    }

    #[test]
    fn test_select_best_hits() {
        let hits: Vec<Vec<GHit>> = vec![
            vec![make_test_ghit(0, 100, 0)], // snp=0
            vec![],                          // snp=1
            vec![make_test_ghit(0, 200, 2), make_test_ghit(0, 300, 2)], // snp=2
        ];

        let (best, snp_level) = select_best_hits(&hits);

        assert_eq!(snp_level, 0);
        assert_eq!(best.len(), 1);
        assert_eq!(best[0].loc, 100);
    }

    #[test]
    fn test_count_unique_hits() {
        let hits: Vec<Vec<GHit>> = vec![
            vec![make_test_ghit(0, 100, 0)],
            vec![make_test_ghit(0, 200, 1), make_test_ghit(0, 300, 1)],
            vec![],
        ];

        assert_eq!(count_unique_hits(&hits), 3);
    }

    #[test]
    fn test_is_unique_hit() {
        let hits_unique: Vec<Vec<GHit>> = vec![
            vec![make_test_ghit(0, 100, 0)],
            vec![],
            vec![],
        ];
        assert!(is_unique_hit(&hits_unique));

        let hits_multiple: Vec<Vec<GHit>> = vec![
            vec![],
            vec![make_test_ghit(0, 100, 1), make_test_ghit(0, 200, 1)],
            vec![],
        ];
        assert!(!is_unique_hit(&hits_multiple));

        let hits_empty: Vec<Vec<GHit>> = vec![vec![], vec![], vec![]];
        assert!(!is_unique_hit(&hits_empty));
    }

    #[test]
    fn test_clear_hits() {
        let mut hits: Vec<Vec<GHit>> = vec![
            vec![make_test_ghit(0, 100, 0)],
            vec![make_test_ghit(0, 200, 1)],
        ];

        clear_hits(&mut hits);

        assert!(hits[0].is_empty());
        assert!(hits[1].is_empty());
    }

    #[test]
    fn test_merge_segment_hits() {
        let seg1 = vec![
            make_test_ghit(0, 100, 0),
            make_test_ghit(0, 200, 1),
        ];
        let seg2 = vec![
            make_test_ghit(0, 150, 0),
            make_test_ghit(0, 200, 1), // 重复
        ];

        let merged = merge_segment_hits(vec![seg1, seg2]);

        assert_eq!(merged[0].len(), 2); // 100 和 150
        assert_eq!(merged[1].len(), 1); // 200（去重）
    }

    #[test]
    fn test_calculate_strand() {
        assert_eq!(calculate_strand(0, 0), 0b00); // ++
        assert_eq!(calculate_strand(1, 0), 0b01); // +-
        assert_eq!(calculate_strand(0, 1), 0b10); // -+
        assert_eq!(calculate_strand(1, 1), 0b11); // --
    }

    #[test]
    fn test_ext_hit_to_ghit() {
        let ext = ExtHit {
            chr: 5,
            loc: 1000,
            snps: 2,
            chain: 1,
            gap_size: -1,
            gap_pos: 10,
        };

        let ghit = ext.to_ghit(0b11);

        assert_eq!(ghit.chr, 5);
        assert_eq!(ghit.loc, 1000);
        assert_eq!(ghit.strand, 0b11);
        assert_eq!(ghit.gap_size, -1);
        assert_eq!(ghit.gap_pos, 10);
    }
}
