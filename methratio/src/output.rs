//! TXT + WIG 输出格式
//! 对应 methratio.py 第 194-257 行

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use crate::{ChromosomeCounts, Config};

/// 判定上下文类型
/// 对应 methratio.py 第 219-233 行
fn get_context(ref_seq: &[u8], i: usize, is_c: bool) -> Option<&'static str> {
    if is_c {
        // 参考碱基为 C（正链）
        if i + 2 >= ref_seq.len() { return None; }
        if ref_seq[i + 1] == b'G' { return Some("CG"); }
        if ref_seq[i + 2] == b'G' { return Some("CHG"); }
        Some("CHH")
    } else {
        // 参考碱基为 G（负链）
        if i == 0 { return None; }
        if ref_seq[i - 1] == b'C' { return Some("CG"); }
        if i == 1 { return None; }
        if ref_seq[i - 2] == b'C' { return Some("CHG"); }
        Some("CHH")
    }
}

/// 写入 TXT 输出
/// 对应 methratio.py 第 200-253 行
pub fn write_txt(
    counts: &HashMap<String, ChromosomeCounts>,
    ref_seqs: &HashMap<String, Vec<u8>>,
    config: &Config,
    writer: &mut dyn Write,
) -> std::io::Result<()> {
    // header (methratio.py 第 200-201 行)
    if !config.no_header {
        if config.ct_snp > 0 {
            writer.write_all(b"chr\tpos\tstrand\tcontext\tratio\teff_CT_count\tC_count\tCT_count\trev_G_count\trev_GA_count\tCI_lower\tCI_upper\n")?;
        } else {
            writer.write_all(b"chr\tpos\tstrand\tcontext\tratio\teff_CT_count\tC_count\tCT_count\tNA\tNA\tCI_lower\tCI_upper\n")?;
        }
    }

    let z95 = 1.96;
    let z95sq = z95 * z95;
    let mut nc: usize = 0;
    let mut nd: f64 = 0.0;

    // 按染色体名排序输出 (methratio.py 第 204 行)
    let mut chroms: Vec<&String> = counts.keys().collect();
    chroms.sort();

    for chrom in &chroms {
        let chrom_counts = counts.get(*chrom).unwrap();
        let ref_seq = match ref_seqs.get(*chrom) {
            Some(s) => s,
            None => continue,
        };

        // 收集所有有覆盖的位置并排序
        let mut positions: Vec<usize> = chrom_counts.depth.keys().cloned().collect();
        positions.sort();

        for i in positions {
            let dd = *chrom_counts.depth.get(&i).unwrap_or(&0);
            if (dd as usize) < config.min_depth { continue; }

            // CT_SNP 处理 (methratio.py 第 212-217 行)
            let d: f64 = if config.ct_snp > 0 {
                let m1 = *chrom_counts.meth1.get(&i).unwrap_or(&0);
                let d1 = *chrom_counts.depth1.get(&i).unwrap_or(&0);
                if m1 != d1 {
                    if config.ct_snp == 2 { continue; } // skip
                    dd as f64 * m1 as f64 / d1 as f64 // correct
                } else {
                    dd as f64
                }
            } else {
                dd as f64
            };

            // 判定上下文 (methratio.py 第 219-233 行)
            let is_c = ref_seq.get(i) == Some(&b'C');
            let is_g = ref_seq.get(i) == Some(&b'G');
            if !is_c && !is_g { continue; }

            let strand = if is_c { '+' } else { '-' };
            let context = match get_context(ref_seq, i, is_c) {
                Some(ctx) => ctx,
                None => continue,
            };

            // 上下文过滤 (methratio.py 第 234-235 行)
            if !config.context.is_empty() && !config.context.contains(&context.to_string()) {
                continue;
            }

            let m = *chrom_counts.meth.get(&i).unwrap_or(&0);
            // ratio = min(m, d) / d (methratio.py 第 237 行)
            let ratio: f64 = if d > 0.0 { (m as f64).min(d) / d } else { continue };

            nc += 1;
            nd += d;

            // Wilson CI (methratio.py 第 248-251 行)
            let pmid = ratio + z95sq / (2.0 * d);
            let sd = z95 * ((ratio * (1.0 - ratio) / d + z95sq / (4.0 * d * d)).sqrt());
            let denom = 1.0 + z95sq / d;
            let ci_lower: f64 = (pmid - sd) / denom;
            let ci_upper: f64 = (pmid + sd) / denom;

            if config.ct_snp > 0 {
                let m1 = *chrom_counts.meth1.get(&i).unwrap_or(&0);
                let d1 = *chrom_counts.depth1.get(&i).unwrap_or(&0);
                let line = format!("{}\t{}\t{}\t{}\t{:.3}\t{:.2}\t{}\t{}\t{}\t{}\t{:.3}\t{:.3}\n",
                    chrom, i + 1, strand, context, ratio, d, m, dd, m1, d1, ci_lower, ci_upper);
                writer.write_all(line.as_bytes())?;
            } else {
                let line = format!("{}\t{}\t{}\t{}\t{:.3}\t{:.2}\t{}\t{}\tNA\tNA\t{:.3}\t{:.3}\n",
                    chrom, i + 1, strand, context, ratio, d, m, dd, ci_lower, ci_upper);
                writer.write_all(line.as_bytes())?;
            }
        }
    }

    // 统计信息 (methratio.py 第 257 行)
    eprintln!("[methratio] total {} covered cytosines, average coverage: {:.2} fold.",
        nc, if nc > 0 { nd / nc as f64 } else { 0.0 });

    Ok(())
}

/// 写入 WIG 输出
/// 对应 methratio.py 第 197-199, 241-247 行
pub fn write_wig(
    counts: &HashMap<String, ChromosomeCounts>,
    ref_seqs: &HashMap<String, Vec<u8>>,
    config: &Config,
    wig_path: &str,
) -> std::io::Result<()> {
    let mut fwig = File::create(wig_path)?;
    fwig.write_all(b"track type=wiggle_0\n")?;

    let mut chroms: Vec<&String> = counts.keys().collect();
    chroms.sort();

    let wig_bin = config.wig_bin;

    for chrom in &chroms {
        let chrom_counts = counts.get(*chrom).unwrap();
        let _ref_seq = match ref_seqs.get(*chrom) {
            Some(s) => s,
            None => continue,
        };

        writeln!(fwig, "variableStep chrom={} span={}", chrom, wig_bin)?;

        let mut positions: Vec<usize> = chrom_counts.depth.keys().cloned().collect();
        positions.sort();

        let mut bin_idx: usize = 0;
        let mut bin_depth: f64 = 0.0;
        let mut bin_meth: f64 = 0.0;

        for i in positions {
            let dd = *chrom_counts.depth.get(&i).unwrap_or(&0);
            if (dd as usize) < config.min_depth { continue; }

            let m = *chrom_counts.meth.get(&i).unwrap_or(&0);
            let current_bin = i / wig_bin;

            if current_bin != bin_idx {
                if bin_depth > 0.0 {
                    let ratio = (bin_meth / bin_depth).min(1.0);
                    writeln!(fwig, "{}\t{:.3}", bin_idx * wig_bin + 1, ratio)?;
                }
                bin_idx = current_bin;
                bin_depth = 0.0;
                bin_meth = 0.0;
            }

            bin_depth += dd as f64;
            bin_meth += m as f64;
        }

        // 写入最后一个 bin
        if bin_depth > 0.0 {
            let ratio = (bin_meth / bin_depth).min(1.0);
            writeln!(fwig, "{}\t{:.3}", bin_idx * wig_bin + 1, ratio)?;
        }
    }

    Ok(())
}
