//! Command-line interface for BSMAP-rs.
//!
//! Uses `clap` derive macros with subcommands to support both
//! `bsmap align` (mapping) and `bsmap index` (index building).

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// BSMAP-rs: Bisulfite Sequence MAPping — ultra-fast BS-seq aligner.
///
/// Maps bisulfite-treated sequencing reads to a reference genome,
/// handling the asymmetric C→T conversion. Supports WGBS and RRBS
/// modes, single-end and paired-end reads, FASTA/FASTQ/SAM/BAM input,
/// and SAM/BSP output.
///
/// # 子命令
///
/// - `bsmap index -d ref.fa`: 构建参考序列索引
/// - `bsmap align -a reads.fq -d ref.fa -o out.sam`: 比对读段
/// - `bsmap -a reads.fq -d ref.fa -o out.sam`: 等价于 `bsmap align`（向后兼容）
#[derive(Parser, Debug)]
#[command(
    name = "bsmap",
    version = env!("CARGO_PKG_VERSION"),
    about = "Bisulfite Sequence MAPping in Rust",
    long_about = None,
)]
pub struct Cli {
    /// 子命令。如果不指定，默认为 `align`（向后兼容原版 BSMAP 用法）。
    #[command(subcommand)]
    pub command: Option<Commands>,

    // ── 以下参数在无子命令时用于 align（向后兼容） ─────────────────────
    /// Query reads file (FASTA/FASTQ/BAM, plain or gzipped).
    #[arg(short = 'a', value_name = "FILE", global = true)]
    pub query_a: Option<PathBuf>,

    /// Query reads file b for paired-end data (FASTA/FASTQ/BAM).
    #[arg(short = 'b', value_name = "FILE", global = true)]
    pub query_b: Option<PathBuf>,

    /// Reference genome FASTA file (plain or gzipped).
    #[arg(short = 'd', value_name = "FILE", global = true)]
    pub reference: Option<PathBuf>,

    /// Output alignment file. Suffix determines format:
    /// .sam → SAM, .bam → sorted BAM, otherwise → BSP.
    /// Omit to write SAM to STDOUT.
    #[arg(short = 'o', value_name = "FILE", global = true)]
    pub output: Option<PathBuf>,

    /// Verbose level: 0=quiet, 1=normal, 2=detailed.
    #[arg(long = "verbose", value_name = "INT", default_value = "1", global = true)]
    pub verbose: u8,
}

/// 可用子命令。
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 构建参考序列索引。
    ///
    /// 构建完成后索引保存为 `.bsi` 文件，后续比对时自动加载。
    /// 如果索引文件已存在且兼容，将跳过构建。
    ///
    /// # 示例
    ///
    /// ```bash
    /// bsmap index -d ref.fa
    /// bsmap index -d ref.fa -s 12 -D C-CGG   # RRBS 模式
    /// ```
    Index {
        /// Reference genome FASTA file (plain or gzipped).
        #[arg(short = 'd', value_name = "FILE")]
        reference: PathBuf,

        /// Seed size (k-mer length), 10-16. Default: 16 (WGBS), 12 (RRBS).
        #[arg(short = 's', value_name = "INT", default_value = "16")]
        seed_size: u32,

        /// Index interval (1-16): genome indexed every N bp. Default: 4.
        #[arg(short = 'I', value_name = "INT", default_value = "4")]
        index_interval: u32,

        /// Cut-off ratio for over-represented k-mers.
        #[arg(short = 'k', value_name = "FLOAT", default_value = "5e-7")]
        kmer_cutoff: f64,

        /// RRBS restriction enzyme digestion site(s). Mark cut with '-'.
        /// Example: -D C-CGG for MspI. Repeatable for multiple enzymes.
        #[arg(short = 'D', value_name = "SITE")]
        digestion_sites: Vec<String>,

        /// Minimum insert size for RRBS. Default: 28.
        #[arg(short = 'm', value_name = "INT", default_value = "28")]
        min_insert: u32,

        /// Maximum insert size for RRBS. Default: 1000.
        #[arg(short = 'x', value_name = "INT", default_value = "1000")]
        max_insert: u32,

        /// Verbose level: 0=quiet, 1=normal, 2=detailed.
        #[arg(long = "verbose", value_name = "INT", default_value = "1")]
        verbose: u8,
    },

    /// 比对读段到参考序列（默认子命令）。
    ///
    /// # 示例
    ///
    /// ```bash
    /// bsmap align -a reads.fq -d ref.fa -o out.sam
    /// bsmap align -a reads_1.fq -b reads_2.fq -d ref.fa -o out.bam
    /// ```
    Align {
        /// Query reads file (FASTA/FASTQ/BAM, plain or gzipped).
        #[arg(short = 'a', value_name = "FILE")]
        query_a: PathBuf,

        /// Query reads file b for paired-end data (FASTA/FASTQ/BAM).
        #[arg(short = 'b', value_name = "FILE")]
        query_b: Option<PathBuf>,

        /// Reference genome FASTA file (plain or gzipped).
        #[arg(short = 'd', value_name = "FILE")]
        reference: PathBuf,

        /// Output alignment file. Suffix determines format.
        #[arg(short = 'o', value_name = "FILE")]
        output: Option<PathBuf>,

        /// Seed size (k-mer length), 10-16. Default: 16.
        #[arg(short = 's', value_name = "INT", default_value = "16")]
        seed_size: u32,

        /// Max mismatches. 0 < v < 1 = rate, otherwise absolute count.
        #[arg(short = 'v', value_name = "FLOAT", default_value = "0.08")]
        max_mismatch: f64,

        /// Max continuous gap size, 0-3. Default: 0.
        #[arg(short = 'g', value_name = "INT", default_value = "0")]
        gap_size: u32,

        /// Max number of equal-best hits to count. Default: 100.
        #[arg(short = 'w', value_name = "INT", default_value = "100")]
        max_hits: u32,

        /// Use 3-nucleotide mapping.
        #[arg(long = "nt3", default_value_t = false)]
        nt3: bool,

        /// Start from the Nth read. Default: 1.
        #[arg(short = 'B', value_name = "INT", default_value = "1")]
        read_start: u32,

        /// End at the Nth read.
        #[arg(short = 'E', value_name = "INT")]
        read_end: Option<u32>,

        /// Index interval (1-16). Default: 4.
        #[arg(short = 'I', value_name = "INT", default_value = "4")]
        index_interval: u32,

        /// Cut-off ratio for over-represented k-mers.
        #[arg(short = 'k', value_name = "FLOAT", default_value = "5e-7")]
        kmer_cutoff: f64,

        /// Quality threshold for 3'-end trimming. Default: 0.
        #[arg(short = 'q', value_name = "INT", default_value = "0")]
        qual_threshold: u8,

        /// Base quality offset. Default: 33.
        #[arg(short = 'z', value_name = "INT", default_value = "33")]
        zero_qual: u8,

        /// Filter reads with > N 'N's. Default: 5.
        #[arg(short = 'f', value_name = "INT", default_value = "5")]
        max_ns: u32,

        /// 3'-end adapter sequence(s) to trim.
        #[arg(short = 'A', value_name = "SEQ")]
        adapters: Vec<String>,

        /// Map only the first N nucleotides of each read.
        #[arg(short = 'L', value_name = "INT")]
        max_read_len: Option<u32>,

        /// How to report repeat hits: 0=unique, 1=random, 2=all.
        #[arg(short = 'r', value_name = "INT", default_value = "1")]
        report_repeat: u8,

        /// Include reference sequence in SAM XR:Z field.
        #[arg(short = 'R', default_value_t = false)]
        out_ref: bool,

        /// Report unmapped reads.
        #[arg(short = 'u', default_value_t = false)]
        out_unmap: bool,

        /// Suppress SAM header.
        #[arg(short = 'H', default_value_t = false)]
        no_header: bool,

        /// Minimum insert size for paired-end. Default: 28.
        #[arg(short = 'm', value_name = "INT", default_value = "28")]
        min_insert: u32,

        /// Maximum insert size for paired-end. Default: 1000.
        #[arg(short = 'x', value_name = "INT", default_value = "1000")]
        max_insert: u32,

        /// Set mapping strand information.
        /// -n 0: only map to 2 forward strands, i.e. BSW(++) and BSC(-+) (Lister protocol, default).
        ///       For PE sequencing, map read#1 to ++ and -+, read#2 to +- and --.
        /// -n 1: map SE or PE reads to all 4 strands, i.e. ++, +-, -+, -- (Cokus protocol).
        /// Default: 0. Most bisulfite sequencing data is generated only from forward strands.
        #[arg(short = 'n', value_name = "INT", default_value = "0")]
        chains: u8,

        /// Nucleotide transition. Default: TC.
        #[arg(short = 'M', value_name = "NT", default_value = "TC")]
        align_transition: String,

        /// RRBS digestion site(s).
        #[arg(short = 'D', value_name = "SITE")]
        digestion_sites: Vec<String>,

        /// Number of threads. Default: CPU count (max 8).
        #[arg(short = 'p', value_name = "INT")]
        num_threads: Option<usize>,

        /// Random seed. 0 = system clock.
        #[arg(short = 'S', value_name = "INT", default_value = "0")]
        randseed: u32,

        /// Verbose level: 0=quiet, 1=normal, 2=detailed.
        #[arg(long = "verbose", value_name = "INT", default_value = "1")]
        verbose: u8,
    },
}

/// 从子命令或全局参数中提取比对参数。
///
/// 支持三种调用方式：
/// 1. `bsmap align -a reads.fq -d ref.fa`（显式子命令）
/// 2. `bsmap -a reads.fq -d ref.fa`（向后兼容，无子命令）
/// 3. `bsmap index -d ref.fa`（索引构建）
pub fn resolve_command(cli: &Cli) -> ResolvedCommand {
    match &cli.command {
        Some(Commands::Index { .. }) => {
            // 不需要做任何事，由 main.rs 直接处理
            ResolvedCommand::Index
        }
        Some(Commands::Align {
            query_a,
            query_b,
            reference,
            output,
            seed_size,
            max_mismatch,
            gap_size,
            max_hits,
            nt3,
            read_start,
            read_end,
            index_interval,
            kmer_cutoff,
            qual_threshold,
            zero_qual,
            max_ns,
            adapters,
            max_read_len,
            report_repeat,
            out_ref,
            out_unmap,
            no_header,
            min_insert,
            max_insert,
            chains,
            align_transition,
            digestion_sites,
            num_threads,
            randseed,
            verbose,
        }) => ResolvedCommand::Align(AlignArgs {
            query_a: Some(query_a.clone()),
            query_b: query_b.clone(),
            reference: Some(reference.clone()),
            output: output.clone(),
            seed_size: *seed_size,
            max_mismatch: *max_mismatch,
            gap_size: *gap_size,
            max_hits: *max_hits,
            nt3: *nt3,
            read_start: *read_start,
            read_end: *read_end,
            index_interval: *index_interval,
            kmer_cutoff: *kmer_cutoff,
            qual_threshold: *qual_threshold,
            zero_qual: *zero_qual,
            max_ns: *max_ns,
            adapters: adapters.clone(),
            max_read_len: max_read_len.unwrap_or(0),
            report_repeat: *report_repeat,
            out_ref: *out_ref,
            out_unmap: *out_unmap,
            no_header: *no_header,
            min_insert: *min_insert,
            max_insert: *max_insert,
            chains: *chains,
            align_transition: align_transition.clone(),
            digestion_sites: digestion_sites.clone(),
            num_threads: *num_threads,
            randseed: *randseed,
            verbose: *verbose,
        }),
        None => {
            // 向后兼容：无子命令时，如果提供了 -a 和 -d，视为 align
            if cli.query_a.is_some() && cli.reference.is_some() {
                ResolvedCommand::Align(AlignArgs {
                    query_a: cli.query_a.clone(),
                    query_b: cli.query_b.clone(),
                    reference: cli.reference.clone(),
                    output: cli.output.clone(),
                    seed_size: 16,        // 默认值
                    max_mismatch: 0.08,
                    gap_size: 0,
                    max_hits: 100,
                    nt3: false,
                    read_start: 1,
                    read_end: None,
                    index_interval: 4,
                    kmer_cutoff: 5e-7,
                    qual_threshold: 0,
                    zero_qual: 33,
                    max_ns: 5,
                    adapters: Vec::new(),
                    max_read_len: 0,
                    report_repeat: 1,
                    out_ref: false,
                    out_unmap: false,
                    no_header: false,
                    min_insert: 28,
                    max_insert: 1000,
                    chains: 0,
                    align_transition: "TC".to_string(),
                    digestion_sites: Vec::new(),
                    num_threads: None,
                    randseed: 0,
                    verbose: cli.verbose,
                })
            } else {
                // 没有足够参数，打印帮助信息
                ResolvedCommand::Help
            }
        }
    }
}

/// 解析后的命令。
pub enum ResolvedCommand {
    /// 构建索引。
    Index,
    /// 比对读段。
    Align(AlignArgs),
    /// 打印帮助信息。
    Help,
}

/// 比对参数（从 Align 子命令或全局参数中提取）。
#[derive(Debug, Clone)]
pub struct AlignArgs {
    pub query_a: Option<PathBuf>,
    pub query_b: Option<PathBuf>,
    pub reference: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub seed_size: u32,
    pub max_mismatch: f64,
    pub gap_size: u32,
    pub max_hits: u32,
    pub nt3: bool,
    pub read_start: u32,
    pub read_end: Option<u32>,
    pub index_interval: u32,
    pub kmer_cutoff: f64,
    pub qual_threshold: u8,
    pub zero_qual: u8,
    pub max_ns: u32,
    pub adapters: Vec<String>,
    pub max_read_len: u32,
    pub report_repeat: u8,
    pub out_ref: bool,
    pub out_unmap: bool,
    pub no_header: bool,
    pub min_insert: u32,
    pub max_insert: u32,
    pub chains: u8,
    pub align_transition: String,
    pub digestion_sites: Vec<String>,
    pub num_threads: Option<usize>,
    pub randseed: u32,
    pub verbose: u8,
}

/// 从 Index 子命令中提取索引构建参数。
pub fn resolve_index_args(cli: &Cli) -> Option<IndexArgs> {
    match &cli.command {
        Some(Commands::Index {
            reference,
            seed_size,
            index_interval,
            kmer_cutoff,
            digestion_sites,
            min_insert,
            max_insert,
            verbose,
        }) => Some(IndexArgs {
            reference: reference.clone(),
            seed_size: *seed_size,
            index_interval: *index_interval,
            kmer_cutoff: *kmer_cutoff,
            digestion_sites: digestion_sites.clone(),
            min_insert: *min_insert,
            max_insert: *max_insert,
            verbose: *verbose,
        }),
        _ => None,
    }
}

/// 索引构建参数。
#[derive(Debug, Clone)]
pub struct IndexArgs {
    pub reference: PathBuf,
    pub seed_size: u32,
    pub index_interval: u32,
    pub kmer_cutoff: f64,
    pub digestion_sites: Vec<String>,
    pub min_insert: u32,
    pub max_insert: u32,
    pub verbose: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_align_subcommand() {
        let cli = Cli::try_parse_from([
            "bsmap", "align",
            "-a", "reads.fq",
            "-d", "ref.fa",
        ]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Align { .. })));
    }

    #[test]
    fn test_explicit_index_subcommand() {
        let cli = Cli::try_parse_from([
            "bsmap", "index",
            "-d", "ref.fa",
        ]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Index { .. })));
    }

    #[test]
    fn test_backward_compat_no_subcommand() {
        let cli = Cli::try_parse_from([
            "bsmap",
            "-a", "reads.fq",
            "-d", "ref.fa",
        ]).unwrap();
        assert!(cli.command.is_none());
        assert!(cli.query_a.is_some());
        assert!(cli.reference.is_some());
    }

    #[test]
    fn test_resolve_align_from_subcommand() {
        let cli = Cli::try_parse_from([
            "bsmap", "align",
            "-a", "reads.fq",
            "-d", "ref.fa",
            "-s", "12",
        ]).unwrap();
        match resolve_command(&cli) {
            ResolvedCommand::Align(args) => {
                assert_eq!(args.seed_size, 12);
            }
            _ => panic!("expected Align command"),
        }
    }

    #[test]
    fn test_resolve_align_backward_compat() {
        let cli = Cli::try_parse_from([
            "bsmap",
            "-a", "reads.fq",
            "-d", "ref.fa",
        ]).unwrap();
        match resolve_command(&cli) {
            ResolvedCommand::Align(args) => {
                assert_eq!(args.seed_size, 16);
            }
            _ => panic!("expected Align command"),
        }
    }

    #[test]
    fn test_resolve_index() {
        let cli = Cli::try_parse_from([
            "bsmap", "index",
            "-d", "ref.fa",
            "-s", "12",
            "-D", "C-CGG",
        ]).unwrap();
        let args = resolve_index_args(&cli).unwrap();
        assert_eq!(args.seed_size, 12);
        assert_eq!(args.digestion_sites, vec!["C-CGG"]);
    }
}
