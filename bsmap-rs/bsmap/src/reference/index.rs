//! K-mer seed index for WGBS and RRBS alignment modes.
//!
//! Builds a hash-table index over all k-mers (seeds) in the reference
//! genome. For WGBS mode, uses `KmerLoc2` with flat position storage.
//! For RRBS mode, uses one offset table and a flat `Hit` array.
//!
//! Mirrors C++ `RefSeq::InitialIndex()`, `CalKmerFreq()`, `AllocIndex()`,
//! `FillIndex()`, and `FinishIndex()`.

use crate::alphabet::make_seed;
use crate::param::{Hit, KmerLoc2, SEGLEN};
use crate::reference::binseq::{BinSeqCollection, Block};
use crate::reference::fasta::Reference;
use crate::reference::rrbs::{build_rrbs_index_from_sites, find_sites, DigestionSite};

/// Prefetch lookahead for frequency counting.
const PREFETCH_CAL_UNIT: usize = 8;
/// Prefetch lookahead for index filling.
const PREFETCH_CRT_UNIT: usize = 6;

/// Low bits in an RRBS hit store the C++ block id (`chr` in the original code).
pub const RRBS_CHR_MASK: u32 = 0x0000ffff;
/// RRBS seed mode starts at bit 16, matching C++ `(j << 16)`.
pub const RRBS_MODE_SHIFT: u32 = 16;
/// C++ marker for cross-chain RRBS entries.
pub const RRBS_BSC_FLAG: u32 = 0x01000000;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MappedSection {
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug)]
pub(crate) struct MappedKmerIndex {
    pub mmap: memmap2::Mmap,
    pub index2: MappedSection,
    pub positions: MappedSection,
    pub start_offsets: MappedSection,
    pub rrbs_offsets: MappedSection,
    pub rrbs_hits: MappedSection,
    pub rrbs_site_offsets: MappedSection,
    pub rrbs_sites: MappedSection,
}

impl MappedKmerIndex {
    #[inline]
    unsafe fn slice<T>(&self, section: MappedSection) -> &[T] {
        std::slice::from_raw_parts(
            self.mmap.as_ptr().add(section.offset) as *const T,
            section.len,
        )
    }
}

/// 安全的种子提取：检查边界，避免越界访问。
/// 返回 None 如果位置超出 words 数组范围。
#[inline]
fn try_make_seed(words: &[u64], bit_pos: u64, seed_bits_lz: u32) -> Option<u32> {
    let word_idx = (bit_pos / (SEGLEN as u64 * 2)) as usize;
    let bit_offset = (bit_pos % 64) as u32;

    if word_idx >= words.len() {
        return None;
    }

    // 检查是否需要跨 word 边界
    if bit_offset > 0 && word_idx + 1 >= words.len() {
        return None;
    }

    Some(make_seed(words, bit_pos, seed_bits_lz))
}

// ── K-mer Index ───────────────────────────────────────────────────────────────

/// Complete k-mer seed index for alignment.
pub struct KmerIndex {
    /// Total number of k-mer hash buckets (3^seed_size).
    pub total_kmers: u32,
    /// Maximum k-mer frequency cutoff (top ratio filtered).
    pub max_kmer_num: u32,

    // ── WGBS mode ──────────────────────────────────────────────────────
    /// WGBS index entries (one per hash bucket).
    pub index2: Vec<KmerLoc2>,
    /// Flat storage for all k-mer hit positions.
    pub positions: Vec<u32>,
    /// Precomputed start offsets for each hash bucket in the positions array.
    /// `start_offsets[i]` = byte offset where hash i's forward positions begin.
    /// Enables O(1) lookup in `lookup_separated`.
    pub(crate) start_offsets: Vec<u32>,

    // ── RRBS mode ──────────────────────────────────────────────────────
    /// RRBS bucket boundaries. Bucket `i` is
    /// `rrbs_hits[rrbs_offsets[i]..rrbs_offsets[i + 1]]`.
    pub rrbs_offsets: Vec<u32>,
    /// Flat RRBS hit storage in the same order as C++ `FillIndex()`.
    pub rrbs_hits: Vec<Hit>,
    /// Per-chromosome boundaries into `rrbs_sites`.
    pub(crate) rrbs_site_offsets: Vec<u32>,
    /// Sorted `(cut_position, reverse_offset)` pairs used for RRBS ZP/ZL.
    pub(crate) rrbs_sites: Vec<[u32; 2]>,

    /// Seed size (k-mer length), required for RRBS position conversion.
    pub seed_size: u32,

    /// v7 raw sections mapped directly from the index file.
    pub(crate) mapped: Option<MappedKmerIndex>,
}

/// Incremental RRBS digestion-site collector for streaming FASTA input.
pub struct RrbsIndexBuilder {
    seed_size: u32,
    total_kmers: u32,
    total_blocks: usize,
    sites: Vec<DigestionSite>,
    min_insert: u32,
    max_insert: u32,
    ccgg_index: Vec<Vec<Vec<u32>>>,
    rrbs_site_offsets: Vec<u32>,
    rrbs_sites: Vec<(u32, u32)>,
}

impl RrbsIndexBuilder {
    pub fn new(
        chromosome_count: usize,
        seed_size: u32,
        digest_sites: &[String],
        min_insert: u32,
        max_insert: u32,
    ) -> Self {
        let total_blocks = chromosome_count * 2;
        let max_modes = ((crate::param::FIXELEMENT - 1) * SEGLEN) as u32 / seed_size;
        Self {
            seed_size,
            total_kmers: 3u32.pow(seed_size),
            total_blocks,
            sites: digest_sites
                .iter()
                .filter_map(|site| DigestionSite::parse(site))
                .collect(),
            min_insert,
            max_insert,
            ccgg_index: vec![vec![Vec::new(); total_blocks]; max_modes as usize],
            rrbs_site_offsets: vec![0],
            rrbs_sites: Vec::new(),
        }
    }

    pub fn push_reference(&mut self, chromosome_index: usize, reference: &Reference) {
        let required_blocks = (chromosome_index + 1) * 2;
        if required_blocks > self.total_blocks {
            for mode_blocks in &mut self.ccgg_index {
                mode_blocks.resize_with(required_blocks, Vec::new);
            }
            self.total_blocks = required_blocks;
        }
        let chromosome_sites = find_sites(&reference.seq, &self.sites);
        self.rrbs_sites.extend_from_slice(&chromosome_sites);
        self.rrbs_site_offsets.push(self.rrbs_sites.len() as u32);
        if chromosome_sites.is_empty() {
            return;
        }
        let rc_offset = ((reference.len + SEGLEN as u32 - 1) / SEGLEN as u32
            + crate::param::BINSEQPAD as u32)
            * SEGLEN as u32;
        let digested = build_rrbs_index_from_sites(
            &chromosome_sites,
            reference.len,
            rc_offset,
            self.seed_size,
            self.min_insert,
            self.max_insert,
        );
        for (mode, chains) in digested.iter().enumerate() {
            for chain in 0..2usize {
                let target_block = chromosome_index * 2 + chain;
                if mode < self.ccgg_index.len() && target_block < self.total_blocks {
                    self.ccgg_index[mode][target_block].extend(chains[chain].iter().copied());
                }
            }
        }
    }

    pub fn finish(self, coll: &BinSeqCollection) -> KmerIndex {
        let mut index = build_flat_rrbs_index(
            coll,
            &self.ccgg_index,
            self.total_blocks,
            self.seed_size,
            self.total_kmers,
        );
        index.rrbs_site_offsets = self.rrbs_site_offsets;
        index.rrbs_sites = self
            .rrbs_sites
            .into_iter()
            .map(|(position, reverse_offset)| [position, reverse_offset])
            .collect();
        index
    }
}

impl KmerIndex {
    #[inline]
    pub(crate) fn index2_slice(&self) -> &[KmerLoc2] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.index2) },
            None => &self.index2,
        }
    }

    #[inline]
    pub(crate) fn positions_slice(&self) -> &[u32] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.positions) },
            None => &self.positions,
        }
    }

    #[inline]
    pub(crate) fn start_offsets_slice(&self) -> &[u32] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.start_offsets) },
            None => &self.start_offsets,
        }
    }

    #[inline]
    pub(crate) fn rrbs_offsets_slice(&self) -> &[u32] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.rrbs_offsets) },
            None => &self.rrbs_offsets,
        }
    }

    #[inline]
    pub(crate) fn rrbs_hits_slice(&self) -> &[Hit] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.rrbs_hits) },
            None => &self.rrbs_hits,
        }
    }

    #[inline]
    pub(crate) fn rrbs_site_offsets_slice(&self) -> &[u32] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.rrbs_site_offsets) },
            None => &self.rrbs_site_offsets,
        }
    }

    #[inline]
    pub(crate) fn rrbs_sites_slice(&self) -> &[[u32; 2]] {
        match &self.mapped {
            Some(mapped) => unsafe { mapped.slice(mapped.rrbs_sites) },
            None => &self.rrbs_sites,
        }
    }

    #[inline]
    pub fn has_rrbs_index(&self) -> bool {
        !self.rrbs_offsets_slice().is_empty()
    }

    /// Build the k-mer index for WGBS mode.
    ///
    /// Three-pass algorithm:
    /// 1. Count k-mer frequencies per chain across all unmasked blocks
    /// 2. Allocate storage (compute prefix sums, apply frequency cutoff)
    /// 3. Fill positions: forward chain first (chain=0), then reverse chain (chain=1)
    ///
    /// The resulting `KmerLoc2.n` semantics match C++ `KmerLoc2`:
    ///   `n[0]` = reverse chain hit count
    ///   `n[1]` = forward chain hit count
    ///   positions layout: [forward_hits... | reverse_hits...]
    pub fn build_wgbs(
        coll: &BinSeqCollection,
        seed_size: u32,
        index_interval: u32,
        max_kmer_ratio: f64,
    ) -> Self {
        let total_kmers = 3u32.pow(seed_size);
        let seed_bits_lz = (SEGLEN as u32 - seed_size) * 2;

        // ── Pass 1: Count frequencies per chain ────────────────────────
        // fwd_counts[hash] = forward chain (chain=0) frequency
        // rev_counts[hash] = reverse chain (chain=1) frequency
        let mut fwd_counts = vec![0u32; total_kmers as usize];
        let mut rev_counts = vec![0u32; total_kmers as usize];

        count_frequencies_separated(
            coll.refcat.as_slice(),
            coll.crefcat.as_slice(),
            &coll.blocks,
            coll,
            index_interval,
            seed_size,
            seed_bits_lz,
            &mut fwd_counts,
            &mut rev_counts,
        );

        // ── Pass 2: Compute cutoff and prefix sums ────────────────────
        let total_counts: Vec<u32> = fwd_counts
            .iter()
            .zip(rev_counts.iter())
            .map(|(&f, &r)| f + r)
            .collect();

        let mut sorted_counts = total_counts.clone();
        // Match C++: sort(kmer_count, kmer_count + total_kmers - 1)
        // C++ sorts only N-1 elements, leaving the last unsorted
        let n = sorted_counts.len();
        if n > 1 {
            sorted_counts[..n - 1].sort_unstable();
        }

        let cutoff_idx = ((total_kmers as f64) * (1.0 - max_kmer_ratio)) as usize;
        let max_kmer_num = if cutoff_idx > 0 {
            sorted_counts[cutoff_idx.saturating_sub(1)]
        } else {
            u32::MAX
        };

        // Keep raw counts for every bucket. C++ CountSeeds uses these counts even
        // when SnpAlign later skips an over-represented bucket.
        let mut index2: Vec<KmerLoc2> = Vec::with_capacity(total_kmers as usize);
        let mut total_positions: u32 = 0;

        // C++ filters by n[0] which is the TOTAL count across both chains.
        // Using fwd-only would exclude k-mers that only appear on the Crick strand (fwd=0, rev>0).
        for i in 0..total_kmers as usize {
            let fwd = fwd_counts[i];
            let rev = rev_counts[i];
            let total = fwd + rev;
            index2.push(KmerLoc2 {
                n: [rev, fwd],
            });
            if total > 0 && total <= max_kmer_num {
                total_positions += fwd + rev;
            }
        }

        // ── Pass 3: Fill positions (forward first, then reverse) ──────
        let mut positions: Vec<u32> = vec![0u32; total_positions as usize];

        // Compute write offsets per hash.
        // Layout: [hash0_fwd | hash0_rev | hash1_fwd | hash1_rev | ...]
        // For each hash i, forward positions start at sum_{j<i}(fwd_counts[j] + rev_counts[j])
        // and reverse positions follow immediately after forward positions.
        let mut fwd_write_offsets: Vec<u32> = vec![0u32; total_kmers as usize];
        let mut rev_write_offsets: Vec<u32> = vec![0u32; total_kmers as usize];
        {
            let mut running_offset: u32 = 0;
            for i in 0..total_kmers as usize {
                let fwd = fwd_counts[i];
                let rev = rev_counts[i];
                let total = fwd + rev;
                if total > 0 && total <= max_kmer_num {
                    fwd_write_offsets[i] = running_offset;
                    rev_write_offsets[i] = running_offset + fwd;
                    running_offset += total;
                }
                // For filtered-out k-mers, offsets remain 0 (unused)
            }
        }

        // Save start offsets for O(1) lookup
        let start_offsets = fwd_write_offsets.clone();

        // Fill forward chain positions (chain=0)
        fill_positions_chain(
            coll.refcat.as_slice(),
            &coll.blocks,
            coll,
            0, // chain=0 (forward)
            index_interval,
            seed_size,
            seed_bits_lz,
            &mut positions,
            &mut fwd_write_offsets,
            &total_counts,
            max_kmer_num,
        );

        // Fill reverse chain positions (chain=1)
        fill_positions_chain(
            coll.crefcat.as_slice(),
            &coll.blocks,
            coll,
            1, // chain=1 (reverse)
            index_interval,
            seed_size,
            seed_bits_lz,
            &mut positions,
            &mut rev_write_offsets,
            &total_counts,
            max_kmer_num,
        );

        // Verify critical index entries for known test reads

        Self {
            total_kmers,
            max_kmer_num,
            index2,
            positions,
            start_offsets,
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            seed_size,
            mapped: None,
        }
    }

    /// Build RRBS mode index.
    ///
    /// Uses CCGG positions from RRBS module instead of scanning all blocks.
    /// 对每条染色体调用 build_rrbs_index 获取酶切位点索引，
    /// 然后构建 KmerLoc 索引。
    pub fn build_rrbs(
        coll: &BinSeqCollection,
        refs: &[Reference],
        seed_size: u32,
        _index_interval: u32,
        digest_sites: &[String],
        min_insert: u32,
        max_insert: u32,
    ) -> Self {
        let mut builder = RrbsIndexBuilder::new(
            refs.len(),
            seed_size,
            digest_sites,
            min_insert,
            max_insert,
        );
        for (chromosome_index, reference) in refs.iter().enumerate() {
            builder.push_reference(chromosome_index, reference);
        }
        builder.finish(coll)
    }

    /// Return C++ `CCGG_seglen()` output `(ZP, ZL)` for an RRBS alignment.
    pub fn rrbs_fragment(&self, chr: u32, pos: u32, read_len: u32) -> Option<(u32, u32)> {
        let chr = chr as usize;
        let site_offsets = self.rrbs_site_offsets_slice();
        let all_sites = self.rrbs_sites_slice();
        let start = *site_offsets.get(chr)? as usize;
        let end = *site_offsets.get(chr + 1)? as usize;
        let sites = all_sites.get(start..end)?;
        if sites.len() < 2 {
            return None;
        }

        let mut left = 0usize;
        let mut right = sites.len() - 1;
        while left < right - 1 {
            let mid = (left + right) / 2;
            if sites[mid][0] == pos {
                left = mid;
                right = mid + 1;
                break;
            } else if sites[mid][0] < pos {
                left = mid;
            } else {
                right = mid;
            }
        }

        let target_end = pos.checked_add(read_len)?;
        while right < sites.len() {
            let site_end = sites[right][0].checked_add(sites[right][1])?;
            if site_end >= target_end {
                let segment_start = sites[left][0];
                return Some((segment_start + 1, site_end - segment_start));
            }
            right += 1;
        }
        None
    }

    /// Look up k-mer seed in WGBS index.
    ///
    /// Returns all positions (forward + reverse chain combined) for the
    /// given seed hash. This is kept for backward compatibility.
    #[inline]
    pub fn lookup(&self, seed_hash: u32) -> &[u32] {
        let (fwd, rev) = self.lookup_separated(seed_hash);
        if fwd.is_empty() && rev.is_empty() {
            return &[];
        }
        // Both slices are contiguous in the positions array:
        // positions[...fwd...|...rev...]
        // Reconstruct the combined slice using pointer arithmetic.
        let total_len = fwd.len() + rev.len();
        // Safety: both slices are from the same positions array and contiguous
        unsafe {
            let ptr = fwd.as_ptr();
            std::slice::from_raw_parts(ptr, total_len)
        }
    }

    /// Look up k-mer seed in WGBS index, returning separated forward/reverse chain positions.
    ///
    /// Returns `(forward_chain_positions, reverse_chain_positions)`.
    ///
    /// - Forward chain positions: hits from `refcat` (block.id % 2 == 0)
    /// - Reverse chain positions: hits from `crefcat` (block.id % 2 == 1)
    ///
    /// The caller can use forward positions with `refcat` for validation and
    /// reverse positions with `crefcat` for validation.
    #[inline]
    pub fn lookup_separated(&self, seed_hash: u32) -> (&[u32], &[u32]) {
        let index2 = self.index2_slice();
        let start_offsets = self.start_offsets_slice();
        let positions = self.positions_slice();
        if index2.is_empty() || start_offsets.is_empty() {
            return (&[], &[]);
        }
        let idx = seed_hash as usize;
        if idx >= index2.len() {
            return (&[], &[]);
        }
        let entry = &index2[idx];
        let fwd_count = entry.n[1] as usize;
        let rev_count = entry.n[0] as usize;

        if fwd_count + rev_count == 0 || fwd_count + rev_count > self.max_kmer_num as usize {
            return (&[], &[]);
        }

        let start = start_offsets[idx] as usize;
        let fwd_slice = &positions[start..start + fwd_count];
        let rev_slice = &positions[start + fwd_count..start + fwd_count + rev_count];
        (fwd_slice, rev_slice)
    }

    /// Return the raw WGBS bucket size used by C++ `CountSeeds()`.
    #[inline]
    pub fn wgbs_candidate_count(&self, seed_hash: u32) -> u32 {
        self.index2_slice()
            .get(seed_hash as usize)
            .map_or(0, |entry| entry.n[0].saturating_add(entry.n[1]))
    }

    /// Look up one RRBS k-mer bucket in flat storage.
    #[inline]
    pub fn lookup_rrbs(&self, seed_hash: u32) -> &[Hit] {
        let rrbs_offsets = self.rrbs_offsets_slice();
        let rrbs_hits = self.rrbs_hits_slice();
        let idx = seed_hash as usize;
        if idx + 1 >= rrbs_offsets.len() {
            return &[];
        }
        let start = rrbs_offsets[idx] as usize;
        let end = rrbs_offsets[idx + 1] as usize;
        rrbs_hits.get(start..end).unwrap_or(&[])
    }
}

fn visit_rrbs_hits<F>(
    coll: &BinSeqCollection,
    ccgg_index: &[Vec<Vec<u32>>],
    total_blocks: usize,
    seed_size: u32,
    seed_bits_lz: u32,
    mut visit: F,
) where
    F: FnMut(usize, Hit),
{
    for (mode, mode_blocks) in ccgg_index.iter().enumerate() {
        for block_id in 0..total_blocks as u32 {
            let chr_idx = (block_id / 2) as usize;
            if chr_idx + 1 >= coll.ref_anchor.len() {
                continue;
            }
            let anchor = coll.ref_anchor[chr_idx];
            let rc_offset = coll.ref_anchor[chr_idx + 1] - anchor;
            let words = if block_id & 1 == 0 {
                coll.refcat.as_slice()
            } else {
                coll.crefcat.as_slice()
            };
            let hit_chr = block_id | ((mode as u32) << RRBS_MODE_SHIFT);

            for &loc in &mode_blocks[block_id as usize] {
                if let Some(hash) =
                    try_make_seed(words, (anchor as u64 + loc as u64) * 2, seed_bits_lz)
                {
                    visit(hash as usize, Hit { chr: hit_chr, loc });
                }
            }

            let other_block = (block_id ^ 1) as usize;
            let tmp_offset = rc_offset.saturating_sub(seed_size);
            let cross_hit_chr = hit_chr | RRBS_BSC_FLAG;
            for &other_pos in &mode_blocks[other_block] {
                if let Some(loc) = tmp_offset.checked_sub(other_pos) {
                    if let Some(hash) =
                        try_make_seed(words, (anchor as u64 + loc as u64) * 2, seed_bits_lz)
                    {
                        visit(
                            hash as usize,
                            Hit {
                                chr: cross_hit_chr,
                                loc,
                            },
                        );
                    }
                }
            }
        }
    }
}

fn build_flat_rrbs_index(
    coll: &BinSeqCollection,
    ccgg_index: &[Vec<Vec<u32>>],
    total_blocks: usize,
    seed_size: u32,
    total_kmers: u32,
) -> KmerIndex {
    if ccgg_index.is_empty() {
        return KmerIndex {
            total_kmers,
            max_kmer_num: u32::MAX,
            index2: Vec::new(),
            positions: Vec::new(),
            start_offsets: Vec::new(),
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: Vec::new(),
            rrbs_sites: Vec::new(),
            seed_size,
            mapped: None,
        };
    }

    let seed_bits_lz = (SEGLEN as u32 - seed_size) * 2;
    let mut counts = vec![0u32; total_kmers as usize];
    visit_rrbs_hits(
        coll,
        ccgg_index,
        total_blocks,
        seed_size,
        seed_bits_lz,
        |hash, _| counts[hash] = counts[hash].checked_add(1).expect("RRBS hit count overflow"),
    );

    let mut rrbs_offsets = vec![0u32; total_kmers as usize + 1];
    for (i, &count) in counts.iter().enumerate() {
        rrbs_offsets[i + 1] = rrbs_offsets[i]
            .checked_add(count)
            .expect("RRBS flat index exceeds u32 offsets");
    }
    let mut rrbs_hits = vec![Hit::default(); rrbs_offsets[total_kmers as usize] as usize];
    let mut write_offsets = rrbs_offsets[..total_kmers as usize].to_vec();
    visit_rrbs_hits(
        coll,
        ccgg_index,
        total_blocks,
        seed_size,
        seed_bits_lz,
        |hash, hit| {
            let offset = write_offsets[hash] as usize;
            rrbs_hits[offset] = hit;
            write_offsets[hash] += 1;
        },
    );

    KmerIndex {
        total_kmers,
        max_kmer_num: u32::MAX,
        index2: Vec::new(),
        positions: Vec::new(),
        start_offsets: Vec::new(),
        rrbs_offsets,
        rrbs_hits,
        rrbs_site_offsets: Vec::new(),
        rrbs_sites: Vec::new(),
        seed_size,
        mapped: None,
    }
}

// ── Frequency Counting (Pass 1) ───────────────────────────────────────────────

/// Count k-mer frequencies separately for forward and reverse chains.
///
/// Forward chain (chain=0): blocks with even block.id, stored in `refcat`
/// Reverse chain (chain=1): blocks with odd block.id, stored in `crefcat`
fn count_frequencies_separated(
    refcat: &[u64],
    crefcat: &[u64],
    blocks: &[Block],
    coll: &BinSeqCollection,
    index_interval: u32,
    seed_size: u32,
    seed_bits_lz: u32,
    fwd_counts: &mut [u32],
    rev_counts: &mut [u32],
) {
    let prefetch = PREFETCH_CAL_UNIT as u32 * index_interval;

    for block in blocks {
        let chain = block.id % 2;
        let words = if chain == 0 { refcat } else { crefcat };
        let counts: &mut [u32] = if chain == 0 { &mut fwd_counts[..] } else { &mut rev_counts[..] };

        let end_seedable = if block.end >= seed_size {
            ((block.end - seed_size) / index_interval) * index_interval
        } else {
            continue; // block too short for seed
        };

        let chr_idx = (block.id / 2) as usize;
        let anchor_pos = if chr_idx < coll.ref_anchor.len() {
            coll.ref_anchor[chr_idx]
        } else {
            continue;
        };

        // Prefetch buffer
        let mut dbs = [0u32; PREFETCH_CAL_UNIT];

        // Fill initial prefetch window
        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        for j in 0..PREFETCH_CAL_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, (anchor_pos as u64 + pos as u64) * 2, seed_bits_lz);
                pos += index_interval;
            }
            ptr += 1;
        }

        // Process all positions in block
        pos = start_pos;
        while pos <= end_seedable {
            let hash = dbs[(ptr % PREFETCH_CAL_UNIT as u32) as usize];
            counts[hash as usize] += 1;

            // Prefetch next
            let next_pos = pos + prefetch;
            if next_pos <= end_seedable {
                let next_hash = make_seed(words, (anchor_pos as u64 + next_pos as u64) * 2, seed_bits_lz);
                dbs[((ptr + PREFETCH_CAL_UNIT as u32) % PREFETCH_CAL_UNIT as u32)
                    as usize] = next_hash;
            }

            pos += index_interval;
            ptr += 1;
        }
    }
}

// ── Position Filling (Pass 3) ─────────────────────────────────────────────────

/// Fill positions for a single chain into the flat positions array.
///
/// `chain` is 0 (forward/refcat) or 1 (reverse/crefcat).
/// `write_offsets` is the per-hash write offset for this chain.
/// `total_counts` and `max_kmer_num` are used to skip over-represented k-mers (by total count, matching C++).
fn fill_positions_chain(
    words: &[u64],
    blocks: &[Block],
    coll: &BinSeqCollection,
    chain: u32,
    index_interval: u32,
    seed_size: u32,
    seed_bits_lz: u32,
    positions: &mut [u32],
    write_offsets: &mut [u32],
    total_counts: &[u32],
    max_kmer_num: u32,
) {
    let prefetch = PREFETCH_CRT_UNIT as u32 * index_interval;

    for block in blocks {
        if block.id % 2 != chain {
            continue;
        }

        let end_seedable = if block.end >= seed_size {
            ((block.end - seed_size) / index_interval) * index_interval
        } else {
            continue;
        };

        let chr_idx = (block.id / 2) as usize;
        let anchor_pos = if chr_idx < coll.ref_anchor.len() {
            coll.ref_anchor[chr_idx]
        } else {
            continue;
        };

        let mut dbs = [0u32; PREFETCH_CRT_UNIT];

        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        for j in 0..PREFETCH_CRT_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, (anchor_pos as u64 + pos as u64) * 2, seed_bits_lz);
                pos += index_interval;
            }
            ptr += 1;
        }

        pos = start_pos;
        let chr_id = block.id;
        while pos <= end_seedable {
            let hash = dbs[(ptr % PREFETCH_CRT_UNIT as u32) as usize] as usize;
            // Only fill if this k-mer is not over-represented
            if total_counts[hash] > 0 && total_counts[hash] <= max_kmer_num {
                let offset = write_offsets[hash] as usize;
                if offset < positions.len() {
                    positions[offset] = coll.hit2int(chr_id, pos);
                }
                write_offsets[hash] += 1;
            }

            let next_pos = pos + prefetch;
            if next_pos <= end_seedable {
                let next_hash = make_seed(words, (anchor_pos as u64 + next_pos as u64) * 2, seed_bits_lz);
                dbs[((ptr + PREFETCH_CRT_UNIT as u32) % PREFETCH_CRT_UNIT as u32)
                    as usize] = next_hash;
            }

            pos += index_interval;
            ptr += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::fasta::Reference;
    use crate::reference::binseq::BinSeqCollection;

    #[test]
    fn test_kmer_index_build_empty() {
        // Build a tiny reference and verify index structure
        let refs = vec![Reference {
            name: "tiny".into(),
            seq: b"A".repeat(100).to_vec(),
            len: 100,
        }];
        let coll = BinSeqCollection::from_references(&refs);

        // Use small seed_size=3 for manageable index size
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        assert_eq!(index.total_kmers, 27); // 3^3 = 27
    }

    #[test]
    fn test_kmer_index_total_kmers() {
        // 3^seed_size must fit in u32 for typical seed sizes
        assert_eq!(3u32.pow(12), 531_441); // typical min seed
        assert_eq!(3u32.pow(16), 43_046_721); // typical max seed (WGBS default)
    }

    #[test]
    fn test_streaming_rrbs_builder_matches_batch_builder() {
        let refs = vec![
            Reference {
                name: "chr1".into(),
                seq: b"ACGTCCGGAAAAAAAAAAAAAAAAAAAAAAACCGGTTTTTTTTTTTTTTTTTTTTTTTTCCGG"
                    .to_vec(),
                len: 68,
            },
            Reference {
                name: "chr2".into(),
                seq: b"TTTTCCGGCCCCCCCCCCCCCCCCCCCCCCCCCCGGAAAAAAAAAAAAAAAAAAAAAAAA"
                    .to_vec(),
                len: 62,
            },
        ];
        let coll = BinSeqCollection::from_references(&refs);
        let sites = vec!["C-CGG".to_string()];
        let batch = KmerIndex::build_rrbs(&coll, &refs, 3, 4, &sites, 4, 1000);
        let mut builder = RrbsIndexBuilder::new(refs.len(), 3, &sites, 4, 1000);
        for (index, reference) in refs.iter().enumerate() {
            builder.push_reference(index, reference);
        }
        let streamed = builder.finish(&coll);

        assert_eq!(streamed.rrbs_offsets, batch.rrbs_offsets);
        assert_eq!(streamed.rrbs_hits, batch.rrbs_hits);
        assert_eq!(streamed.rrbs_site_offsets, batch.rrbs_site_offsets);
        assert_eq!(streamed.rrbs_sites, batch.rrbs_sites);
    }

    #[test]
    fn test_rrbs_fragment_matches_cpp_ccgg_seglen() {
        let index = KmerIndex {
            total_kmers: 0,
            max_kmer_num: 0,
            index2: Vec::new(),
            positions: Vec::new(),
            start_offsets: Vec::new(),
            rrbs_offsets: Vec::new(),
            rrbs_hits: Vec::new(),
            rrbs_site_offsets: vec![0, 3],
            rrbs_sites: vec![[5, 2], [40, 2], [90, 2]],
            seed_size: 12,
            mapped: None,
        };

        assert_eq!(index.rrbs_fragment(0, 10, 20), Some((6, 37)));
        assert_eq!(index.rrbs_fragment(0, 40, 20), Some((41, 52)));
        assert_eq!(index.rrbs_fragment(1, 10, 20), None);
    }

    #[test]
    fn test_large_position_no_overflow() {
        // 验证大位置值（>16M bp，模拟 chr1 约 250M bp）不会溢出
        // 使用 hit2int 编码后，位置值应正确存储和检索
        let refs = vec![Reference {
            name: "large_chr".into(),
            // 创建一个足够长的序列，确保位置编码不会溢出
            seq: b"ACGT".repeat(10_000_000).to_vec(), // 40M bp
            len: 40_000_000,
        }];
        let coll = BinSeqCollection::from_references(&refs);

        // 验证 hit2int 对大位置值不会溢出
        // block.id = 0（chr0 正义链），位置 20_000_000（20M bp）
        let encoded = coll.hit2int(0, 20_000_000);
        // 确保编码值大于 24 位限制（16M = 0x100_0000）
        assert!(
            encoded > 0x100_0000,
            "编码值 {} 应大于 24 位限制 0x100_0000，大位置值不应溢出",
            encoded
        );

        // 同样验证 block.id = 1（chr0 反义链）
        let encoded_rc = coll.hit2int(1, 20_000_000);
        assert!(
            encoded_rc > 0x100_0000,
            "反义链编码值 {} 应大于 24 位限制",
            encoded_rc
        );

        // hit2int 使用 ref_anchor[chr/2] + loc，其中 chr/2 对正反链相同（都是 0），
        // 因此同一条染色体的正反链编码值相同。链信息通过 block.id 的奇偶性区分。
        assert_eq!(
            encoded, encoded_rc,
            "同一染色体的正反链编码值应相同（链信息通过 block.id 奇偶性区分）"
        );

        // 验证不同位置产生不同编码值
        let encoded_diff = coll.hit2int(0, 20_000_001);
        assert_ne!(
            encoded, encoded_diff,
            "不同位置的编码值应该不同"
        );
    }

    #[test]
    fn test_lookup_separated_fwd_rev() {
        // 验证 lookup_separated 正确分离正反链位置
        let refs = vec![Reference {
            name: "chr1".into(),
            seq: b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT".to_vec(),
            len: 64,
        }];
        let coll = BinSeqCollection::from_references(&refs);
        let index = KmerIndex::build_wgbs(&coll, 12, 4, 1.0);

        // Check hash 106288 (forward chain hits for ACGT repeating pattern)
        let hash = 106288u32;
        let entry = &index.index2[hash as usize];

        let (fwd, rev) = index.lookup_separated(hash);
        assert_eq!(fwd.len(), entry.n[1] as usize, "fwd count should match n[1]");
        assert_eq!(rev.len(), entry.n[0] as usize, "rev count should match n[0]");

        // Verify lookup returns combined
        let combined = index.lookup(hash);
        assert_eq!(
            combined.len(),
            fwd.len() + rev.len(),
            "lookup should return fwd + rev combined"
        );

        // Forward chain should have hits
        assert!(entry.n[1] > 0, "forward chain should have hits");
        assert_eq!(entry.n[1], 14, "forward chain should have 14 seed positions");

        // Total positions across all hashes should account for both chains
        let total_fwd: u32 = index.index2.iter().map(|e| e.n[1]).sum();
        let total_rev: u32 = index.index2.iter().map(|e| e.n[0]).sum();
        assert_eq!(total_fwd, 14, "total forward positions should be 14");
        assert_eq!(total_rev, 14, "total reverse positions should be 14");
        assert_eq!(index.positions.len(), 28, "total positions should be 28");
    }
}
