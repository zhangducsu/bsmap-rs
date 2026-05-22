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

/// 格式化比对结果为 SAM 记录。
///
/// # 参数
/// - `read`: 读段信息
/// - `hit`: 命中信息
/// - `coll`: 二进制参考序列集合
/// - `config`: 比对配置
/// - `is_unique`: 是否为唯一比对
/// - `total_hits`: 总命中数
///
/// # 返回值
/// SAM 格式的字符串
pub fn format_sam(
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) -> String {
    let record = build_record(read, hit, coll, config, is_unique, total_hits);
    format_sam_record(&record)
}

/// 格式化比对结果为 BSP 记录。
///
/// # 参数
/// - `read`: 读段信息
/// - `hit`: 命中信息
/// - `coll`: 二进制参考序列集合
/// - `config`: 比对配置
/// - `hit_type`: 命中类型（"UM"=唯一, "MA"=多重, "NM"=无命中, "QC"=质量控制失败）
///
/// # 返回值
/// BSP 格式的字符串
pub fn format_bsp(
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    hit_type: &str,
) -> String {
    // 获取参考名称
    let ref_name = get_reference_name(hit.chr, coll);

    // 解析链信息
    let chain = Chain::from_strand(hit.strand);
    let ref_strand = if chain.is_ref_forward() { '+' } else { '-' };
    let read_strand = if chain.is_read_forward() { '+' } else { '-' };

    // 列 1: read name
    // 列 2: mapped read sequence
    let seq = String::from_utf8_lossy(&read.seq);
    // 列 3: quality scores
    let qual: String = read.qual.iter().map(|&b| b as char).collect();
    // 列 4: hit_type (UM/MA/OF)
    // 列 5: ref_name
    // 列 6: 1-based position
    let pos = hit.loc + 1;
    // 列 7: strand combination
    let strand = format!("{}{}", ref_strand, read_strand);
    // 列 8: insert size (no pair info available in ReadInf, use 0)
    let ins_size = 0;
    // 列 9: Watson strand reference sequence (placeholder, not used by bsp2sam)
    let refseq = "*";
    // 列 10: mismatch info
    let mm_info = if hit.gap_size != 0 {
        format!("{}:{}:{}", hit.snps, hit.gap_size.unsigned_abs(), hit.gap_pos)
    } else {
        format!("{}", hit.snps)
    };
    // 列 11: mismatch hit count distribution (placeholder)
    let mismatch_info = "0";

    // BSP 11-column format (tab-separated):
    // id\tseq\tqual\tmap_flag\tref\tref_loc\tstrand\tins_size\trefseq\tmm_info\tmismatch_info
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        read.name,
        seq,
        qual,
        hit_type,
        ref_name,
        pos,
        strand,
        ins_size,
        refseq,
        mm_info,
        mismatch_info,
    )
}

/// 构建比对记录。
fn build_record(
    read: &ReadInf,
    hit: &GHit,
    coll: &BinSeqCollection,
    config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) -> AlignmentRecord {
    // 获取参考名称
    let ref_name = get_reference_name(hit.chr, coll);

    // 解析链信息
    let chain = Chain::from_strand(hit.strand);

    // 计算 FLAG
    let flag = calculate_flag(chain, is_unique, total_hits);

    // 计算 MAPQ
    let mapq = calculate_mapq(hit.snps as u32, is_unique, total_hits);

    // 构建 CIGAR
    let cigar = make_cigar(
        read.seq.len() as u32,
        hit.gap_size as i8,
        hit.gap_pos as u8,
    );

    // 选择输出序列（根据参考链）
    // 当参考链为反向时（-+ 和 --），输出反向互补序列
    let rev_seq = !chain.is_ref_forward();
    let (seq, qual) = select_output_seq(read, rev_seq);

    // 构建 ZS 标签
    let zs = make_zs_tag(
        if chain.is_ref_forward() { 0 } else { 1 },
        if chain.is_read_forward() { 0 } else { 1 },
    );

    // 计算输出位置
    // hit.loc 已统一为正向坐标（对应 C++ int2hit 行为）
    let pos = hit.loc + 1;

    AlignmentRecord {
        read_name: read.name.clone(),
        seq,
        qual,
        ref_name,
        pos,
        flag,
        mapq,
        cigar,
        nm: hit.snps as u32,
        zs,
        is_unique,
        total_hits,
    }
}

/// 格式化 SAM 记录。
fn format_sam_record(record: &AlignmentRecord) -> String {
    // SAM 格式：
    // QNAME\tFLAG\tRNAME\tPOS\tMAPQ\tCIGAR\tRNEXT\tPNEXT\tTLEN\tSEQ\tQUAL\t[TAG:TYPE:VALUE...]
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tNM:i:{}\tZS:Z:{}",
        record.read_name,
        record.flag,
        record.ref_name,
        record.pos,
        record.mapq,
        record.cigar,
        "*", // RNEXT
        0,   // PNEXT
        0,   // TLEN
        record.seq,
        record.qual,
        record.nm,
        record.zs
    )
}

/// 生成 CIGAR 字符串。
///
/// # 参数
/// - `map_readlen`: 读段长度
/// - `gap_size`: gap 大小（正数=插入，负数=缺失，0=无 gap）
/// - `gap_pos`: gap 位置
///
/// # 返回值
/// CIGAR 字符串
pub fn make_cigar(map_readlen: u32, gap_size: i8, gap_pos: u8) -> String {
    if gap_size == 0 {
        // 无 gap
        format!("{}M", map_readlen)
    } else {
        let gap_pos = gap_pos as u32;
        let gap_len = gap_size.unsigned_abs() as u32;

        if gap_size > 0 {
            // 插入：M I M
            let left = gap_pos;
            let right = map_readlen - gap_pos - gap_len;
            if right > 0 {
                format!("{}M{}I{}M", left, gap_len, right)
            } else {
                format!("{}M{}I", left, gap_len)
            }
        } else {
            // 缺失：M D M
            let left = gap_pos;
            let right = map_readlen - gap_pos;
            if right > 0 {
                format!("{}M{}D{}M", left, gap_len, right)
            } else {
                format!("{}M{}D", left, gap_len)
            }
        }
    }
}

/// 生成 ZS 标签（链信息）。
///
/// # 参数
/// - `ref_chain`: 参考链（0=正义链, 1=反义链）
/// - `read_chain`: 读段链（0=正义链, 1=反义链）
///
/// # 返回值
/// ZS 标签字符串
pub fn make_zs_tag(ref_chain: u8, read_chain: u8) -> String {
    match (ref_chain, read_chain) {
        (0, 0) => "++".to_string(), // BSW++
        (0, 1) => "+-".to_string(), // BSC+-
        (1, 0) => "-+".to_string(), // BSW-+
        (1, 1) => "--".to_string(), // BSC--
        _ => "++".to_string(),
    }
}

/// 选择最佳输出读段（正向或反向互补）。
///
/// # 参数
/// - `read`: 读段信息
/// - `rev_seq`: 是否使用反向互补序列
///
/// # 返回值
/// (序列, 质量值) 元组
pub fn select_output_seq(read: &ReadInf, rev_seq: bool) -> (String, String) {
    if rev_seq {
        // 返回反向互补序列
        let seq: String = read
            .seq
            .iter()
            .rev()
            .map(|&b| match b {
                b'A' => 'T',
                b'T' => 'A',
                b'C' => 'G',
                b'G' => 'C',
                b'a' => 't',
                b't' => 'a',
                b'c' => 'g',
                b'g' => 'c',
                _ => 'N',
            })
            .collect();

        let qual: String = read.qual.iter().rev().map(|&b| b as char).collect();

        (seq, qual)
    } else {
        // 返回正向序列
        let seq = String::from_utf8_lossy(&read.seq).to_string();
        let qual = read.qual.iter().map(|&b| b as char).collect();

        (seq, qual)
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

/// 格式化未比对读段。
///
/// # 参数
/// - `read`: 读段信息
/// - `format`: 输出格式
///
/// # 返回值
/// 未比对读段的格式化字符串
pub fn format_unmapped(read: &ReadInf, format: OutputFormat) -> String {
    match format {
        OutputFormat::Sam => {
            // SAM 格式：FLAG=4 表示未比对
            format!(
                "{}\t4\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                read.name,
                String::from_utf8_lossy(&read.seq),
                read.qual.iter().map(|&b| b as char).collect::<String>()
            )
        }
        OutputFormat::Bsp => {
            // BSP 11-column format: NM (no hit)
            let seq = String::from_utf8_lossy(&read.seq);
            let qual: String = read.qual.iter().map(|&b| b as char).collect();
            format!(
                "{}\t{}\t{}\tNM\t*\t0\t*\t0\t*\t0\t0",
                read.name, seq, qual
            )
        }
    }
}

/// 格式化质量控制失败的读段。
///
/// # 参数
/// - `read`: 读段信息
/// - `format`: 输出格式
///
/// # 返回值
/// QC 失败读段的格式化字符串
pub fn format_qc_failed(read: &ReadInf, format: OutputFormat) -> String {
    match format {
        OutputFormat::Sam => {
            // SAM 格式：FLAG=512 表示 QC 失败
            format!(
                "{}\t512\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                read.name,
                String::from_utf8_lossy(&read.seq),
                read.qual.iter().map(|&b| b as char).collect::<String>()
            )
        }
        OutputFormat::Bsp => {
            // BSP 11-column format: QC (quality control failed)
            let seq = String::from_utf8_lossy(&read.seq);
            let qual: String = read.qual.iter().map(|&b| b as char).collect();
            format!(
                "{}\t{}\t{}\tQC\t*\t0\t*\t0\t*\t0\t0",
                read.name, seq, qual
            )
        }
    }
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
        let output = format_unmapped(&read, OutputFormat::Sam);

        assert!(output.contains("test_read"));
        assert!(output.contains("\t4\t")); // FLAG=4
        assert!(output.contains("\t*\t0\t0\t*")); // 未比对标记
    }

    #[test]
    fn test_format_unmapped_bsp() {
        let read = make_test_read();
        let output = format_unmapped(&read, OutputFormat::Bsp);

        assert!(output.contains("test_read"));
        assert!(output.contains("\tNM"));
    }

    #[test]
    fn test_format_qc_failed_sam() {
        let read = make_test_read();
        let output = format_qc_failed(&read, OutputFormat::Sam);

        assert!(output.contains("test_read"));
        assert!(output.contains("\t512\t")); // FLAG=512
    }

    #[test]
    fn test_format_qc_failed_bsp() {
        let read = make_test_read();
        let output = format_qc_failed(&read, OutputFormat::Bsp);

        assert!(output.contains("test_read"));
        assert!(output.contains("\tQC"));
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
