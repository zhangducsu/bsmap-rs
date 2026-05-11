//! K-mer seed index for WGBS and RRBS alignment modes.
//!
//! Builds a hash-table index over all k-mers (seeds) in the reference
//! genome. For WGBS mode, uses `KmerLoc2` with flat position storage.
//! For RRBS mode, uses `KmerLoc` with per-site Hit storage.
//!
//! Mirrors C++ `RefSeq::InitialIndex()`, `CalKmerFreq()`, `AllocIndex()`,
//! `FillIndex()`, and `FinishIndex()`.

use crate::alphabet::make_seed;
use crate::param::{Hit, KmerLoc, KmerLoc2, SEGLEN};
use crate::reference::binseq::{BinSeqCollection, Block};

/// Prefetch lookahead for frequency counting.
const PREFETCH_CAL_UNIT: usize = 8;
/// Prefetch lookahead for index filling.
const PREFETCH_CRT_UNIT: usize = 6;

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

    // ── RRBS mode ──────────────────────────────────────────────────────
    /// RRBS index entries, None if not in RRBS mode.
    pub rrbs_index: Option<Vec<KmerLoc>>,
}

impl KmerIndex {
    /// Build the k-mer index for WGBS mode.
    ///
    /// Three-pass algorithm:
    /// 1. Count k-mer frequencies across all unmasked blocks
    /// 2. Allocate storage (compute prefix sums, apply frequency cutoff)
    /// 3. Fill positions
    pub fn build_wgbs(
        coll: &BinSeqCollection,
        seed_size: u32,
        index_interval: u32,
        max_kmer_ratio: f64,
    ) -> Self {
        let total_kmers = 3u32.pow(seed_size);
        let seed_bits_lz = (SEGLEN as u32 - seed_size) * 2;

        // ── Pass 1: Count frequencies ──────────────────────────────────
        let mut counts = vec![0u32; total_kmers as usize];

        for chain in 0..2u32 {
            count_frequencies(
                &coll.refcat,
                &coll.crefcat,
                &coll.blocks,
                chain,
                index_interval,
                seed_size,
                seed_bits_lz,
                &mut counts,
            );
        }

        // ── Pass 2: Compute cutoff and prefix sums ────────────────────
        let mut sorted_counts = counts.clone();
        sorted_counts.sort_unstable();

        let cutoff_idx = ((total_kmers as f64) * (1.0 - max_kmer_ratio)) as usize;
        let max_kmer_num = if cutoff_idx > 0 {
            sorted_counts[cutoff_idx.saturating_sub(1)]
        } else {
            u32::MAX
        };

        // Build index2 entries and compute offsets
        let mut index2: Vec<KmerLoc2> = Vec::with_capacity(total_kmers as usize);
        let mut total_positions: u32 = 0;

        for &count in &counts {
            let (n0, n1) = if count <= max_kmer_num {
                (count, total_positions) // valid k-mer: store positions
            } else {
                (0, 0) // over-represented: skip
            };
            index2.push(KmerLoc2 {
                n: [n0, 0],
                loc1: Vec::new(), // filled in pass 3 via direct write
            });
            total_positions += n0;
        }

        // ── Pass 3: Fill positions ────────────────────────────────────
        let mut positions: Vec<u32> = vec![0u32; total_positions as usize];
        let mut write_offsets: Vec<u32> = vec![0u32; total_kmers as usize];

        // Initialize write offsets from prefix sums
        let mut offset: u32 = 0;
        for (i, _) in counts.iter().enumerate() {
            write_offsets[i] = offset;
            if counts[i] <= max_kmer_num {
                offset += counts[i];
            }
        }

        for chain in 0..2u32 {
            fill_positions(
                &coll.refcat,
                &coll.crefcat,
                &coll.blocks,
                chain,
                index_interval,
                seed_size,
                seed_bits_lz,
                &mut positions,
                &mut write_offsets,
            );
        }

        // Copy write offset info into index2 entries
        let mut running_start = 0u32;
        for (i, entry) in index2.iter_mut().enumerate() {
            if counts[i] <= max_kmer_num && counts[i] > 0 {
                entry.n = [counts[i], running_start];
                running_start += counts[i];
            }
        }

        Self {
            total_kmers,
            max_kmer_num,
            index2,
            positions,
            rrbs_index: None,
        }
    }

    /// Build RRBS mode index.
    ///
    /// Uses CCGG positions from RRBS module instead of scanning all blocks.
    pub fn build_rrbs(
        _coll: &BinSeqCollection,
        _seed_size: u32,
        _index_interval: u32,
        _ccgg_index: &[Vec<Vec<u32>>],
    ) -> Self {
        // RRBS index building follows a different path — allocated and filled
        // per CCGG site rather than per genomic position.
        // This will be fully implemented when we wire up the alignment engine.

        let total_kmers = 3u32.pow(_seed_size);
        Self {
            total_kmers,
            max_kmer_num: u32::MAX,
            index2: Vec::new(),
            positions: Vec::new(),
            rrbs_index: None,
        }
    }

    /// Look up k-mer seed in WGBS index.
    #[inline]
    pub fn lookup(&self, seed_hash: u32) -> &[u32] {
        let entry = &self.index2[seed_hash as usize];
        if entry.n[0] == 0 {
            return &[];
        }
        let start = entry.n[1] as usize;
        let end = start + entry.n[0] as usize;
        &self.positions[start..end]
    }
}

// ── Frequency Counting (Pass 1) ───────────────────────────────────────────────

fn count_frequencies(
    refcat: &[u64],
    crefcat: &[u64],
    blocks: &[Block],
    chain: u32,
    index_interval: u32,
    seed_size: u32,
    seed_bits_lz: u32,
    counts: &mut [u32],
) {
    let prefetch = PREFETCH_CAL_UNIT as u32 * index_interval;

    for block in blocks {
        if block.id % 2 != chain {
            continue;
        }

        // Determine which concatenated array to use
        let words = if block.id % 2 == 0 { refcat } else { crefcat };

        let end_seedable = if block.end >= seed_size {
            ((block.end - seed_size) / index_interval) * index_interval
        } else {
            continue; // block too short for seed
        };

        // Prefetch buffer
        let mut dbs = [0u32; PREFETCH_CAL_UNIT];

        // Fill initial prefetch window
        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        for j in 0..PREFETCH_CAL_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, pos, seed_bits_lz);
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
                let next_hash = make_seed(words, next_pos, seed_bits_lz);
                dbs[((ptr + PREFETCH_CAL_UNIT as u32) % PREFETCH_CAL_UNIT as u32)
                    as usize] = next_hash;
            }

            pos += index_interval;
            ptr += 1;
        }
    }
}

// ── Position Filling (Pass 3) ─────────────────────────────────────────────────

fn fill_positions(
    refcat: &[u64],
    crefcat: &[u64],
    blocks: &[Block],
    chain: u32,
    index_interval: u32,
    seed_size: u32,
    seed_bits_lz: u32,
    positions: &mut [u32],
    write_offsets: &mut [u32],
) {
    let prefetch = PREFETCH_CRT_UNIT as u32 * index_interval;

    for block in blocks {
        if block.id % 2 != chain {
            continue;
        }

        let words = if block.id % 2 == 0 { refcat } else { crefcat };

        let end_seedable = if block.end >= seed_size {
            ((block.end - seed_size) / index_interval) * index_interval
        } else {
            continue;
        };

        let mut dbs = [0u32; PREFETCH_CRT_UNIT];

        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        for j in 0..PREFETCH_CRT_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, pos, seed_bits_lz);
                pos += index_interval;
            }
            ptr += 1;
        }

        pos = start_pos;
        let chr_id = block.id;
        while pos <= end_seedable {
            let hash = dbs[(ptr % PREFETCH_CRT_UNIT as u32) as usize] as usize;
            let offset = write_offsets[hash] as usize;
            // Encode (chr, loc) as u32: ref_anchor[chr/2] + loc
            // Simplified: use (chr_id << 24) | loc for compact storage
            positions[offset] = (chr_id << 24) | pos;
            write_offsets[hash] += 1;

            let next_pos = pos + prefetch;
            if next_pos <= end_seedable {
                let next_hash = make_seed(words, next_pos, seed_bits_lz);
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
}
