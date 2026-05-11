//! Constants, types, and global configuration for BSMAP-rs.
//!
//! Mirrors the C++ `param.h` and `param.cpp` — defines the fundamental
//! data types, alignment parameters, and the `AlignConfig` struct.

use std::sync::atomic::AtomicU32;

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
#[derive(Debug, Clone, Copy, Default)]
pub struct GHit {
    pub loc: RefLoc,        // 0-based position on reference
    pub chr: RefId,         // chromosome index
    pub strand: u8,         // 00:++, 01:+-, 10:-+, 11:--
    pub gap_size: i16,      // >0: insertion on read, <0: deletion on read
    pub gap_pos: u16,       // gap position from read start
}

/// Seed index entry for RRBS mode (C++ `KmerLoc`)
#[derive(Debug, Clone)]
pub struct KmerLoc {
    pub n1: u32,
    pub loc1: Vec<Hit>,
}

/// Seed index entry for WGBS mode (C++ `KmerLoc2`)
#[derive(Debug, Clone)]
pub struct KmerLoc2 {
    /// n[0] = count, n[1] = accumulated offset for filling
    pub n: [u32; 2],
    pub loc1: Vec<u32>,
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
            .map(|n| n.get().min(8))
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
