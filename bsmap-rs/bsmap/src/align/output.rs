//! SAM/BSP 输出格式化模块。
//!
//! 对应 C++ align.cpp 中的 `StringAlign()` 和 `s_OutHit()` 函数。
//! 提供比对结果的 SAM 和 BSP 格式输出。
//!
//! ## 输出格式
//!
//! - **SAM**: 标准 SAM 格式，包含 FLAG、RNAME、POS、MAPQ、CIGAR 等字段
//! - **BSP**: BSMAP 自定义格式，包含链信息和 mismatch 数

use crate::align::Chain;
use crate::param::{AlignConfig, GHit, ReadInf};
use crate::reference::binseq::BinSeqCollection;

/// 输出格式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// SAM 格式（标准比对格式）。
    Sam,
    /// BSP 格式（BSMAP 自定义格式）。
    Bsp,
}

/// 比对记录。
///
/// 包含格式化为字符串所需的所有信息。
#[derive(Debug, Clone)]
pub struct AlignmentRecord {
    /// 读段名称。
    pub read_name: String,
    /// 读段序列。
    pub seq: String,
    /// 质量值。
    pub qual: String,
    /// 参考序列名称。
    pub ref_name: String,
    /// 比对位置（1-based）。
    pub pos: u32,
    /// SAM FLAG。
    pub flag: u16,
    /// MAPQ 分数。
    pub mapq: u8,
    /// CIGAR 字符串。
    pub cigar: String,
    /// Mismatch 数（NM 标签）。
    pub nm: u32,
    /// 链信息（ZS 标签）。
    pub zs: String,
    /// 是否为唯一比对。
    pub is_unique: bool,
    /// 总命中数。
    pub total_hits: usize,
}

/// 格式化比对结果为 SAM 行，写入 buffer。
pub fn format_sam(
    buf: &mut String,
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) {
    use std::fmt::Write;
    let chain = Chain::from_strand(hit.strand);
    let flag = calculate_flag(chain, is_unique, total_hits);
    let rev_seq = !chain.is_ref_forward();
    let (seq, qual) = select_output_seq(read, rev_seq);
    let zs = make_zs_tag(
        if chain.is_ref_forward() { 0 } else { 1 },
        if chain.is_read_forward() { 0 } else { 1 },
    );
    let ref_name = get_reference_name(hit.chr, coll);
    let pos = hit.loc + 1;

    buf.clear();
    let _ = write!(
        buf,
        "{}\t{}\t{}\t{}\t255\t",
        read.name, flag, ref_name, pos,
    );
    // CIGAR
    write_cigar(buf, read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);
    let _ = write!(
        buf,
        "\t*\t0\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
        seq, qual, hit.snps, zs,
    );
}

/// 格式化比对结果为 BSP 行，写入 buffer。
pub fn format_bsp(
    buf: &mut String,
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    hit_type: &str,
) {
    use std::fmt::Write;
    let chain = Chain::from_strand(hit.strand);
    let ref_name = get_reference_name(hit.chr, coll);
    let seq = std::str::from_utf8(&read.seq).unwrap_or("");
    let pos = hit.loc + 1;
    let strand = make_zs_tag(chain.ref_chain(), chain.read_chain());

    buf.clear();
    let _ = write!(buf, "{}\t{}\t", read.name, seq);
    for &b in &read.qual {
        buf.push(b as char);
    }
    if hit.gap_size != 0 {
        let _ = write!(buf, "\t{}\t{}\t{}\t{}\t0\t*\t{}:{}:{}\t0",
            hit_type, ref_name, pos, strand,
            hit.snps, hit.gap_size.unsigned_abs(), hit.gap_pos);
    } else {
        let _ = write!(buf, "\t{}\t{}\t{}\t{}\t0\t*\t{}\t0",
            hit_type, ref_name, pos, strand, hit.snps);
    }
}

/// 格式化 AlignmentRecord 为 SAM 行（用于测试兼容）。
pub fn format_sam_record(record: &AlignmentRecord) -> String {
    use std::fmt::Write;
    let mut buf = String::with_capacity(256);
    let _ = write!(
        buf, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
        record.read_name, record.flag, record.ref_name, record.pos,
        record.mapq, record.cigar, "*", 0,
        record.seq, record.qual, record.nm, record.zs,
    );
    buf
}

/// 构建比对记录（保留用于 compat）。
fn build_record(
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) -> AlignmentRecord {
    build_record_impl(read, hit, coll, is_unique, total_hits)
}

/// 构建比对记录 — 内部实现。
fn build_record_impl(
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    is_unique: bool,
    total_hits: usize,
) -> AlignmentRecord {
    let ref_name = get_reference_name(hit.chr, coll);
    let chain = Chain::from_strand(hit.strand);
    let flag = calculate_flag(chain, is_unique, total_hits);
    // MAPQ fixed at 255
    let cigar = make_cigar(read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);
    let rev_seq = !chain.is_ref_forward();
    let (seq, qual) = select_output_seq(read, rev_seq);
    let zs = make_zs_tag(
        if chain.is_ref_forward() { 0 } else { 1 },
        if chain.is_read_forward() { 0 } else { 1 },
    );

    AlignmentRecord {
        read_name: read.name.clone(),
        seq: seq.into_owned(),
        qual: qual.into_owned(),
        ref_name,
        pos: hit.loc + 1,
        flag,
        mapq: 255,
        cigar,
        nm: hit.snps as u32,
        zs: zs.to_string(),
        is_unique,
        total_hits,
    }
}

/// 将 CIGAR 字符串写入 buffer。
pub fn write_cigar(buf: &mut String, map_readlen: u32, gap_size: i8, gap_pos: u8) {
    use std::fmt::Write;
    if gap_size == 0 {
        let _ = write!(buf, "{}M", map_readlen);
    } else {
        let gap_pos = gap_pos as u32;
        let gap_len = gap_size.unsigned_abs() as u32;

        if gap_size > 0 {
            let left = gap_pos;
            let right = map_readlen - gap_pos - gap_len;
            if right > 0 {
                let _ = write!(buf, "{}M{}I{}M", left, gap_len, right);
            } else {
                let _ = write!(buf, "{}M{}I", left, gap_len);
            }
        } else {
            let left = gap_pos;
            let right = map_readlen - gap_pos;
            if right > 0 {
                let _ = write!(buf, "{}M{}D{}M", left, gap_len, right);
            } else {
                let _ = write!(buf, "{}M{}D", left, gap_len);
            }
        }
    }
}

/// 生成 CIGAR 字符串（保留用于兼容）。
pub fn make_cigar(map_readlen: u32, gap_size: i8, gap_pos: u8) -> String {
    let mut buf = String::with_capacity(32);
    write_cigar(&mut buf, map_readlen, gap_size, gap_pos);
    buf
}

/// 生成 ZS 标签（链信息）。
pub fn make_zs_tag(ref_chain: u8, read_chain: u8) -> &'static str {
    match (ref_chain, read_chain) {
        (0, 0) => "++",
        (0, 1) => "+-",
        (1, 0) => "-+",
        (1, 1) => "--",
        _ => "++",
    }
}

/// 选择输出读段序列和质量值。
///
/// 不 revcomp 时返回借用，仅 revcomp 时分配新 String。
pub fn select_output_seq(read: &ReadInf, rev_seq: bool) -> (std::borrow::Cow<str>, std::borrow::Cow<str>) {
    if rev_seq {
        let seq: String = read
            .seq
            .iter()
            .rev()
            .map(|&b| match b {
                b'A' => 'T', b'T' => 'A', b'C' => 'G', b'G' => 'C',
                b'a' => 't', b't' => 'a', b'c' => 'g', b'g' => 'c',
                _ => 'N',
            })
            .collect();
        let qual: String = read.qual.iter().rev().map(|&b| b as char).collect();
        (seq.into(), qual.into())
    } else {
        (std::str::from_utf8(&read.seq).unwrap_or("").into(),
         std::str::from_utf8(&read.qual).unwrap_or("").into())
    }
}

/// 计算 SAM FLAG。
///
/// # 参数
/// - `chain`: 链信息
/// - `is_unique`: 是否为唯一比对
/// - `total_hits`: 总命中数
///
/// # 返回值
/// SAM FLAG 值
fn calculate_flag(chain: Chain, is_unique: bool, total_hits: usize) -> u16 {
    let mut flag: u16 = 0;

    // 0x4: 读段未比对（这里假设已比对）
    // 0x10: 序列反向互补
    // 对于反向参考链（-+ 和 --），序列需要反向互补
    if !chain.is_ref_forward() {
        flag |= 0x10;
    }

    // 0x100: 二次比对
    if !is_unique && total_hits > 1 {
        flag |= 0x100;
    }

    flag
}

/// 计算 MAPQ 分数。
///
/// # 参数
/// - `snps`: mismatch 数
/// - `is_unique`: 是否为唯一比对
/// - `total_hits`: 总命中数
///
/// # 返回值
/// MAPQ 分数（0-255）
/// 与 C++ BSMAP 保持一致：固定输出 255（表示 mapping quality 不可用）。
fn calculate_mapq(_snps: u32, _is_unique: bool, _total_hits: usize) -> u8 {
    255
}

/// 获取参考序列名称。
pub fn get_reference_name(chr: u32, coll: &BinSeqCollection) -> String {
    // chr 是染色体索引 (0-based)
    let chr_idx = chr as usize;
    if chr_idx < coll.chr_names.len() {
        // 只取空格前的部分（与 C++ BSMAP 一致）
        coll.chr_names[chr_idx]
            .split_whitespace()
            .next()
            .unwrap_or(&coll.chr_names[chr_idx])
            .to_string()
    } else {
        format!("chr{}", chr + 1)
    }
}

/// 获取染色体长度。
///
/// 从 ref_anchor 计算染色体长度（不包括 BINSEQPAD padding）。
pub fn get_chromosome_length(chr: u32, coll: &BinSeqCollection) -> u32 {
    let chr_idx = chr as usize;
    if chr_idx + 1 >= coll.ref_anchor.len() {
        return 0;
    }
    // ref_anchor 存储的是以碱基为单位的偏移量（包括 REF_MARGIN 和 BINSEQPAD）
    // 实际序列长度 = (end - start) - BINSEQPAD * SEGLEN
    let start = coll.ref_anchor[chr_idx];
    let end = coll.ref_anchor[chr_idx + 1];
    let padded_len = (end - start) as u32;
    // BINSEQPAD = 2, SEGLEN = 32
    let padding = 2 * 32;
    let chr_len = if padded_len > padding {
        padded_len - padding
    } else {
        0
    };
    
    // 对于单染色体情况，使用 sum_length 以获得更精确的长度
    if coll.total_num == 2 && chr_idx == 0 {
        coll.sum_length as u32
    } else {
        chr_len
    }
}

/// 生成 SAM 文件头。
///
/// # 参数
/// - `coll`: 二进制参考序列集合
/// - `program_name`: 程序名称
/// - `program_version`: 程序版本
///
/// # 返回值
/// SAM 头字符串
pub fn generate_sam_header(
    coll: &BinSeqCollection,
    program_name: &str,
    program_version: &str,
) -> String {
    let mut header = String::new();

    // HD 行（与 C++ BSMAP 格式对齐）
    header.push_str("@HD\tVN:1.0\n");

    // SQ 行（参考序列）- 只输出 Accession，不包含描述
    for (i, name) in coll.chr_names.iter().enumerate() {
        // 提取 Accession（第一个空格前的部分）
        let accession = name.split_whitespace().next().unwrap_or(name);
        // 计算该染色体的长度
        let len = if i + 1 < coll.ref_anchor.len() {
            coll.ref_anchor[i + 1] - coll.ref_anchor[i]
        } else {
            coll.sum_length as u32
        };
        header.push_str(&format!("@SQ\tSN:{}\tLN:{}\n", accession, len));
    }

    // PG 行（程序信息）- 与 C++ BSMAP 格式对齐
    header.push_str(&format!(
        "@PG\tID:{}\tVN:{}\n",
        program_name, program_version
    ));

    header
}

/// 格式化未比对读段，写入 buffer。
pub fn format_unmapped(buf: &mut String, read: &ReadInf, format: OutputFormat) {
    use std::fmt::Write;
    let seq = std::str::from_utf8(&read.seq).unwrap_or("");
    buf.clear();
    match format {
        OutputFormat::Sam => {
            let _ = write!(buf, "{}\t4\t*\t0\t0\t*\t*\t0\t0\t{}\t",
                read.name, seq);
            for &b in &read.qual {
                buf.push(b as char);
            }
        }
        OutputFormat::Bsp => {
            let _ = write!(buf, "{}\t{}\t", read.name, seq);
            for &b in &read.qual {
                buf.push(b as char);
            }
            let _ = write!(buf, "\tNM\t*\t0\t*\t0\t*\t0\t0");
        }
    }
}

/// 格式化质量控制失败的读段，写入 buffer。
pub fn format_qc_failed(buf: &mut String, read: &ReadInf, format: OutputFormat) {
    use std::fmt::Write;
    let seq = std::str::from_utf8(&read.seq).unwrap_or("");
    buf.clear();
    match format {
        OutputFormat::Sam => {
            let _ = write!(buf, "{}\t512\t*\t0\t0\t*\t*\t0\t0\t{}\t",
                read.name, seq);
            for &b in &read.qual {
                buf.push(b as char);
            }
        }
        OutputFormat::Bsp => {
            let _ = write!(buf, "{}\t{}\t", read.name, seq);
            for &b in &read.qual {
                buf.push(b as char);
            }
            let _ = write!(buf, "\tQC\t*\t0\t*\t0\t*\t0\t0");
        }
    }
}

/// 构建未比对读段的 BAM RecordBuf。
pub fn build_bam_record_unmapped(read: &ReadInf) -> noodles::sam::alignment::RecordBuf {
    use noodles::sam::alignment::{
        record::Flags,
        record_buf::{QualityScores, Sequence},
        RecordBuf,
    };

    RecordBuf::builder()
        .set_name(read.name.as_str())
        .set_flags(Flags::UNMAPPED)
        .set_sequence(Sequence::from(read.seq.clone()))
        .set_quality_scores(QualityScores::from(read.qual.clone()))
        .build()
}

/// 构建 QC 失败读段的 BAM RecordBuf。
pub fn build_bam_record_qc_failed(read: &ReadInf) -> noodles::sam::alignment::RecordBuf {
    use noodles::sam::alignment::{
        record::Flags,
        record_buf::{QualityScores, Sequence},
        RecordBuf,
    };

    RecordBuf::builder()
        .set_name(read.name.as_str())
        .set_flags(Flags::QC_FAIL)
        .set_sequence(Sequence::from(read.seq.clone()))
        .set_quality_scores(QualityScores::from(read.qual.clone()))
        .build()
}

/// 从比对数据直接构建 BAM RecordBuf，跳过 SAM 文本格式化与解析。
///
/// 针对单端比对的 SAM 格式记录（`format_sam` 的 BAM 等价物）。
pub fn build_bam_record_se(
    read: &ReadInf,
    hit: &GHit,
    is_unique: bool,
    total_hits: usize,
) -> noodles::sam::alignment::RecordBuf {
    use noodles::core::Position;
    use noodles::sam::alignment::{
        record::data::field::Tag,
        record_buf::{data::field::Value, Data},
        RecordBuf,
    };

    let chain = Chain::from_strand(hit.strand);
    let flag = calculate_flag(chain, is_unique, total_hits);
    let rev_seq = !chain.is_ref_forward();
    let (seq, qual) = select_output_seq(read, rev_seq);
    let zs = make_zs_tag(
        if chain.is_ref_forward() { 0 } else { 1 },
        if chain.is_read_forward() { 0 } else { 1 },
    );

    // CIGAR
    let cigar = build_cigar_vec(read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);

    // Tags: NM:i and ZS:Z
    let mut data = Data::default();
    data.insert(Tag::EDIT_DISTANCE, Value::UInt32(hit.snps as u32));
    let zs_tag = Tag::new(b'Z', b'S');
    data.insert(zs_tag, Value::String(zs.as_bytes().into()));

    RecordBuf::builder()
        .set_name(read.name.as_str())
        .set_flags(noodles::sam::alignment::record::Flags::from_bits_truncate(flag))
        .set_reference_sequence_id(hit.chr as usize)
        .set_alignment_start(Position::new(hit.loc as usize + 1).unwrap_or(Position::MIN))
        .set_cigar(cigar.into_iter().collect())
        .set_sequence(seq.as_bytes().to_vec().into())
        .set_quality_scores(qual.as_bytes().to_vec().into())
        .set_data(data)
        .build()
}

/// 从 gap 信息构建 CIGAR 操作列表。
fn build_cigar_vec(map_readlen: u32, gap_size: i8, gap_pos: u8) -> Vec<noodles::sam::alignment::record::cigar::Op> {
    use noodles::sam::alignment::record::cigar::{op::Kind, Op};

    let mut ops: Vec<Op> = Vec::new();
    if gap_size == 0 {
        ops.push(Op::new(Kind::Match, map_readlen as usize));
    } else {
        let gap_pos = gap_pos as u32;
        let gap_len = gap_size.unsigned_abs() as u32;

        if gap_size > 0 {
            let left = gap_pos;
            let right = map_readlen - gap_pos - gap_len;
            if left > 0 {
                ops.push(Op::new(Kind::Match, left as usize));
            }
            ops.push(Op::new(Kind::Insertion, gap_len as usize));
            if right > 0 {
                ops.push(Op::new(Kind::Match, right as usize));
            }
        } else {
            let left = gap_pos;
            let right = map_readlen - gap_pos;
            if left > 0 {
                ops.push(Op::new(Kind::Match, left as usize));
            }
            ops.push(Op::new(Kind::Deletion, gap_len as usize));
            if right > 0 {
                ops.push(Op::new(Kind::Match, right as usize));
            }
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_read() -> ReadInf {
        ReadInf {
            index: 0,
            read_set: 0,
            name: "test_read".to_string(),
            seq: b"ACGTACGTACGTACGT".to_vec(),
            qual: vec![33u8; 16], // "!" * 16
        }
    }

    fn make_test_hit() -> GHit {
        GHit {
            loc: 100,
            chr: 0,
            strand: 0, // ++
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        }
    }

    fn make_test_config() -> AlignConfig {
        AlignConfig::default()
    }

    #[test]
    fn test_make_cigar_no_gap() {
        let cigar = make_cigar(32, 0, 0);
        assert_eq!(cigar, "32M");
    }

    #[test]
    fn test_make_cigar_insertion() {
        let cigar = make_cigar(34, 2, 8);
        assert_eq!(cigar, "8M2I24M");
    }

    #[test]
    fn test_make_cigar_deletion() {
        let cigar = make_cigar(30, -2, 8);
        assert_eq!(cigar, "8M2D22M");
    }

    #[test]
    fn test_make_zs_tag() {
        assert_eq!(make_zs_tag(0, 0), "++");
        assert_eq!(make_zs_tag(0, 1), "+-");
        assert_eq!(make_zs_tag(1, 0), "-+");
        assert_eq!(make_zs_tag(1, 1), "--");
    }

    #[test]
    fn test_select_output_seq_forward() {
        let read = make_test_read();
        let (seq, qual) = select_output_seq(&read, false);

        assert_eq!(seq, "ACGTACGTACGTACGT");
        assert_eq!(qual, "!!!!!!!!!!!!!!!!");
    }

    #[test]
    fn test_select_output_seq_reverse() {
        let read = make_test_read();
        let (seq, qual) = select_output_seq(&read, true);

        // 反向互补：ACGTACGTACGTACGT -> ACGTACGTACGTACGT（回文）
        assert_eq!(seq, "ACGTACGTACGTACGT");
        assert_eq!(qual, "!!!!!!!!!!!!!!!!");
    }

    #[test]
    fn test_calculate_flag() {
        let chain_pp = Chain::PlusPlus;
        let chain_pm = Chain::PlusMinus;
        let chain_mp = Chain::MinusPlus;
        let chain_mm = Chain::MinusMinus;

        // 正向参考链（++ 和 +-）：无 0x10
        assert_eq!(calculate_flag(chain_pp, true, 1), 0);
        assert_eq!(calculate_flag(chain_pm, true, 1), 0);
        // 反向参考链（-+ 和 --）：设 0x10
        assert_eq!(calculate_flag(chain_mp, true, 1), 0x10);
        assert_eq!(calculate_flag(chain_mm, true, 1), 0x10);
        // 非唯一比对：设 0x100
        assert!(calculate_flag(chain_pp, false, 2) & 0x100 != 0);
        // 非唯一比对不应设 0x800
        assert_eq!(calculate_flag(chain_pp, true, 2) & 0x800, 0);
    }

    #[test]
    fn test_calculate_mapq() {
        // 与 C++ BSMAP 一致：MAPQ 固定输出 255
        assert_eq!(calculate_mapq(0, true, 1), 255);
        assert_eq!(calculate_mapq(1, true, 1), 255);
        assert_eq!(calculate_mapq(0, false, 2), 255);
    }

    #[test]
    fn test_format_unmapped_sam() {
        let read = make_test_read();
        let mut buf = String::new();
        format_unmapped(&mut buf, &read, OutputFormat::Sam);

        assert!(buf.contains("test_read"));
        assert!(buf.contains("\t4\t")); // FLAG=4
        assert!(buf.contains("\t*\t0\t0\t*")); // 未比对标记
    }

    #[test]
    fn test_format_unmapped_bsp() {
        let read = make_test_read();
        let mut buf = String::new();
        format_unmapped(&mut buf, &read, OutputFormat::Bsp);

        assert!(buf.contains("test_read"));
        assert!(buf.contains("\tNM"));
    }

    #[test]
    fn test_format_qc_failed_sam() {
        let read = make_test_read();
        let mut buf = String::new();
        format_qc_failed(&mut buf, &read, OutputFormat::Sam);

        assert!(buf.contains("test_read"));
        assert!(buf.contains("\t512\t")); // FLAG=512
    }

    #[test]
    fn test_format_qc_failed_bsp() {
        let read = make_test_read();
        let mut buf = String::new();
        format_qc_failed(&mut buf, &read, OutputFormat::Bsp);

        assert!(buf.contains("test_read"));
        assert!(buf.contains("\tQC"));
    }

    #[test]
    fn test_alignment_record() {
        let record = AlignmentRecord {
            read_name: "read1".to_string(),
            seq: "ACGT".to_string(),
            qual: "!!!!".to_string(),
            ref_name: "chr1".to_string(),
            pos: 100,
            flag: 0,
            mapq: 40,
            cigar: "4M".to_string(),
            nm: 0,
            zs: "++".to_string(),
            is_unique: true,
            total_hits: 1,
        };

        let sam = format_sam_record(&record);
        assert!(sam.contains("read1"));
        assert!(sam.contains("chr1"));
        assert!(sam.contains("100"));
        assert!(sam.contains("NM:i:0"));
        assert!(sam.contains("ZS:Z:++"));
    }

    #[test]
    fn test_generate_sam_header() {
        let refs = vec![crate::reference::fasta::Reference {
            name: "chr1".to_string(),
            seq: b"ACGT".repeat(100).to_vec(),
            len: 400,
        }];
        let coll = crate::reference::binseq::BinSeqCollection::from_references(&refs);

        let header = generate_sam_header(&coll, "bsmap", "1.0.0");

        assert!(header.contains("@HD"));
        assert!(header.contains("@SQ"));
        assert!(header.contains("@PG"));
        assert!(header.contains("bsmap"));
        assert!(header.contains("1.0.0"));
    }
}
