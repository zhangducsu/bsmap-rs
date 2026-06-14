//! Constants, types, and global configuration for BSMAP-rs.
//!
//! Mirrors the C++ `param.h` and `param.cpp` — defines the fundamental
//! data types, alignment parameters, and the `AlignConfig` struct.

use std::sync::atomic::AtomicU32;

use crate::cli::AlignArgs;

// ── Core Constants ──────────────────────────────────────────────────────────

/// Bases per 64-bit word (2 bits per base → 32 bases)
pub const SEGLEN: usize = 32;

/// Number of u64 words per read segment (160/32 + 1 padding)
pub const FIXELEMENT: usize = 6;

/// Total fixed-size array length in 64-bit words
pub const FIXSIZE: usize = SEGLEN * FIXELEMENT; // 192

/// Maximum mismatches allowed on a read
pub const MAXSNPS: u32 = 15;

/// Maximum continuous gap size (insertion or deletion)
pub const MAXGAPS: u32 = 3;

/// Maximum number of equal-best hits to track
pub const MAXHITS: usize = 100;

/// Number of reads processed per batch
pub const BATCH_SIZE: usize = 50_000;

/// Reference margin in 64-bit words for concatenated reference
pub const REF_MARGIN: usize = 400;

/// Extra u64 words padding at end of each chromosome binary sequence
pub const BINSEQPAD: usize = 2;

/// Prefetch lookahead for k-mer frequency counting
pub const PREFETCH_CAL_UNIT: usize = 8;

/// Prefetch lookahead for index filling
pub const PREFETCH_CRT_UNIT: usize = 6;

/// Prefetch loop distance
pub const PREFETCH_LOOP: usize = 10;

// ── Type Aliases ────────────────────────────────────────────────────────────

/// Chromosome identifier (matches C++ `ref_id_t`)
pub type RefId = u32;

/// Reference position (matches C++ `ref_loc_t`)
pub type RefLoc = u32;

// ── Hit Structures ──────────────────────────────────────────────────────────

/// A basic hit: chromosome + position (C++ `Hit`)
#[derive(Debug, Clone, Copy, Default)]
pub struct Hit {
    pub chr: RefId,
    pub loc: RefLoc,
}

/// A gapped hit with strand and gap information (C++ `gHit`, bitfield-packed)
///
/// In C++, this is packed into 8 bytes via bitfields:
///   loc:32, chr:18, strand:2, gap_size:4, gap_pos:8
/// We expand to 16 bytes for clarity; can be packed later if needed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GHit {
    pub loc: RefLoc,        // 0-based position on reference
    pub chr: RefId,         // chromosome index
    pub strand: u8,         // 00:++, 01:+-, 10:-+, 11:--
    pub gap_size: i16,      // >0: insertion on read, <0: deletion on read
    pub gap_pos: u16,       // gap position from read start
    pub snps: u8,           // mismatch count
}

/// Seed index entry for RRBS mode (C++ `KmerLoc`)
#[derive(Debug, Clone)]
pub struct KmerLoc {
    pub n1: u32,
    pub loc1: Vec<Hit>,
}

/// Seed index entry for WGBS mode (C++ `KmerLoc2`)
///
/// Matches the C++ `KmerLoc2` layout where positions are stored in a flat
/// array with forward-chain and reverse-chain entries separated:
///
/// ```text
/// loc1 layout: [forward_chain_hits... | reverse_chain_hits...]
/// n[0] = reverse chain hit count
/// n[1] = forward chain hit count
/// ```
///
/// The total hit count is `n[0] + n[1]`. Forward-chain positions start at
/// offset 0 and span `n[1]` entries; reverse-chain positions follow
/// immediately after, spanning `n[0]` entries.
#[derive(Debug, Clone, Copy)]
pub struct KmerLoc2 {
    /// `n[0]` = reverse chain hit count, `n[1]` = forward chain hit count.
    pub n: [u32; 2],
}

// ── Read-related ────────────────────────────────────────────────────────────

/// A single sequencing read (C++ `ReadInf`)
#[derive(Debug, Clone)]
pub struct ReadInf {
    pub index: u32,
    pub read_set: u32,      // 0=single-end, 1=PE read1, 2=PE read2
    pub name: String,
    pub seq: Vec<u8>,
    pub qual: Vec<u8>,
}

// ── Alignment Configuration ─────────────────────────────────────────────────

/// Master configuration for alignment (C++ `Param`)
///
/// This struct holds all CLI-configurable parameters plus derived values.
/// It is wrapped in `Arc` and shared read-only across all worker threads.
pub struct AlignConfig {
    // ── Seed Parameters ─────────────────────────────────────────────────
    /// Seed (k-mer) size, range 10-16
    pub seed_size: u32,
    /// Bit mask for seed extraction: (1 << (seed_size*2)) - 1
    pub seed_bits: u64,
    /// Left-zero bits in seed extraction: (SEGLEN - seed_size) * 2
    pub seed_bits_lz: u32,

    // ── Mismatch Parameters ─────────────────────────────────────────────
    /// Max mismatches: <100 = absolute count, >=100 = (val-100)% of read length
    pub max_snp_num: u32,
    /// Max number of equal-best hits to report
    pub max_num_hits: u32,

    // ── Gap Parameters ──────────────────────────────────────────────────
    /// Max gap size (0-3), 0 = no gaps
    pub gap: u32,
    /// Min distance from read edge for gap placement
    pub gap_edge: u32,

    // ── Paired-end ──────────────────────────────────────────────────────
    pub paired_end: bool,
    pub min_insert: u32,       // default 28
    pub max_insert: u32,       // default 1000

    // ── Indexing ────────────────────────────────────────────────────────
    /// Index interval (1-16): genome indexed every N bp
    pub index_interval: u32,
    /// Max k-mer frequency ratio for over-represented k-mer filtering
    pub max_kmer_ratio: f64,
    /// Computed max k-mer count threshold
    pub max_kmer_num: u32,

    // ── RRBS Mode ───────────────────────────────────────────────────────
    pub rrbs_flag: bool,
    /// Digestion sites: (site_sequence, cut_position)
    pub digest_sites: Vec<String>,
    pub digest_positions: Vec<u32>,

    // ── Strand Configuration ────────────────────────────────────────────
    /// false = forward strands only (BSW++, BSC-+), true = all 4 strands
    pub chains: bool,
    /// Read nucleotide that maps to ref_nt (default 'T')
    pub read_nt: u8,
    /// Reference nucleotide (default 'C')
    pub ref_nt: u8,
    /// 3-nucleotide alignment mode (C+T share same code)
    pub nt3: bool,

    // ── Read Processing ─────────────────────────────────────────────────
    pub min_read_size: u32,
    pub max_read_len: u32,     // (FIXELEMENT-1) * SEGLEN = 160
    pub max_ns: u32,           // max N's allowed in read
    pub qual_threshold: u8,    // quality threshold for 3'-end trimming
    pub zero_qual: u8,         // base quality offset ('!' = 33)
    pub adapters: Vec<String>, // adapter sequences for trimming

    // ── Output ──────────────────────────────────────────────────────────
    pub out_sam: u32,          // 0=BSP, 1=SAM, 2=BAM
    pub out_ref: bool,         // include reference sequence in output
    pub out_unmap: bool,       // report unmapped reads
    pub report_repeat_hits: u8, // 0=unique, 1=random, 2=all
    pub sam_header: bool,
    pub stdout: bool,          // write to stdout
    pub pipe_out: bool,        // output via pipe to samtools
    pub verbose_level: u8,

    // ── Parallelism ─────────────────────────────────────────────────────
    pub num_threads: usize,

    // ── Input ───────────────────────────────────────────────────────────
    pub gz_input: bool,
    pub gz_ref: bool,
    pub input_format: i32,     // 0=FASTA, 1=FASTQ, 2=SAM, 3=BAM, -1=auto
    pub read_start: u32,
    pub read_end: u32,

    // ── Derived ─────────────────────────────────────────────────────────
    pub max_seed_seg_num: usize,
    pub total_ref_seq: u32,
    pub randseed: u32,

    // ── Usable nucleotide characters ────────────────────────────────────
    pub useful_nt: String,
    pub nx_nt: String,

    // ── Seed profile: precomputed start positions per-mismatch-segment/interval ──
    /// profile[mismatch_count][interval_index]
    pub profile: [[u32; 16]; MAXSNPS as usize + 1],
}

impl Default for AlignConfig {
    fn default() -> Self {
        let num_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let mut config = Self {
            seed_size: 16,
            seed_bits: 0,          // set below
            seed_bits_lz: 0,       // set below

            max_snp_num: 108,      // 8% of read length (encoded)
            max_num_hits: MAXHITS as u32,

            gap: 0,
            gap_edge: 6,

            paired_end: false,
            min_insert: 28,
            max_insert: 1000,

            index_interval: 4,
            max_kmer_ratio: 5e-7,
            max_kmer_num: u32::MAX,

            rrbs_flag: false,
            digest_sites: Vec::new(),
            digest_positions: Vec::new(),

            chains: false,
            read_nt: b'T',
            ref_nt: b'C',
            nt3: false,

            min_read_size: 16,
            max_read_len: (FIXELEMENT - 1) as u32 * SEGLEN as u32, // 160
            max_ns: 5,
            qual_threshold: 0,
            zero_qual: b'!',
            adapters: Vec::new(),

            out_sam: 0,
            out_ref: false,
            out_unmap: false,
            report_repeat_hits: 1,
            sam_header: true,
            stdout: true,
            pipe_out: false,
            verbose_level: 1,

            num_threads: num_cpus,

            gz_input: false,
            gz_ref: false,
            input_format: -1,
            read_start: 1,
            read_end: u32::MAX,

            max_seed_seg_num: 0,
            total_ref_seq: 0,
            randseed: 0,

            useful_nt: String::from("ACGTacgt"),
            nx_nt: String::from("NXnx"),

            profile: [[0u32; 16]; MAXSNPS as usize + 1],
        };

        // Recompute derived seed values
        config.set_seed_size(16);
        config.init_profile();
        config
    }
}

impl AlignConfig {
    /// Set seed size and recompute derived masks.
    pub fn set_seed_size(&mut self, n: u32) {
        assert!(n >= 10 && n <= 16, "seed size must be 10-16");
        self.seed_size = n;
        self.seed_bits_lz = (SEGLEN as u32 - n) * 2;
        self.min_read_size = n + self.index_interval - 1;

        self.seed_bits = 0;
        for i in 0..n {
            self.seed_bits |= 0x3u64 << (i * 2);
        }
    }

    /// Initialize the profile matrix: for each snp count j and interval i,
    /// compute ((j*seed_size + i + index_interval - 1) / index_interval) * index_interval
    pub fn init_profile(&mut self) {
        for j in 0..=MAXSNPS as usize {
            for i in 0..self.index_interval as usize {
                self.profile[j][i] = ((j as u32 * self.seed_size
                    + i as u32
                    + self.index_interval
                    - 1)
                    / self.index_interval)
                    * self.index_interval;
            }
        }
    }
}

impl From<&AlignArgs> for AlignConfig {
    fn from(args: &AlignArgs) -> Self {
        let mut config = AlignConfig::default();

        // 种子参数
        let is_rrbs = !args.digestion_sites.is_empty();
        let seed_size = if is_rrbs && args.seed_size == 16 { 12 } else { args.seed_size };
        config.set_seed_size(seed_size);

        // 错配参数：0 < v < 1 编码为百分比，否则直接使用
        if args.max_mismatch > 0.0 && args.max_mismatch < 1.0 {
            config.max_snp_num = (args.max_mismatch * 100.0) as u32 + 100;
        } else {
            config.max_snp_num = args.max_mismatch as u32;
        }

        // 最大命中数
        config.max_num_hits = args.max_hits;

        // 间隙参数
        config.gap = args.gap_size;

        // 双端测序
        config.paired_end = args.query_b.is_some();
        config.min_insert = args.min_insert;
        config.max_insert = args.max_insert;

        // 索引参数
        config.index_interval = args.index_interval;
        config.max_kmer_ratio = args.kmer_cutoff;

        // RRBS 模式
        config.rrbs_flag = !args.digestion_sites.is_empty();
        config.digest_sites = args.digestion_sites.clone();
        // Match C++: RRBS forces index_interval=1, but -n still controls chains.
        if config.rrbs_flag {
            config.index_interval = 1;
        }

        // Chain mode follows the -n argument in both WGBS and RRBS.
        config.chains = args.chains == 1;

        // 碱基转换配置：解析 align_transition（如 "TC"）
        let transition_bytes = args.align_transition.as_bytes();
        if transition_bytes.len() == 2 {
            config.read_nt = transition_bytes[0];
            config.ref_nt = transition_bytes[1];
        }

        // 三核苷酸模式
        config.nt3 = args.nt3;

        // 读处理参数
        config.max_ns = args.max_ns;
        config.qual_threshold = args.qual_threshold;
        config.zero_qual = args.zero_qual;
        config.adapters = args.adapters.clone();

        // 输出参数
        config.out_ref = args.out_ref;
        config.out_unmap = args.out_unmap;
        config.report_repeat_hits = args.report_repeat;
        config.sam_header = !args.no_header;
        config.stdout = args.output.is_none();

        // 根据输出文件后缀判断输出格式
        if let Some(ref out_path) = args.output {
            let ext = out_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());
            config.out_sam = match ext.as_deref() {
                Some("sam") => 1,
                Some("bam") => 2,
                _ => 0, // BSP 格式
            };
        } else if config.stdout {
            // 管道模式（stdout）默认输出 SAM 格式，便于下游工具（如 methratio）解析
            config.out_sam = 1;
        }

        // 并行参数
        if let Some(threads) = args.num_threads {
            config.num_threads = threads;
        }

        // 其他参数
        config.randseed = args.randseed;
        config.verbose_level = args.verbose;
        config.read_start = args.read_start;

        if args.max_read_len > 0 {
            config.max_read_len = args.max_read_len;
        }

        if let Some(read_end) = args.read_end {
            config.read_end = read_end;
        }

        // 重新初始化 profile（因为 index_interval 可能已更改）
        config.init_profile();

        config
    }
}

// ── Global Statistics (lock-free atomics) ───────────────────────────────────

pub struct AlignStats {
    pub n_aligned: AtomicU32,
    pub n_unique: AtomicU32,
    pub n_multiple: AtomicU32,
    pub n_aligned_pairs: AtomicU32,
    pub n_unique_pairs: AtomicU32,
    pub n_multiple_pairs: AtomicU32,
    pub n_aligned_a: AtomicU32,
    pub n_unique_a: AtomicU32,
    pub n_multiple_a: AtomicU32,
    pub n_aligned_b: AtomicU32,
    pub n_unique_b: AtomicU32,
    pub n_multiple_b: AtomicU32,
}

impl Default for AlignStats {
    fn default() -> Self {
        Self {
            n_aligned: AtomicU32::new(0),
            n_unique: AtomicU32::new(0),
            n_multiple: AtomicU32::new(0),
            n_aligned_pairs: AtomicU32::new(0),
            n_unique_pairs: AtomicU32::new(0),
            n_multiple_pairs: AtomicU32::new(0),
            n_aligned_a: AtomicU32::new(0),
            n_unique_a: AtomicU32::new(0),
            n_multiple_a: AtomicU32::new(0),
            n_aligned_b: AtomicU32::new(0),
            n_unique_b: AtomicU32::new(0),
            n_multiple_b: AtomicU32::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_set_seed_size() {
        let mut config = AlignConfig::default();

        // 测试 seed_size=12
        config.set_seed_size(12);
        assert_eq!(config.seed_size, 12);
        assert_eq!(config.seed_bits_lz, (SEGLEN as u32 - 12) * 2); // (32-12)*2 = 40
        // seed_bits 应该是低 24 位全为 1（12 个碱基，每个 2 位）
        let expected_bits: u64 = (1u64 << (12 * 2)) - 1;
        assert_eq!(config.seed_bits, expected_bits);

        // 测试 seed_size=16
        config.set_seed_size(16);
        assert_eq!(config.seed_size, 16);
        assert_eq!(config.seed_bits_lz, (SEGLEN as u32 - 16) * 2); // (32-16)*2 = 32
        let expected_bits: u64 = (1u64 << (16 * 2)) - 1;
        assert_eq!(config.seed_bits, expected_bits);

        // 测试 seed_size=10（最小值）
        config.set_seed_size(10);
        assert_eq!(config.seed_size, 10);
        assert_eq!(config.seed_bits_lz, (SEGLEN as u32 - 10) * 2); // (32-10)*2 = 44
    }

    #[test]
    fn test_init_profile() {
        let mut config = AlignConfig::default();
        config.set_seed_size(16);
        config.index_interval = 4;
        config.init_profile();

        // profile[0][0] = ((0*16 + 0 + 4 - 1) / 4) * 4 = (3/4)*4 = 0
        assert_eq!(config.profile[0][0], 0);

        // profile[0][1] = ((0*16 + 1 + 4 - 1) / 4) * 4 = (4/4)*4 = 4
        assert_eq!(config.profile[0][1], 4);

        // profile[1][0] = ((1*16 + 0 + 4 - 1) / 4) * 4 = (19/4)*4 = 4*4 = 16
        assert_eq!(config.profile[1][0], 16);

        // profile[1][3] = ((1*16 + 3 + 4 - 1) / 4) * 4 = (22/4)*4 = 5*4 = 20
        assert_eq!(config.profile[1][3], 20);

        // profile[2][0] = ((2*16 + 0 + 4 - 1) / 4) * 4 = (35/4)*4 = 8*4 = 32
        assert_eq!(config.profile[2][0], 32);
    }

    #[test]
    fn test_default_config() {
        let config = AlignConfig::default();

        // 验证默认值的合理性
        assert_eq!(config.seed_size, 16);
        assert!(config.seed_bits > 0);
        assert!(config.seed_bits_lz > 0);
        assert_eq!(config.max_snp_num, 108); // 8% 编码
        assert_eq!(config.max_num_hits, MAXHITS as u32);
        assert_eq!(config.gap, 0);
        assert_eq!(config.index_interval, 4);
        assert!(!config.paired_end);
        assert!(!config.chains);
        assert!(!config.nt3);
        assert!(!config.rrbs_flag);
        assert_eq!(config.min_insert, 28);
        assert_eq!(config.max_insert, 1000);
        assert_eq!(config.read_nt, b'T');
        assert_eq!(config.ref_nt, b'C');
        assert!(config.num_threads >= 1);
        assert!(config.max_read_len > 0);
        assert_eq!(config.read_start, 1);
        assert_eq!(config.read_end, u32::MAX);
        assert!(config.sam_header);
        assert!(config.stdout);
        assert_eq!(config.out_sam, 0);
    }

    #[test]
    fn test_from_align_args_basic() {
        use crate::cli::AlignArgs;
        use std::path::PathBuf;

        // 使用最小参数构建 AlignArgs
        let args = AlignArgs {
            query_a: Some(PathBuf::from("reads.fq")),
            query_b: None,
            reference: Some(PathBuf::from("ref.fa")),
            output: None,
            seed_size: 16,
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
            verbose: 1,
        };

        let config = AlignConfig::from(&args);

        // 验证基本映射
        assert_eq!(config.seed_size, args.seed_size);
        assert_eq!(config.max_num_hits, args.max_hits);
        assert_eq!(config.gap, args.gap_size);
        assert_eq!(config.min_insert, args.min_insert);
        assert_eq!(config.max_insert, args.max_insert);
        assert_eq!(config.index_interval, args.index_interval);
        assert_eq!(config.max_kmer_ratio, args.kmer_cutoff);
        assert_eq!(config.qual_threshold, args.qual_threshold);
        assert_eq!(config.zero_qual, args.zero_qual);
        assert_eq!(config.max_ns, args.max_ns);
        assert!(!config.paired_end); // 没有提供 query_b
        assert!(config.stdout); // 没有提供 output
        assert!(config.sam_header); // 没有设置 no_header
        assert!(!config.rrbs_flag); // 没有提供 digestion_sites
        assert_eq!(config.randseed, args.randseed);
        assert_eq!(config.verbose_level, args.verbose);

        // 验证错配编码：默认 0.08 → (0.08*100)+100 = 108
        assert_eq!(config.max_snp_num, 108);

        // 验证 read_nt 和 ref_nt 来自默认的 "TC"
        assert_eq!(config.read_nt, b'T');
        assert_eq!(config.ref_nt, b'C');
    }

    #[test]
    fn test_from_align_args_paired_end() {
        use crate::cli::AlignArgs;
        use std::path::PathBuf;

        let args = AlignArgs {
            query_a: Some(PathBuf::from("reads_1.fq")),
            query_b: Some(PathBuf::from("reads_2.fq")),
            reference: Some(PathBuf::from("ref.fa")),
            output: Some(PathBuf::from("output.sam")),
            seed_size: 16,
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
            verbose: 1,
        };

        let config = AlignConfig::from(&args);

        assert!(config.paired_end);
        assert!(!config.stdout); // 有输出文件
        assert_eq!(config.out_sam, 1); // .sam 后缀
    }

    #[test]
    fn test_from_align_args_rrbs() {
        use crate::cli::AlignArgs;
        use std::path::PathBuf;

        let args = AlignArgs {
            query_a: Some(PathBuf::from("reads.fq")),
            query_b: None,
            reference: Some(PathBuf::from("ref.fa")),
            output: None,
            seed_size: 12,
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
            digestion_sites: vec!["C-CGG".to_string()],
            num_threads: None,
            randseed: 0,
            verbose: 1,
        };

        let config = AlignConfig::from(&args);

        assert!(config.rrbs_flag);
        assert_eq!(config.digest_sites, vec!["C-CGG"]);
        assert_eq!(config.seed_size, 12);
    }

    #[test]
    fn test_from_align_args_output_format() {
        use crate::cli::AlignArgs;
        use std::path::PathBuf;

        // 测试 BAM 输出
        let args = AlignArgs {
            query_a: Some(PathBuf::from("reads.fq")),
            query_b: None,
            reference: Some(PathBuf::from("ref.fa")),
            output: Some(PathBuf::from("output.bam")),
            seed_size: 16,
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
            verbose: 1,
        };

        let config = AlignConfig::from(&args);
        assert_eq!(config.out_sam, 2); // BAM

        // 测试 BSP 输出（无标准后缀）
        let args2 = AlignArgs {
            output: Some(PathBuf::from("output.bsp")),
            ..args.clone()
        };

        let config = AlignConfig::from(&args2);
        assert_eq!(config.out_sam, 0); // BSP
    }
}
