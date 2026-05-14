//! BSP to SAM converter.
//!
//! Converts BSMAP BSP format (11-column, tab-separated) to standard SAM format.
//! Outputs numeric SAM FLAGs compatible with samtools and downstream tools.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use anyhow::{Context, Result};
use clap::Parser;
use log::info;
use needletail::parse_fastx_file;

/// Convert BSMAP BSP format to SAM format.
#[derive(Parser, Debug)]
#[command(name = "bsp2sam", version, about)]
struct Args {
    /// Reference genome FASTA file (required for @SQ header).
    #[arg(short, long)]
    ref_file: String,

    /// Output SAM file.
    #[arg(short, long)]
    out: String,

    /// Suppress progress messages.
    #[arg(short, long)]
    quiet: bool,

    /// BSP input file.
    input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.quiet {
        "error"
    } else {
        "info"
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    info!("Reading reference genome: {}", args.ref_file);

    // Build SAM header from reference FASTA
    let header = build_sam_header(&args.ref_file)?;

    // Open input BSP file
    let input_file = File::open(&args.input)
        .with_context(|| format!("Failed to open BSP input file: {}", args.input))?;
    let reader = BufReader::new(input_file);

    // Open output SAM file
    let output_file = File::create(&args.out)
        .with_context(|| format!("Failed to create output file: {}", args.out))?;
    let mut writer = BufWriter::new(output_file);

    // Write header
    write!(writer, "{}", header)?;

    info!("Converting BSP to SAM: {} -> {}", args.input, args.out);

    // Process each line
    let mut line_count: u64 = 0;
    for line_result in reader.lines() {
        let line = line_result?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let sam_line = convert_bsp_line(line);
        writeln!(writer, "{}", sam_line)?;

        line_count += 1;
        if !args.quiet && line_count % 1_000_000 == 0 {
            info!("Processed {} records", line_count);
        }
    }

    writer.flush()?;
    info!("Done. Total records: {}", line_count);

    Ok(())
}

/// Build SAM header from reference FASTA file.
fn build_sam_header(ref_file: &str) -> Result<String> {
    let mut header = String::new();

    // @HD line
    header.push_str("@HD\tVN:1.0\n");

    // Read reference sequences and generate @SQ lines
    let mut fastx_reader = parse_fastx_file(ref_file)
        .with_context(|| format!("Failed to open reference: {}", ref_file))?;

    while let Some(record) = fastx_reader.next() {
        let record = record?;
        let full_id = std::str::from_utf8(record.id())
            .context("Invalid UTF-8 in reference sequence name")?;
        // needletail may return the full FASTA header line; take only the first word as SN
        let name = full_id.split_whitespace().next().unwrap_or(full_id);
        let len = record.num_bases();
        header.push_str(&format!("@SQ\tSN:{}\tLN:{}\n", name, len));
    }

    // @PG line
    header.push_str("@PG\tID:bsmap-rs\n");

    Ok(header)
}

/// Build a numeric SAM FLAG value.
///
/// # Arguments
/// - `is_paired`: 0x1 - template has multiple segments
/// - `is_proper`: 0x2 - each segment properly aligned
/// - `is_unmapped`: 0x4 - segment unmapped
/// - `mate_unmapped`: 0x8 - next segment unmapped
/// - `is_reverse`: 0x10 - SEQ is reverse complemented
/// - `mate_reverse`: 0x20 - next SEQ is reverse complemented
/// - `is_first`: 0x40 - first segment in template
/// - `is_last`: 0x80 - last segment in template
/// - `is_secondary`: 0x100 - secondary alignment
fn build_flag(
    is_paired: bool,
    is_proper: bool,
    is_unmapped: bool,
    mate_unmapped: bool,
    is_reverse: bool,
    mate_reverse: bool,
    is_first: bool,
    is_last: bool,
    is_secondary: bool,
) -> u16 {
    let mut flag: u16 = 0;
    if is_paired {
        flag |= 0x1;
    }
    if is_proper {
        flag |= 0x2;
    }
    if is_unmapped {
        flag |= 0x4;
    }
    if mate_unmapped {
        flag |= 0x8;
    }
    if is_reverse {
        flag |= 0x10;
    }
    if mate_reverse {
        flag |= 0x20;
    }
    if is_first {
        flag |= 0x40;
    }
    if is_last {
        flag |= 0x80;
    }
    if is_secondary {
        flag |= 0x100;
    }
    flag
}

/// Convert a single BSP line (11 columns) to a SAM line.
///
/// BSP format (11 columns, tab-separated):
///   col[0]: id (read name, may end with _R1/_R2)
///   col[1]: seq (mapped read sequence)
///   col[2]: qual (quality scores)
///   col[3]: map_flag (UM/MA/OF/NM/QC)
///   col[4]: ref (reference name)
///   col[5]: ref_loc (1-based position)
///   col[6]: strand (++, +-, -+, --)
///   col[7]: ins_size (insert size)
///   col[8]: refseq (Watson strand reference, unused)
///   col[9]: mm_info (mismatch info)
///   col[10]: mismatch_info (hit count distribution)
fn convert_bsp_line(line: &str) -> String {
    let cols: Vec<&str> = line.split('\t').collect();

    // Need at least 4 columns for minimal processing
    if cols.len() < 4 {
        return line.to_string();
    }

    let name = cols[0];
    let seq = cols[1];
    let qual = cols[2];
    let map_flag = cols[3];

    // Detect paired-end from read name suffix (_R1/_R2)
    let is_paired = name.ends_with("_R1") || name.ends_with("_R2");
    let is_first = name.ends_with("_R1");
    let is_last = name.ends_with("_R2");

    match map_flag {
        "NM" => {
            // Unmapped
            let flag = build_flag(is_paired, false, true, is_paired, false, false, is_first, is_last, false);
            format!(
                "{}\t{}\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                name, flag, seq, qual
            )
        }
        "QC" => {
            // QC failed (treat as unmapped + 0x200)
            let flag = build_flag(is_paired, false, true, is_paired, false, false, is_first, is_last, false) | 512;
            format!(
                "{}\t{}\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                name, flag, seq, qual
            )
        }
        _ => {
            // Mapped reads (UM/MA/OF)
            if cols.len() < 11 {
                // Insufficient columns, output as unmapped
                let flag = build_flag(is_paired, false, true, is_paired, false, false, is_first, is_last, false);
                format!(
                    "{}\t{}\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                    name, flag, seq, qual
                )
            } else {
                let cr = cols[4];      // reference name
                let pos = cols[5];     // 1-based position
                let strand = cols[6];  // ++, +-, -+, --
                let ins_size: i64 = cols[7].parse().unwrap_or(0);
                let mm_info = cols[9]; // mismatch info

                // Parse mismatch count from mm_info
                // mm_info format: "#mismatches" or "#mm:#gap_size:#gap_pos"
                let mm: u32 = mm_info
                    .split(':')
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                // Determine strand flags from ZS strand tag
                // strand: ++ => read forward, ref forward
                // strand: +- => read reverse, ref forward
                // strand: -+ => read forward, ref reverse
                // strand: -- => read reverse, ref reverse
                let is_reverse = strand == "+-" || strand == "--";
                // Mate strand is opposite of read strand for paired-end
                let mate_is_reverse = is_paired && !is_reverse;

                // Secondary alignment: MA or OF
                let is_secondary = map_flag == "MA" || map_flag == "OF";

                // Proper pair: paired with positive insert size
                let is_proper = is_paired && ins_size > 0;

                let readlen = seq.len();
                let cigar = format!("{}M", readlen);

                // Build numeric SAM FLAG
                let flag = build_flag(
                    is_paired,
                    is_proper,
                    false,             // is_unmapped
                    false,             // mate_unmapped (assume mate mapped for paired)
                    is_reverse,
                    mate_is_reverse,
                    is_first,
                    is_last,
                    is_secondary,
                );

                if is_paired {
                    // Paired output
                    let tlen = if ins_size > 0 {
                        if is_reverse {
                            -(ins_size as i32)
                        } else {
                            ins_size as i32
                        }
                    } else {
                        0
                    };
                    format!(
                        "{}\t{}\t{}\t{}\t255\t{}\t=\t0\t{}\t{}\t{}\tNM:i:{}\tZS:Z:{}",
                        name, flag, cr, pos, cigar, tlen, seq, qual, mm, strand
                    )
                } else {
                    // Single-end output
                    format!(
                        "{}\t{}\t{}\t{}\t255\t{}\t*\t0\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
                        name, flag, cr, pos, cigar, seq, qual, mm, strand
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_flag_paired_first_forward() {
        // R1, forward, mate reverse => 0x1 | 0x2 | 0x20 | 0x40 = 1+2+32+64 = 99
        let flag = build_flag(true, true, false, false, false, true, true, false, false);
        assert_eq!(flag, 99);
    }

    #[test]
    fn test_build_flag_paired_last_reverse() {
        // R2, reverse, mate forward => 0x1 | 0x2 | 0x10 | 0x80 = 1+2+16+128 = 147
        let flag = build_flag(true, true, false, false, true, false, false, true, false);
        assert_eq!(flag, 147);
    }

    #[test]
    fn test_build_flag_paired_first_reverse() {
        // R1, reverse, mate forward => 0x1 | 0x2 | 0x10 | 0x40 = 1+2+16+64 = 83
        let flag = build_flag(true, true, false, false, true, false, true, false, false);
        assert_eq!(flag, 83);
    }

    #[test]
    fn test_build_flag_paired_last_forward() {
        // R2, forward, mate reverse => 0x1 | 0x2 | 0x20 | 0x80 = 1+2+32+128 = 163
        let flag = build_flag(true, true, false, false, false, true, false, true, false);
        assert_eq!(flag, 163);
    }

    #[test]
    fn test_convert_nm() {
        let bsp = "read1\tACGT\t!!!!\tNM\t*\t0\t*\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(sam, "read1\t4\t*\t0\t0\t*\t*\t0\t0\tACGT\t!!!!");
    }

    #[test]
    fn test_convert_qc() {
        let bsp = "read1\tACGT\t!!!!\tQC\t*\t0\t*\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(sam, "read1\t516\t*\t0\t0\t*\t*\t0\t0\tACGT\t!!!!");
    }

    #[test]
    fn test_convert_paired_r1_strand_minus_plus() {
        // R1 with strand -+ => read forward, mate reverse => FLAG=97 (no proper pair since ins_size=0)
        // 0x1 | 0x20 | 0x40 = 1+32+64 = 97
        let bsp = "read1_R1\tACGT\t!!!!\tUM\tchr1\t100\t-+\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1_R1\t97\tchr1\t100\t255\t4M\t=\t0\t0\tACGT\t!!!!\tNM:i:0\tZS:Z:-+"
        );
    }

    #[test]
    fn test_convert_paired_r2_strand_minus_minus() {
        // R2 with strand -- => read reverse, mate forward => FLAG=145 (no proper pair since ins_size=0)
        // 0x1 | 0x10 | 0x80 = 1+16+128 = 145
        let bsp = "read1_R2\tACGT\t!!!!\tUM\tchr1\t200\t--\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1_R2\t145\tchr1\t200\t255\t4M\t=\t0\t0\tACGT\t!!!!\tNM:i:0\tZS:Z:--"
        );
    }

    #[test]
    fn test_convert_paired_r1_strand_plus_plus() {
        // R1 with strand ++ => read forward, mate reverse => FLAG=97 (no proper pair since ins_size=0)
        // 0x1 | 0x20 | 0x40 = 1+32+64 = 97
        let bsp = "read1_R1\tACGT\t!!!!\tUM\tchr1\t100\t++\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1_R1\t97\tchr1\t100\t255\t4M\t=\t0\t0\tACGT\t!!!!\tNM:i:0\tZS:Z:++"
        );
    }

    #[test]
    fn test_convert_paired_r2_strand_plus_minus() {
        // R2 with strand +- => read reverse, mate forward => FLAG=145 (no proper pair since ins_size=0)
        // 0x1 | 0x10 | 0x80 = 1+16+128 = 145
        let bsp = "read1_R2\tACGT\t!!!!\tUM\tchr1\t200\t+-\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1_R2\t145\tchr1\t200\t255\t4M\t=\t0\t0\tACGT\t!!!!\tNM:i:0\tZS:Z:+-"
        );
    }

    #[test]
    fn test_convert_single_end() {
        let bsp = "read1\tACGTACGT\t!!!!!!!!\tUM\tchr1\t100\t++\t0\t*\t2\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\t0\tchr1\t100\t255\t8M\t*\t0\t0\tACGTACGT\t!!!!!!!!\tNM:i:2\tZS:Z:++"
        );
    }

    #[test]
    fn test_convert_secondary() {
        // MA with _R1 and strand +- => is_reverse=true, mate_is_reverse=false, is_secondary=true
        // 0x1 | 0x10 | 0x40 | 0x100 = 1+16+64+256 = 337
        let bsp = "read1_R1\tACGT\t!!!!\tMA\tchr1\t200\t+-\t0\t*\t1\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1_R1\t337\tchr1\t200\t255\t4M\t=\t0\t0\tACGT\t!!!!\tNM:i:1\tZS:Z:+-"
        );
    }

    #[test]
    fn test_convert_paired_with_insert_size() {
        // R1 with strand -+ and insert size => FLAG=99 (proper pair), TLEN negative for reverse
        let bsp = "read1_R1\tACGT\t!!!!\tUM\tchr1\t100\t-+\t300\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        // -+ => read forward (is_reverse=false), so TLEN=300
        assert_eq!(
            sam,
            "read1_R1\t99\tchr1\t100\t255\t4M\t=\t0\t300\tACGT\t!!!!\tNM:i:0\tZS:Z:-+"
        );
    }

    #[test]
    fn test_convert_paired_r2_with_insert_size() {
        // R2 with strand -- and insert size => FLAG=163 (proper pair), TLEN negative for reverse
        // 0x1 | 0x2 | 0x10 | 0x80 = 1+2+16+128 = 147... wait
        // -- => is_reverse=true, mate_is_reverse=false
        // R2 => is_first=false, is_last=true
        // ins_size=300 => is_proper=true
        // 0x1 | 0x2 | 0x10 | 0x80 = 1+2+16+128 = 147
        let bsp = "read1_R2\tACGT\t!!!!\tUM\tchr1\t200\t--\t300\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        // -- => read reverse (is_reverse=true), so TLEN=-300
        assert_eq!(
            sam,
            "read1_R2\t147\tchr1\t200\t255\t4M\t=\t0\t-300\tACGT\t!!!!\tNM:i:0\tZS:Z:--"
        );
    }
}
