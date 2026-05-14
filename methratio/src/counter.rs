//! 甲基化计数核心逻辑
//! 对应 methratio.py 第 126-164 行（计数）和第 170-192 行（combine CpG）

use std::collections::HashMap;

use crate::{AlignmentRecord, ChromosomeCounts, Config};

/// 对所有比对记录进行甲基化计数
/// 对应 methratio.py 第 127-164 行主循环
pub fn count_methylation(
    records: impl Iterator<Item = AlignmentRecord>,
    ref_seqs: &HashMap<String, Vec<u8>>,
    config: &Config,
) -> HashMap<String, ChromosomeCounts> {
    let mut counts: HashMap<String, ChromosomeCounts> = HashMap::new();
    let mut nmap: u64 = 0;

    for record in records {
        let ref_seq = match ref_seqs.get(&record.chrom) {
            Some(s) => s,
            None => continue,
        };

        let chrom_counts = counts.entry(record.chrom.clone()).or_default();

        count_single_record(&record, ref_seq, chrom_counts, config);
        nmap += 1;
    }

    if !config.quiet {
        eprintln!("[methratio] processed {} valid mappings", nmap);
    }

    if config.combine_cpg {
        combine_cpg(&mut counts, ref_seqs, config);
    }

    counts
}

/// 对单条比对记录进行甲基化计数
/// 对应 methratio.py 第 138-164 行
fn count_single_record(
    record: &AlignmentRecord,
    ref_seq: &[u8],
    counts: &mut ChromosomeCounts,
    config: &Config,
) {
    let strand = record.strand;
    let pos = record.pos;
    let seq = &record.seq;
    let pos2 = pos + seq.len();

    // 查找 BS_conversion 对应的转换规则
    // BS_conversion = {'+': ('C','T','G','A'), '-': ('G','A','C','T')}
    // (match_base, convert_base)
    // 甲基化判定：read_base == match_base（与参考碱基相同 = 未被 BS 转换 = 甲基化）
    let (match_base, convert_base) = match strand {
        '+' => (b'C', b'T'),
        '-' => (b'G', b'A'),
        _ => return,
    };

    // 正向计数 (methratio.py 第 144-152 行)
    let mut index = pos;
    while index < pos2 && index < ref_seq.len() {
        if ref_seq[index] == match_base {
            let read_base = if index >= pos && (index - pos) < seq.len() {
                seq[index - pos]
            } else {
                index += 1;
                continue;
            };

            // 只计入有效碱基（非 gap 占位符）
            if read_base == convert_base {
                // 未甲基化（BS 转换后的碱基）
                let depth_entry = counts.depth.entry(index).or_insert(0);
                if *depth_entry < 65535 {
                    *depth_entry += 1;
                }
            } else if read_base == match_base {
                // 甲基化（未被 BS 转换，与参考碱基相同）
                let depth_entry = counts.depth.entry(index).or_insert(0);
                if *depth_entry < 65535 {
                    *depth_entry += 1;
                }
                let meth_entry = counts.meth.entry(index).or_insert(0);
                if *meth_entry < 65535 {
                    *meth_entry += 1;
                }
            }
            // 其他碱基（如 N、-）不计入深度
        }
        index += 1;
    }

    // CT_SNP 反向计数 (methratio.py 第 153-164 行)
    if config.ct_snp == 0 {
        return;
    }

    // 反向链检查：
    // +链反向检查 G（预期 A），-链反向检查 C（预期 T）
    let (rc_match, rc_convert) = match strand {
        '+' => (b'G', b'A'),
        '-' => (b'C', b'T'),
        _ => return,
    };

    let mut index = pos;
    while index < pos2 && index < ref_seq.len() {
        if ref_seq[index] == rc_match {
            let read_base = if index >= pos && (index - pos) < seq.len() {
                seq[index - pos]
            } else {
                index += 1;
                continue;
            };

            if read_base == rc_convert {
                // 未甲基化（反向链）
                let depth_entry = counts.depth1.entry(index).or_insert(0);
                if *depth_entry < 65535 {
                    *depth_entry += 1;
                }
            } else if read_base == rc_match {
                // 甲基化（反向链）
                let depth_entry = counts.depth1.entry(index).or_insert(0);
                if *depth_entry < 65535 {
                    *depth_entry += 1;
                }
                let meth_entry = counts.meth1.entry(index).or_insert(0);
                if *meth_entry < 65535 {
                    *meth_entry += 1;
                }
            }
        }
        index += 1;
    }
}

/// Combine CpG 双链合并
/// 对应 methratio.py 第 170-192 行
fn combine_cpg(
    counts: &mut HashMap<String, ChromosomeCounts>,
    ref_seqs: &HashMap<String, Vec<u8>>,
    config: &Config,
) {
    for (chrom, ref_seq) in ref_seqs {
        let chrom_counts = match counts.get_mut(chrom) {
            Some(c) => c,
            None => continue,
        };

        // 查找所有 CG 位点
        let mut pos = 0;
        while pos + 1 < ref_seq.len() {
            if ref_seq[pos] == b'C' && ref_seq[pos + 1] == b'G' {
                // 合并 pos 和 pos+1 的计数
                let d0 = *chrom_counts.depth.get(&pos).unwrap_or(&0) as u32;
                let d1 = *chrom_counts.depth.get(&(pos + 1)).unwrap_or(&0) as u32;
                let m0 = *chrom_counts.meth.get(&pos).unwrap_or(&0) as u32;
                let m1 = *chrom_counts.meth.get(&(pos + 1)).unwrap_or(&0) as u32;

                let new_d = (d0 + d1).min(65535) as u16;
                let new_m = (m0 + m1).min(65535) as u16;

                chrom_counts.depth.insert(pos, new_d);
                chrom_counts.meth.insert(pos, new_m);
                chrom_counts.depth.insert(pos + 1, 0);
                chrom_counts.meth.insert(pos + 1, 0);

                // CT_SNP 反向链也合并
                if config.ct_snp > 0 {
                    let d0 = *chrom_counts.depth1.get(&pos).unwrap_or(&0) as u32;
                    let d1 = *chrom_counts.depth1.get(&(pos + 1)).unwrap_or(&0) as u32;
                    let m0 = *chrom_counts.meth1.get(&pos).unwrap_or(&0) as u32;
                    let m1 = *chrom_counts.meth1.get(&(pos + 1)).unwrap_or(&0) as u32;

                    chrom_counts.depth1.insert(pos, (d0 + d1).min(65535) as u16);
                    chrom_counts.meth1.insert(pos, (m0 + m1).min(65535) as u16);
                    chrom_counts.depth1.insert(pos + 1, 0);
                    chrom_counts.meth1.insert(pos + 1, 0);
                }

                pos += 2;
            } else {
                pos += 1;
            }
        }
    }
}
