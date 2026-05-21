//! 配对读段 SAM 输出格式化模块。
//!
//! 对应 C++ pairs.cpp 中的输出函数，包括：
//! - `s_OutHitPair()`: 配对 SAM 输出
//! - `s_OutHitUnpair()`: 未配对 SAM 输出
//! - `StringAlignPair()`: 配对结果格式化
//! - `StringAlignUnpair()`: 未配对结果格式化

use crate::align::output::{get_chromosome_length, make_zs_tag, select_output_seq, write_cigar};
use crate::align::Chain;
use crate::param::{AlignConfig, GHit, ReadInf};
use crate::pairs::pair::PairHit;
use crate::reference::binseq::BinSeqCollection;

/// 格式化配对结果为 SAM 记录。
///
/// 对应 C++ `s_OutHitPair()`。
///
/// 生成配对读段的 SAM 记录，包括：
/// - FLAG 计算（0x1=paired, 0x2=proper pair, 0x10/0x20=reverse, 0x40/0x80=first/second, 0x100=secondary）
/// - CIGAR 生成
/// - Insert size (TLEN) 计算
/// - ZS 标签生成
///
/// # 参数
/// - `read_a`: read_a 信息
/// - `read_b`: read_b 信息
/// - `pair_hit`: 配对命中
/// - `coll`: 二进制参考序列集合
/// - `config`: 比对配置
/// - `is_unique`: 是否为唯一配对
/// - `total_hits`: 总配对命中数
///
/// # 返回值
/// read_a 和 read_b 的 SAM 记录元组
pub fn format_pair_sam(
    buf_a: &mut String,
    buf_b: &mut String,
    read_a: &ReadInf,
    read_b: &ReadInf,
    pair_hit: &PairHit,
    coll: &BinSeqCollection,
    config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) {
    format_pair_read(
        buf_a,
        read_a,
        &pair_hit.a,
        &pair_hit.b,
        true,  // is_first
        pair_hit.chain,
        pair_hit.insert,
        coll,
        config,
        is_unique,
        total_hits,
    );

    format_pair_read(
        buf_b,
        read_b,
        &pair_hit.b,
        &pair_hit.a,
        false, // is_second
        pair_hit.chain,
        pair_hit.insert,
        coll,
        config,
        is_unique,
        total_hits,
    );
}

/// 格式化单个配对读段的 SAM 记录。
///
/// 对应 C++ `s_OutHitPair()` 中的单条记录输出。
///
/// # C++ BSMAP 关键逻辑
///
/// - FLAG 0x10: `pp.chain ^ (pp.a.chr % 2)`，Rust 中为 `chain ^ (hit.strand >> 1)`
/// - FLAG 0x20: 当 0x10 未设置时设置（mate 反向）
/// - POS: `hit.loc + 1`（直接使用内部链坐标，不做转换）
/// - PNEXT: `mate_hit.loc + 1`（直接使用）
/// - TLEN: 当 FLAG 0x10 设置时为负，否则为正
/// - ZS: `chain_flag[ref_chain]` + `chain_flag[chain]`
fn format_pair_read(
    buf: &mut String,
    read: &ReadInf,
    hit: &GHit,
    mate_hit: &GHit,
    is_first: bool,
    chain: u8,
    insert: u32,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
) {
    use std::fmt::Write;

    let ref_name = get_reference_name(hit.chr, coll);
    let ref_chain = hit.strand >> 1;

    let effective_chain = if is_first { chain } else { 1 - chain };
    let is_reverse = (effective_chain ^ ref_chain) == 1;

    let tlen: i32 = if is_reverse {
        -(insert as i32)
    } else {
        insert as i32
    };

    let flag = make_pair_flag(
        is_first,
        true,
        is_reverse,
        true,
        !is_reverse,
        true,
        !is_unique && total_hits > 1,
    );

    let (seq, qual) = select_output_seq(read, is_reverse);

    let ref_chain_char = if ref_chain == 0 { '+' } else { '-' };
    let chain_char = if is_first {
        if chain == 0 { '+' } else { '-' }
    } else {
        if chain == 0 { '-' } else { '+' }
    };

    let mate_ref_name = if hit.chr == mate_hit.chr {
        "=".to_string()
    } else {
        get_reference_name(mate_hit.chr, coll)
    };

    let pos = if ref_chain == 0 {
        hit.loc + 1
    } else {
        let chr_len = get_chromosome_length(hit.chr, coll);
        chr_len - hit.loc - read.seq.len() as u32 + 1
    };

    let mate_ref_chain = mate_hit.strand >> 1;
    let mate_pos = if mate_ref_chain == 0 {
        mate_hit.loc + 1
    } else {
        let mate_chr_len = get_chromosome_length(mate_hit.chr, coll);
        mate_chr_len - mate_hit.loc - read.seq.len() as u32 + 1
    };

    buf.clear();
    let _ = write!(buf, "{}\t{}\t{}\t{}\t255\t",
        strip_r_suffix(&read.name), flag, ref_name, pos);
    write_cigar(buf, read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);
    let _ = write!(buf, "\t{}\t{}\t{}\t{}\t{}\tNM:i:{}\tZS:Z:{}{}",
        mate_ref_name, mate_pos, tlen, seq, qual,
        hit.snps, ref_chain_char, chain_char);
}

/// 格式化未配对读段为 SAM 记录。
///
/// 对应 C++ `s_OutHitUnpair()`。
///
/// 当读段对中只有一个读段成功比对或两个都未配对时使用。
///
/// # 参数
/// - `read`: 读段信息
/// - `hit`: 命中信息（如果已比对）
/// - `mate_chr`: mate 的染色体（如果已知）
/// - `mate_loc`: mate 的位置（如果已知）
/// - `is_first`: 是否为第一个读段
/// - `coll`: 二进制参考序列集合
/// - `config`: 比对配置
/// - `is_unique`: 是否为唯一比对
/// - `total_hits`: 总命中数
/// - `mate_mapped`: mate 是否已比对
///
/// # 返回值
/// SAM 格式的字符串
pub fn format_unpair_sam(
    buf: &mut String,
    read: &ReadInf,
    hit: Option<&GHit>,
    mate_chr: Option<u16>,
    mate_loc: Option<u32>,
    is_first: bool,
    coll: &BinSeqCollection,
    _config: &AlignConfig,
    is_unique: bool,
    total_hits: usize,
    mate_mapped: bool,
) {
    match hit {
        Some(hit) => {
            format_mapped_unpair(
                buf,
                read,
                hit,
                mate_chr,
                mate_loc,
                is_first,
                coll,
                is_unique,
                total_hits,
                mate_mapped,
            )
        }
        None => {
            format_unmapped_unpair(
                buf,
                read,
                mate_chr,
                mate_loc,
                is_first,
                mate_mapped,
            )
        }
    }
}

/// 格式化已比对的未配对读段。
fn format_mapped_unpair(
    buf: &mut String,
    read: &ReadInf,
    hit: &GHit,
    mate_chr: Option<u16>,
    mate_loc: Option<u32>,
    is_first: bool,
    coll: &BinSeqCollection,
    is_unique: bool,
    total_hits: usize,
    mate_mapped: bool,
) {
    use std::fmt::Write;

    let ref_name = get_reference_name(hit.chr, coll);
    let chain = Chain::from_strand(hit.strand);
    let is_reverse = !chain.is_read_forward();

    let flag = make_unpair_flag(
        is_first,
        true,
        is_reverse,
        mate_mapped,
        false,
        !is_unique && total_hits > 1,
    );

    let (seq, qual) = select_output_seq(read, is_reverse);
    let zs = make_zs_tag(
        if chain.is_ref_forward() { 0 } else { 1 },
        if chain.is_read_forward() { 0 } else { 1 },
    );

    let (mate_ref, mate_pos) = if mate_mapped {
        if let (Some(chr), Some(loc)) = (mate_chr, mate_loc) {
            let mate_ref_name = if hit.chr == chr as u32 {
                "=".to_string()
            } else {
                get_reference_name(chr as u32, coll)
            };
            (mate_ref_name, loc + 1)
        } else {
            ("*".to_string(), 0)
        }
    } else {
        ("*".to_string(), 0)
    };

    buf.clear();
    let _ = write!(buf, "{}\t{}\t{}\t{}\t255\t",
        strip_r_suffix(&read.name), flag, ref_name, hit.loc + 1);
    write_cigar(buf, read.seq.len() as u32, hit.gap_size as i8, hit.gap_pos as u8);
    let _ = write!(buf, "\t{}\t{}\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
        mate_ref, mate_pos, seq, qual, hit.snps, zs);
}

/// 格式化未比对的未配对读段。
fn format_unmapped_unpair(
    buf: &mut String,
    read: &ReadInf,
    mate_chr: Option<u16>,
    mate_loc: Option<u32>,
    is_first: bool,
    mate_mapped: bool,
) {
    use std::fmt::Write;

    let mut flag: u16 = 0x1; // paired
    if is_first {
        flag |= 0x40;
    } else {
        flag |= 0x80;
    }
    flag |= 0x4; // unmapped
    if !mate_mapped {
        flag |= 0x8; // mate unmapped
    }

    let seq = std::str::from_utf8(&read.seq).unwrap_or("");

    let (mate_ref, mate_pos) = if mate_mapped {
        if let (Some(chr), Some(loc)) = (mate_chr, mate_loc) {
            (format!("chr{}", chr + 1), loc + 1)
        } else {
            ("*".to_string(), 0)
        }
    } else {
        ("*".to_string(), 0)
    };

    buf.clear();
    let _ = write!(buf, "{}\t{}\t*\t0\t0\t*\t{}\t{}\t0\t{}\t",
        strip_r_suffix(&read.name), flag, mate_ref, mate_pos, seq);
    for &b in &read.qual {
        buf.push(b as char);
    }
}

/// 生成配对 SAM 的 FLAG。
///
/// # 参数
/// - `is_first`: 是否为第一个读段
/// - `is_mapped`: 是否已比对
/// - `is_reverse`: 是否反向
/// - `mate_mapped`: mate 是否已比对
/// - `mate_reverse`: mate 是否反向
/// - `is_proper`: 是否为 proper pair
/// - `is_secondary`: 是否为二次比对
///
/// # 返回值
/// FLAG 值
fn make_pair_flag(
    is_first: bool,
    is_mapped: bool,
    is_reverse: bool,
    mate_mapped: bool,
    mate_reverse: bool,
    is_proper: bool,
    is_secondary: bool,
) -> u16 {
    let mut flag: u16 = 0;

    // 0x1: 模板有多个片段（配对测序）
    flag |= 0x1;

    // 0x2: 每个片段都正确比对（proper pair）
    if is_proper {
        flag |= 0x2;
    }

    // 0x4: 该片段未比对
    if !is_mapped {
        flag |= 0x4;
    }

    // 0x8: mate 未比对
    if !mate_mapped {
        flag |= 0x8;
    }

    // 0x10: 序列反向互补
    if is_reverse {
        flag |= 0x10;
    }

    // 0x20: mate 序列反向互补
    if mate_reverse {
        flag |= 0x20;
    }

    // 0x40: 第一个片段
    if is_first {
        flag |= 0x40;
    }

    // 0x80: 最后一个片段
    if !is_first {
        flag |= 0x80;
    }

    // 0x100: 二次比对
    if is_secondary {
        flag |= 0x100;
    }

    flag
}

/// 生成未配对 SAM 的 FLAG。
///
/// # 参数
/// - `is_first`: 是否为第一个读段
/// - `is_mapped`: 是否已比对
/// - `is_reverse`: 是否反向
/// - `mate_mapped`: mate 是否已比对
/// - `mate_reverse`: mate 是否反向
/// - `is_secondary`: 是否为二次比对
///
/// # 返回值
/// FLAG 值
fn make_unpair_flag(
    is_first: bool,
    is_mapped: bool,
    is_reverse: bool,
    mate_mapped: bool,
    mate_reverse: bool,
    is_secondary: bool,
) -> u16 {
    let mut flag: u16 = 0;

    // 0x1: 模板有多个片段（配对测序）
    flag |= 0x1;

    // 0x4: 该片段未比对
    if !is_mapped {
        flag |= 0x4;
    }

    // 0x8: mate 未比对
    if !mate_mapped {
        flag |= 0x8;
    }

    // 0x10: 序列反向互补
    if is_reverse {
        flag |= 0x10;
    }

    // 0x20: mate 序列反向互补
    if mate_reverse {
        flag |= 0x20;
    }

    // 0x40: 第一个片段
    if is_first {
        flag |= 0x40;
    }

    // 0x80: 最后一个片段
    if !is_first {
        flag |= 0x80;
    }

    // 0x100: 二次比对
    if is_secondary {
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

/// 去除 QNAME 的 read 编号后缀（与 C++ BSMAP 保持一致）。
/// 支持 _R1/_R2、_r1/_r2、/1//2 格式。
fn strip_r_suffix(name: &str) -> &str {
    if name.ends_with("_R1") || name.ends_with("_R2") || 
       name.ends_with("_r1") || name.ends_with("_r2") {
        &name[..name.len()-3]
    } else if name.ends_with("/1") || name.ends_with("/2") {
        &name[..name.len()-2]
    } else {
        name
    }
}

/// 获取参考序列名称。
fn get_reference_name(chr: u32, coll: &BinSeqCollection) -> String {
    // 与 C++ BSMAP 保持一致：只输出 Accession（第一个空格前的部分）
    if (chr as usize) < coll.chr_names.len() {
        coll.chr_names[chr as usize]
            .split_whitespace()
            .next()
            .unwrap_or(&coll.chr_names[chr as usize])
            .to_string()
    } else {
        format!("chr{}", chr + 1)
    }
}

/// 格式化未配对读段对为 SAM 记录。
///
/// 这是 `format_unpair_sam` 的便捷包装，直接处理 `PairBatchResult`。
///
/// # 参数
/// - `read_a`: 第一个读段
/// - `read_b`: 第二个读段
/// - `result`: 配对批量处理结果
/// - `coll`: 二进制参考序列集合
/// - `config`: 比对配置
///
/// # 返回值
/// read_a 和 read_b 的 SAM 记录元组
pub fn format_unpair_sam_pair(
    buf_a: &mut String,
    buf_b: &mut String,
    read_a: &ReadInf,
    read_b: &ReadInf,
    result: &crate::pairs::PairBatchResult,
    coll: &BinSeqCollection,
    config: &AlignConfig,
) {
    let hit_a = result.unpair_hits_a.first();
    let mate_chr_a = result.unpair_hits_b.first().map(|h| h.chr as u16);
    let mate_loc_a = result.unpair_hits_b.first().map(|h| h.loc);
    let mate_mapped_a = !result.unpair_hits_b.is_empty();

    let hit_b = result.unpair_hits_b.first();
    let mate_chr_b = result.unpair_hits_a.first().map(|h| h.chr as u16);
    let mate_loc_b = result.unpair_hits_a.first().map(|h| h.loc);
    let mate_mapped_b = !result.unpair_hits_a.is_empty();

    let total_hits_a = result.unpair_hits_a.len();
    let total_hits_b = result.unpair_hits_b.len();

    format_unpair_sam(
        buf_a,
        read_a,
        hit_a,
        mate_chr_a,
        mate_loc_a,
        true,
        coll,
        config,
        total_hits_a == 1,
        total_hits_a,
        mate_mapped_a,
    );

    format_unpair_sam(
        buf_b,
        read_b,
        hit_b,
        mate_chr_b,
        mate_loc_b,
        false,
        coll,
        config,
        total_hits_b == 1,
        total_hits_b,
        mate_mapped_b,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_read_a() -> ReadInf {
        ReadInf {
            index: 0,
            read_set: 1,
            name: "read1/1".to_string(),
            seq: b"ACGTACGTACGTACGT".to_vec(),
            qual: vec![33u8; 16],
        }
    }

    fn make_test_read_b() -> ReadInf {
        ReadInf {
            index: 1,
            read_set: 2,
            name: "read1/2".to_string(),
            seq: b"TGCATGCATGCATGCA".to_vec(),
            qual: vec![33u8; 16],
        }
    }

    fn make_test_hit_a() -> GHit {
        GHit {
            loc: 100,
            chr: 0,
            strand: 0, // ++
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        }
    }

    fn make_test_hit_b() -> GHit {
        GHit {
            loc: 200,
            chr: 0,
            strand: 1, // +-
            gap_size: 0,
            gap_pos: 0,
            snps: 1,
        }
    }

    fn make_test_pair_hit() -> PairHit {
        PairHit {
            chain: 0,
            na: 0,
            nb: 1,
            insert: 116, // 200 + 16 - 100 = 116
            a: make_test_hit_a(),
            b: make_test_hit_b(),
        }
    }

    #[test]
    fn test_make_pair_flag() {
        // 第一个读段，正向，proper pair，唯一比对
        let flag = make_pair_flag(true, true, false, true, true, true, false);
        assert!(flag & 0x1 != 0);   // paired
        assert!(flag & 0x2 != 0);   // proper pair
        assert!(flag & 0x4 == 0);   // mapped
        assert!(flag & 0x8 == 0);   // mate mapped
        assert!(flag & 0x10 == 0);  // not reverse
        assert!(flag & 0x20 != 0);  // mate reverse
        assert!(flag & 0x40 != 0);  // first
        assert!(flag & 0x80 == 0);  // not last
        assert!(flag & 0x100 == 0); // not secondary

        // 第二个读段，反向，proper pair，多重比对
        let flag = make_pair_flag(false, true, true, true, false, true, true);
        assert!(flag & 0x1 != 0);   // paired
        assert!(flag & 0x2 != 0);   // proper pair
        assert!(flag & 0x4 == 0);   // mapped
        assert!(flag & 0x8 == 0);   // mate mapped
        assert!(flag & 0x10 != 0);  // reverse
        assert!(flag & 0x20 == 0);  // mate not reverse
        assert!(flag & 0x40 == 0);  // not first
        assert!(flag & 0x80 != 0);  // last
        assert!(flag & 0x100 != 0); // secondary
    }

    #[test]
    fn test_make_unpair_flag() {
        // 第一个读段已比对，mate 未比对
        let flag = make_unpair_flag(true, true, false, false, false, false);
        assert!(flag & 0x1 != 0);   // paired
        assert!(flag & 0x4 == 0);   // mapped
        assert!(flag & 0x8 != 0);   // mate unmapped
        assert!(flag & 0x40 != 0);  // first

        // 第二个读段未比对，mate 已比对
        let flag = make_unpair_flag(false, false, false, true, true, false);
        assert!(flag & 0x1 != 0);   // paired
        assert!(flag & 0x4 != 0);   // unmapped
        assert!(flag & 0x8 == 0);   // mate mapped
        assert!(flag & 0x80 != 0);  // last
    }

    #[test]
    fn test_calculate_mapq() {
        // 与 C++ BSMAP 保持一致：固定输出 255
        assert_eq!(calculate_mapq(0, true, 1), 255);
        assert_eq!(calculate_mapq(1, true, 1), 255);
        assert_eq!(calculate_mapq(5, true, 1), 255);
        assert_eq!(calculate_mapq(0, false, 2), 255);
        assert_eq!(calculate_mapq(0, true, 2), 255);
    }

    #[test]
    fn test_format_unpair_sam_mapped() {
        let read = make_test_read_a();
        let hit = make_test_hit_a();
        let coll = make_test_collection();
        let config = AlignConfig::default();

        let mut buf = String::new();
        format_unpair_sam(
            &mut buf,
            &read,
            Some(&hit),
            Some(0),
            Some(200),
            true,
            &coll,
            &config,
            true,
            1,
            true,
        );

        let fields: Vec<&str> = buf.split('\t').collect();
        assert_eq!(fields[0], "read1"); // QNAME (strip_r_suffix 去除 /1)
        assert!(fields[1].parse::<u16>().unwrap() & 0x40 != 0); // FLAG & 0x40 (first)
        assert_eq!(fields[2], "chr1"); // RNAME
        assert_eq!(fields[3], "101"); // POS (1-based)
        assert_eq!(fields[4], "255"); // MAPQ
        assert_eq!(fields[5], "16M"); // CIGAR
        assert!(!fields[10].is_empty()); // SEQ 不为空
    }

    #[test]
    fn test_format_unpair_sam_unmapped() {
        let read = make_test_read_a();

        let mut buf = String::new();
        format_unpair_sam(
            &mut buf,
            &read,
            None,
            None,
            None,
            true,
            &make_test_collection(),
            &AlignConfig::default(),
            false,
            0,
            false,
        );

        let fields: Vec<&str> = buf.split('\t').collect();
        assert_eq!(fields[0], "read1"); // QNAME (strip_r_suffix 去除 /1)
        let flag = fields[1].parse::<u16>().unwrap();
        assert!(flag & 0x4 != 0); // FLAG & 0x4 (unmapped)
        assert!(flag & 0x40 != 0); // FLAG & 0x40 (first)
        assert_eq!(fields[2], "*"); // RNAME
        assert_eq!(fields[3], "0"); // POS
    }

    #[test]
    fn test_format_pair_sam() {
        let read_a = make_test_read_a();
        let read_b = make_test_read_b();
        let pair_hit = make_test_pair_hit();
        let coll = make_test_collection();
        let config = AlignConfig::default();

        let mut buf_a = String::new();
        let mut buf_b = String::new();
        format_pair_sam(
            &mut buf_a, &mut buf_b,
            &read_a,
            &read_b,
            &pair_hit,
            &coll,
            &config,
            true,
            1,
        );

        // 验证 read_a 的 SAM
        let fields_a: Vec<&str> = buf_a.split('\t').collect();
        assert_eq!(fields_a[0], "read1"); // QNAME (strip_r_suffix 去除 /1)
        let flag_a = fields_a[1].parse::<u16>().unwrap();
        assert!(flag_a & 0x1 != 0);  // paired
        assert!(flag_a & 0x2 != 0);  // proper pair
        assert!(flag_a & 0x40 != 0); // first
        assert_eq!(fields_a[2], "chr1");
        assert_eq!(fields_a[3], "101"); // hit_a.loc + 1

        // 验证 read_b 的 SAM
        let fields_b: Vec<&str> = buf_b.split('\t').collect();
        assert_eq!(fields_b[0], "read1"); // QNAME (strip_r_suffix 去除 /2)
        let flag_b = fields_b[1].parse::<u16>().unwrap();
        assert!(flag_b & 0x1 != 0);  // paired
        assert!(flag_b & 0x2 != 0);  // proper pair
        assert!(flag_b & 0x80 != 0); // last
        assert_eq!(fields_b[2], "chr1");
        assert_eq!(fields_b[3], "201"); // hit_b.loc + 1

        // 验证 TLEN
        let tlen_a: i32 = fields_a[8].parse().unwrap();
        let tlen_b: i32 = fields_b[8].parse().unwrap();
        assert_eq!(tlen_a, 116); // insert size
        assert_eq!(tlen_b, -116); // negative for second read
    }

    fn make_test_collection() -> BinSeqCollection {
        use crate::reference::fasta::Reference;

        let refs = vec![Reference {
            name: "chr1".to_string(),
            seq: b"ACGTACGTACGTACGTACGTACGTACGTACGT".repeat(100).to_vec(),
            len: 3200,
        }];

        BinSeqCollection::from_references(&refs)
    }
}
