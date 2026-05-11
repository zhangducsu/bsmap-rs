//! Command-line interface for BSMAP-rs.
//!
//! Uses `clap` derive macros to replicate all 30+ options from the
//! original C++ `mGetOptions()` in `main.cpp`.

use std::path::PathBuf;

use clap::Parser;

/// BSMAP-rs: Bisulfite Sequence MAPping — ultra-fast BS-seq aligner.
///
/// Maps bisulfite-treated sequencing reads to a reference genome,
/// handling the asymmetric C→T conversion. Supports WGBS and RRBS
/// modes, single-end and paired-end reads, FASTA/FASTQ/SAM/BAM input,
/// and SAM/BSP output.
#[derive(Parser, Debug)]
#[command(
    name = "bsmap",
    version = env!("CARGO_PKG_VERSION"),
    about = "Bisulfite Sequence MAPping in Rust",
    long_about = None,
)]
pub struct Cli {
    // ── Required / Core ──────────────────────────────────────────────────
    /// Query reads file (FASTA/FASTQ/BAM, plain or gzipped). Required.
    #[arg(short = 'a', value_name = "FILE")]
    pub query_a: PathBuf,

    /// Query reads file b for paired-end data (FASTA/FASTQ/BAM).
    #[arg(short = 'b', value_name = "FILE")]
    pub query_b: Option<PathBuf>,

    /// Reference genome FASTA file (plain or gzipped). Required.
    #[arg(short = 'd', value_name = "FILE")]
    pub reference: PathBuf,

    /// Output alignment file. Suffix determines format:
    /// .sam → SAM, .bam → sorted BAM, otherwise → BSP.
    /// Omit to write SAM to STDOUT.
    #[arg(short = 'o', value_name = "FILE")]
    pub output: Option<PathBuf>,

    // ── Alignment Options ────────────────────────────────────────────────
    /// Seed size (k-mer length), 10-16. Default: 16 (WGBS), 12 (RRBS).
    #[arg(short = 's', value_name = "INT", default_value = "16")]
    pub seed_size: u32,

    /// Max mismatches. If 0 < v < 1, interpreted as rate × read_length.
    /// Otherwise absolute count ≤ 15.
    /// Example: -v 5 = max 5 mismatches, -v 0.1 = max read_length*10%.
    #[arg(short = 'v', value_name = "FLOAT", default_value = "0.08")]
    pub max_mismatch: f64,

    /// Max continuous gap size (insertion/deletion), 0-3. Default: 0.
    #[arg(short = 'g', value_name = "INT", default_value = "0")]
    pub gap_size: u32,

    /// Max number of equal-best hits to count. Default: 100.
    #[arg(short = 'w', value_name = "INT", default_value = "100")]
    pub max_hits: u32,

    /// Use 3-nucleotide mapping (C+T share same code). Default: off.
    #[arg(long = "nt3", default_value_t = false)]
    pub nt3: bool,

    // ── Read Processing ──────────────────────────────────────────────────
    /// Start from the Nth read or read pair. Default: 1.
    #[arg(short = 'B', value_name = "INT", default_value = "1")]
    pub read_start: u32,

    /// End at the Nth read or read pair.
    #[arg(short = 'E', value_name = "INT")]
    pub read_end: Option<u32>,

    /// Index interval (1-16): genome indexed every N bp. Default: 4.
    /// For RRBS mode, fixed to 1.
    #[arg(short = 'I', value_name = "INT", default_value = "4")]
    pub index_interval: u32,

    /// Cut-off ratio for over-represented k-mers.
    /// Top ratio of k-mers will be skipped. Default: 5e-7.
    #[arg(short = 'k', value_name = "FLOAT", default_value = "5e-7")]
    pub kmer_cutoff: f64,

    /// Quality threshold for 3'-end trimming, 0-40. Default: 0 (no trim).
    #[arg(short = 'q', value_name = "INT", default_value = "0")]
    pub qual_threshold: u8,

    /// Base quality offset: 33 (Sanger) or 64 (Illumina). Default: 33.
    #[arg(short = 'z', value_name = "INT", default_value = "33")]
    pub zero_qual: u8,

    /// Filter reads with > N 'N's. Default: 5.
    #[arg(short = 'f', value_name = "INT", default_value = "5")]
    pub max_ns: u32,

    /// 3'-end adapter sequence(s) to trim. Repeatable (-A seq1 -A seq2).
    #[arg(short = 'A', value_name = "SEQ")]
    pub adapters: Vec<String>,

    /// Map only the first N nucleotides of each read.
    #[arg(short = 'L', value_name = "INT")]
    pub max_read_len: Option<u32>,

    // ── Reporting ────────────────────────────────────────────────────────
    /// How to report repeat hits: 0=unique only, 1=random one, 2=all.
    #[arg(short = 'r', value_name = "INT", default_value = "1")]
    pub report_repeat: u8,

    /// Include reference sequence in SAM XR:Z field.
    #[arg(short = 'R', default_value_t = false)]
    pub out_ref: bool,

    /// Report unmapped reads.
    #[arg(short = 'u', default_value_t = false)]
    pub out_unmap: bool,

    /// Suppress SAM header.
    #[arg(short = 'H', default_value_t = false)]
    pub no_header: bool,

    // ── Paired-end ───────────────────────────────────────────────────────
    /// Minimum insert size for paired-end. Default: 28.
    #[arg(short = 'm', value_name = "INT", default_value = "28")]
    pub min_insert: u32,

    /// Maximum insert size for paired-end. Default: 1000.
    #[arg(short = 'x', value_name = "INT", default_value = "1000")]
    pub max_insert: u32,

    // ── Strand ───────────────────────────────────────────────────────────
    /// Mapping strand: 0=forward only (++, -+), 1=all 4 strands.
    #[arg(short = 'n', value_name = "INT", default_value = "0")]
    pub chains: u8,

    /// Additional nucleotide transition: N1N2 means N1 in reads → N2 in ref.
    /// Default: TC (bisulfite C→U/T).
    #[arg(short = 'M', value_name = "NT", default_value = "TC")]
    pub align_transition: String,

    // ── RRBS Mode ────────────────────────────────────────────────────────
    /// RRBS restriction enzyme digestion site(s). Mark cut with '-'.
    /// Example: -D C-CGG for MspI. Repeatable for multiple enzymes.
    #[arg(short = 'D', value_name = "SITE")]
    pub digestion_sites: Vec<String>,

    // ── Parallelism ──────────────────────────────────────────────────────
    /// Number of threads. Default: CPU count (max 8).
    #[arg(short = 'p', value_name = "INT")]
    pub num_threads: Option<usize>,

    // ── Misc ─────────────────────────────────────────────────────────────
    /// Random seed for reproducible mapping. 0 = system clock.
    #[arg(short = 'S', value_name = "INT", default_value = "0")]
    pub randseed: u32,

    /// Verbose level: 0=quiet, 1=normal, 2=detailed.
    #[arg(short = 'V', value_name = "INT", default_value = "1")]
    pub verbose: u8,
}

impl Cli {
    /// Validate CLI options and return a list of errors (if any).
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.seed_size < 10 || self.seed_size > 16 {
            errors.push(format!(
                "seed size must be 10-16, got {}",
                self.seed_size
            ));
        }

        if self.gap_size > 3 {
            errors.push(format!(
                "gap size must be 0-3, got {}",
                self.gap_size
            ));
        }

        if self.report_repeat > 2 {
            errors.push(format!(
                "report repeat (-r) must be 0, 1, or 2, got {}",
                self.report_repeat
            ));
        }

        if self.verbose > 2 {
            errors.push(format!(
                "verbose level (-V) must be 0, 1, or 2, got {}",
                self.verbose
            ));
        }

        if self.index_interval > 16 {
            errors.push(format!(
                "index interval (-I) must be ≤ 16, got {}",
                self.index_interval
            ));
        }

        if self.index_interval == 0 {
            errors.push("index interval (-I) must be ≥ 1".to_string());
        }

        if self.chains > 1 {
            errors.push(format!(
                "strand flag (-n) must be 0 or 1, got {}",
                self.chains
            ));
        }

        if !self.align_transition.is_empty() && self.align_transition.len() == 2 {
            let bytes = self.align_transition.as_bytes();
            if bytes[0] == bytes[1] {
                errors.push(format!(
                    "alignment transition (-M) must specify different nucleotides, got {}",
                    self.align_transition
                ));
            }
        }

        // File existence checks: done at runtime in main.rs
        // (skipped here so validate() can be unit-tested without real files)

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let cli = Cli::try_parse_from([
            "bsmap",
            "-a", "reads.fq",
            "-d", "ref.fa",
        ]).expect("valid args should parse");
        assert_eq!(cli.seed_size, 16);
        assert_eq!(cli.max_hits, 100);
        assert_eq!(cli.gap_size, 0);
        assert_eq!(cli.report_repeat, 1);
        assert_eq!(cli.verbose, 1);
    }

    #[test]
    fn test_validation() {
        let cli = Cli::try_parse_from([
            "bsmap",
            "-a", "reads.fq",
            "-d", "ref.fa",
            "-s", "20",  // invalid seed size
            "-g", "5",   // invalid gap
        ]).expect("should parse even with invalid values");
        let errors = cli.validate();
        assert!(!errors.is_empty());
    }
}
