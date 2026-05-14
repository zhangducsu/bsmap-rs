//! BSP to SAM converter.
//!
//! Converts BSMAP BSP format (11-column, tab-separated) to standard SAM format.
//! Compatible with the original Python bsp2sam.py behavior.

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
        let name = std::str::from_utf8(record.id())
            .context("Invalid UTF-8 in reference sequence name")?;
        let len = record.num_bases();
        header.push_str(&format!("@SQ\tSN:{}\tLN:{}\n", name, len));
    }

    // @PG line
    header.push_str("@PG\tID:bsmap-rs\n");

    Ok(header)
}

/// Convert a single BSP line (11 columns) to a SAM line.
///
/// BSP format (11 columns, tab-separated):
///   col[0]: id (read name)
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

    match map_flag {
        "NM" => {
            // Unmapped: FLAG=4
            format!(
                "{}\t4\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                name, seq, qual
            )
        }
        "QC" => {
            // QC failed: FLAG=512
            format!(
                "{}\t512\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                name, seq, qual
            )
        }
        _ => {
            // Mapped reads (UM/MA/OF)
            if cols.len() < 11 {
                // Insufficient columns, output as unmapped
                format!(
                    "{}\t4\t*\t0\t0\t*\t*\t0\t0\t{}\t{}",
                    name, seq, qual
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

                // Build SAM flag string (original Python behavior)
                // Note: original bsp2sam.py uses character-based flags, not numeric
                let mut samflag = String::new();

                // Reverse strand: +- or -+
                if strand == "+-" || strand == "-+" {
                    samflag.push('r');
                }

                // Secondary alignment: MA or OF
                if map_flag == "MA" || map_flag == "OF" {
                    samflag.push('s');
                }

                // Properly paired flag
                let has_pair = ins_size > 0;
                if has_pair {
                    samflag.push('P');
                }

                let readlen = seq.len();
                let cigar = format!("{}M", readlen);

                if has_pair {
                    // Paired output
                    format!(
                        "{}\t{}\t{}\t{}\t255\t{}\t=\t0\t{}\t{}\t{}\tNM:i:{}\tZS:Z:{}",
                        name, samflag, cr, pos, cigar, ins_size, seq, qual, mm, strand
                    )
                } else {
                    // Single-end output
                    format!(
                        "{}\t{}\t{}\t{}\t255\t{}\t*\t0\t0\t{}\t{}\tNM:i:{}\tZS:Z:{}",
                        name, samflag, cr, pos, cigar, seq, qual, mm, strand
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
    fn test_convert_nm() {
        let bsp = "read1\tACGT\t!!!!\tNM\t*\t0\t*\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(sam, "read1\t4\t*\t0\t0\t*\t*\t0\t0\tACGT\t!!!!");
    }

    #[test]
    fn test_convert_qc() {
        let bsp = "read1\tACGT\t!!!!\tQC\t*\t0\t*\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(sam, "read1\t512\t*\t0\t0\t*\t*\t0\t0\tACGT\t!!!!");
    }

    #[test]
    fn test_convert_um_plus_plus() {
        let bsp = "read1\tACGTACGT\t!!!!!!!!\tUM\tchr1\t100\t++\t0\t*\t2\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\t\tchr1\t100\t255\t8M\t*\t0\t0\tACGTACGT\t!!!!!!!!\tNM:i:2\tZS:Z:++"
        );
    }

    #[test]
    fn test_convert_ma_plus_minus() {
        let bsp = "read1\tACGT\t!!!!\tMA\tchr1\t200\t+-\t0\t*\t1\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\trs\tchr1\t200\t255\t4M\t*\t0\t0\tACGT\t!!!!\tNM:i:1\tZS:Z:+-"
        );
    }

    #[test]
    fn test_convert_paired() {
        let bsp = "read1\tACGT\t!!!!\tUM\tchr1\t100\t-+\t150\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\trP\tchr1\t100\t255\t4M\t=\t0\t150\tACGT\t!!!!\tNM:i:0\tZS:Z:-+"
        );
    }

    #[test]
    fn test_convert_with_gap() {
        // mm_info format: "1:2:8" means 1 mismatch, gap_size=2, gap_pos=8
        let bsp = "read1\tACGTACGT\t!!!!!!!!\tUM\tchr1\t100\t++\t0\t*\t1:2:8\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\t\tchr1\t100\t255\t8M\t*\t0\t0\tACGTACGT\t!!!!!!!!\tNM:i:1\tZS:Z:++"
        );
    }

    #[test]
    fn test_convert_of_minus_plus() {
        let bsp = "read1\tACGT\t!!!!\tOF\tchr2\t50\t-+\t0\t*\t0\t0";
        let sam = convert_bsp_line(bsp);
        assert_eq!(
            sam,
            "read1\trs\tchr2\t50\t255\t4M\t*\t0\t0\tACGT\t!!!!\tNM:i:0\tZS:Z:-+"
        );
    }
}
