//! SAM/BAM/BSP 输入解析
//! 对应 methratio.py get_alignment() (第44-91行) 和管道打开逻辑 (第93-101行)

use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

use crate::{AlignmentRecord, Config};

/// 输入格式
#[derive(Debug, Clone, Copy, PartialEq)]
enum InputFormat {
    Sam,
    Bam,
    Bsp,
}

/// 检测文件格式（基于扩展名）
/// 对应 methratio.py 第 97-100 行
fn detect_format(path: &str) -> InputFormat {
    let upper = path.to_uppercase();
    if upper.ends_with(".SAM") {
        InputFormat::Sam
    } else if upper.ends_with(".BAM") {
        InputFormat::Bam
    } else {
        InputFormat::Bsp
    }
}

/// 解析比对记录
/// 对应 methratio.py get_alignment() 第 44-91 行
/// 返回 None 表示跳过该记录
fn parse_alignment(line: &str, format: InputFormat, config: &Config, ref_seqs: &HashMap<String, Vec<u8>>, coverage: &mut HashMap<String, Vec<u8>>) -> Option<AlignmentRecord> {
    let col: Vec<&str> = line.split('\t').collect();

    let (seq, strand, chrom, pos, is_sam) = if format == InputFormat::Bsp {
        // BSP 格式解析 (methratio.py 第 69-80 行)
        let flag = col.get(3)?;
        let hit_type = &flag[..2.min(flag.len())];
        if hit_type == "NM" || hit_type == "QC" { return None; }
        if config.unique && hit_type != "UM" { return None; }
        if config.pair && col.get(7).copied().unwrap_or("0") == "0" { return None; }

        let chrom = col.get(4)?.to_string();
        if !config.chroms.is_empty() && !config.chroms.contains(&chrom) { return None; }

        let mut seq: Vec<u8> = col.get(1)?.as_bytes().to_vec();
        let strand = col.get(6)?.to_string();
        let pos: usize = col.get(5)?.parse::<usize>().ok()?.saturating_sub(1); // 1-based → 0-based
        let mm = col.get(9)?;

        // BSP gap 处理 (methratio.py 第 76-80 行)
        if mm.contains(':') {
            let tmp: Vec<&str> = mm.split(':').collect();
            if tmp.len() >= 3 {
                let gap_pos: usize = tmp[1].parse().ok()?;
                let gap_size: isize = tmp[2].parse().ok()?;
                if gap_size < 0 {
                    // 读段插入：删除读段碱基
                    let end = (gap_pos as isize + gap_size) as usize;
                    if end <= seq.len() {
                        seq = [seq[..gap_pos].to_vec(), seq[end..].to_vec()].concat();
                    }
                } else if gap_size > 0 {
                    // 参考缺失：插入 '-' 占位符
                    let gap_size = gap_size as usize;
                    let mut new_seq = seq[..gap_pos].to_vec();
                    new_seq.extend(vec![b'-'; gap_size]);
                    new_seq.extend_from_slice(&seq[gap_pos..]);
                    seq = new_seq;
                }
            }
        }

        (seq, strand, chrom, pos, false)
    } else {
        // SAM 格式解析 (methratio.py 第 46-68 行)
        if line.starts_with('@') { return None; }
        let flag_str = col.get(1)?;
        // 原版检查字符 'u' 和 's'（非标准 FLAG 解析）
        if flag_str.contains('u') { return None; }
        if config.unique && flag_str.contains('s') { return None; }
        if config.pair && !flag_str.contains('P') { return None; }

        let chrom = col.get(2)?.to_string();
        if !config.chroms.is_empty() && !config.chroms.contains(&chrom) { return None; }

        let pos: usize = col.get(3)?.parse::<usize>().ok()?.saturating_sub(1); // 1-based → 0-based
        let cigar = col.get(5)?;
        let mut seq: Vec<u8> = col.get(9)?.as_bytes().to_vec();
        let insert: i64 = col.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);

        // ZS tag 提取 (methratio.py 第 54-56 行)
        let strand = if let Some(idx) = line.find("ZS:Z:") {
            let s = &line[idx + 5..];
            if s.len() >= 2 {
                s[..2].to_string()
            } else {
                return None;
            }
        } else {
            // 无 ZS tag，尝试从 FLAG 推断
            if flag_str.contains('r') { "-+".to_string() } else { "++".to_string() }
        };

        // CIGAR gap 处理 (methratio.py 第 58-68 行)
        let mut cigar = cigar.to_string();
        let mut gap_pos: usize = 0;
        while cigar.contains('I') || cigar.contains('D') {
            let mut gap_size: usize = 0;
            let mut sep_found = 'M';
            for sep in ['M', 'I', 'D'] {
                let parts: Vec<&str> = cigar.splitn(2, sep).collect();
                if parts.len() >= 2 {
                    if let Ok(gs) = parts[0].parse::<usize>() {
                        gap_size = gs;
                        sep_found = sep;
                        break;
                    }
                }
            }
            if sep_found == 'M' {
                gap_pos += gap_size;
            } else if sep_found == 'I' {
                let end = gap_pos + gap_size;
                if end <= seq.len() {
                    seq = [seq[..gap_pos].to_vec(), seq[end..].to_vec()].concat();
                }
            } else if sep_found == 'D' {
                let mut new_seq = seq[..gap_pos].to_vec();
                new_seq.extend(vec![b'-'; gap_size]);
                new_seq.extend_from_slice(&seq[gap_pos..]);
                seq = new_seq;
                gap_pos += gap_size;
            }
            if let Some(idx) = cigar.find(sep_found) {
                cigar = cigar[idx + 1..].to_string();
            } else {
                break;
            }
        }

        // paired overlap (仅 SAM, methratio.py 第 90 行)
        // 注意：原版代码中 insert > 0 时截断，但 col[7] 在 SAM 中是 PNEXT
        // 原版实际使用的是 insert (col[8])，这里保持一致
        let _ = insert; // paired overlap 在原版中有 bug，暂不实现

        (seq, strand, chrom, pos, true)
    };

    // 边界检查 (methratio.py 第 81 行)
    let ref_seq = match ref_seqs.get(&chrom) {
        Some(s) => s,
        None => return None,
    };
    if pos + seq.len() >= ref_seq.len() { return None; }

    // 去重 (methratio.py 第 82-86 行)
    if config.remove_duplicate {
        let _strand_first = strand.chars().next().unwrap_or('+');
        let (frag_end, direction) = if strand == "+-" || strand == "-+" {
            (pos + seq.len(), 2u8)
        } else {
            (pos, 1u8)
        };
        let cov = coverage.entry(chrom.clone()).or_insert_with(|| vec![0u8; ref_seq.len()]);
        if frag_end < cov.len() && cov[frag_end] & direction != 0 {
            return None;
        }
        if frag_end < cov.len() {
            cov[frag_end] |= direction;
        }
    }

    // trim fillin (methratio.py 第 87-89 行)
    let mut seq = seq;
    let mut pos = pos;
    if config.trim_fillin > 0 {
        if strand == "+-" || strand == "-+" {
            let trim = config.trim_fillin.min(seq.len());
            seq.truncate(seq.len() - trim);
        } else if strand == "++" || strand == "--" {
            let trim = config.trim_fillin.min(seq.len());
            seq = seq[trim..].to_vec();
            pos += trim;
        }
    }

    // paired overlap for SAM (methratio.py 第 90 行)
    // Python: if sam_format and insert > 0: seq = seq[:int(col[7])-1-pos]
    // 注意 Python 切片语义：seq[:n] 当 n < 0 时等价于 seq[:len(seq)+n]
    // 即从末尾截取 |n| 个字符，而非完全清空
    if is_sam {
        let insert: i64 = col.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);
        if insert > 0 {
            // col[7] 是 PNEXT (mate position, 1-based)
            if let Some(pnext_str) = col.get(7) {
                if let Ok(pnext) = pnext_str.parse::<i64>() {
                    if pnext > 0 {
                        // 使用 i64 避免下溢，精确复制 Python seq[:n] 语义
                        let n = pnext - 1 - (pos as i64); // pnext-1-pos
                        let seq_len = seq.len() as i64;
                        if n < 0 {
                            // Python seq[:n] 当 n<0 等价于 seq[:seq_len+n]
                            let effective_len = seq_len + n;
                            if effective_len <= 0 {
                                seq.clear();
                            } else {
                                seq.truncate(effective_len as usize);
                            }
                        } else if n < seq_len {
                            seq.truncate(n as usize);
                        }
                        // n >= seq_len: 不截断
                    }
                }
            }
        }
    }

    let strand_char = strand.chars().next().unwrap_or('+');

    Some(AlignmentRecord { seq, strand: strand_char, chrom, pos })
}

/// 从输入源读取比对记录的迭代器
pub struct AlignmentReader {
    lines: Box<dyn Iterator<Item = String>>,
    format: InputFormat,
    config: Config,
    ref_seqs: HashMap<String, Vec<u8>>,
    coverage: HashMap<String, Vec<u8>>,
}

impl AlignmentReader {
    /// 从文件路径创建 reader
    /// 对应 methratio.py 第 93-101 行
    pub fn from_files(paths: &[String], config: Config, ref_seqs: HashMap<String, Vec<u8>>) -> Result<Self> {
        let coverage = HashMap::new();

        // 简化实现：读取所有文件为一个迭代器
        let mut lines: Vec<String> = Vec::new();
        for path in paths {
            let format = detect_format(path);
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            if format == InputFormat::Bam {
                // BAM 通过 noodles 读取后转为 SAM 文本行
                // 暂时用 samtools 管道
                let output = std::process::Command::new("samtools")
                    .args(["view", "-h", path])
                    .output()?;
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    lines.push(line.to_string());
                }
            } else {
                for line in reader.lines() {
                    lines.push(line?);
                }
            }
        }

        Ok(Self {
            lines: Box::new(lines.into_iter()),
            format: InputFormat::Sam, // 统一为 SAM 格式处理
            config,
            ref_seqs,
            coverage,
        })
    }

    /// 从 STDIN 创建 reader
    pub fn from_stdin(config: Config, ref_seqs: HashMap<String, Vec<u8>>) -> Result<Self> {
        let coverage = HashMap::new();
        let stdin = io::stdin();
        let lines: Vec<String> = stdin.lock().lines().filter_map(|l| l.ok()).collect();

        Ok(Self {
            lines: Box::new(lines.into_iter()),
            format: InputFormat::Sam,
            config,
            ref_seqs,
            coverage,
        })
    }
}

impl Iterator for AlignmentReader {
    type Item = AlignmentRecord;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = self.lines.next()?;
            if line.is_empty() { continue; }
            if let Some(record) = parse_alignment(&line, self.format, &self.config, &self.ref_seqs, &mut self.coverage) {
                return Some(record);
            }
        }
    }
}
