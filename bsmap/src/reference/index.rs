//! K-mer seed index for WGBS and RRBS alignment modes.
//!
//! Builds a hash-table index over all k-mers (seeds) in the reference
//! genome. For WGBS mode, uses `KmerLoc2` with flat position storage.
//! For RRBS mode, uses `KmerLoc` with per-site Hit storage.
//!
//! Mirrors C++ `RefSeq::InitialIndex()`, `CalKmerFreq()`, `AllocIndex()`,
//! `FillIndex()`, and `FinishIndex()`.

use crate::alphabet::make_seed;
use crate::param::{Hit, KmerLoc, KmerLoc2, REF_MARGIN, SEGLEN};
use crate::reference::binseq::{BinSeqCollection, Block};
use crate::reference::fasta::Reference;
use crate::reference::rrbs::{build_rrbs_index, DigestionSite};

/// Prefetch lookahead for frequency counting.
const PREFETCH_CAL_UNIT: usize = 8;
/// Prefetch lookahead for index filling.
const PREFETCH_CRT_UNIT: usize = 6;

/// 安全的种子提取：检查边界，避免越界访问。
/// 返回 None 如果位置超出 words 数组范围。
#[inline]
fn try_make_seed(words: &[u64], bit_pos: u32, seed_bits_lz: u32) -> Option<u32> {
    let word_idx = (bit_pos / (SEGLEN as u32 * 2)) as usize;
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
    /// RRBS index entries, None if not in RRBS mode.
    pub rrbs_index: Option<Vec<KmerLoc>>,
}

impl KmerIndex {
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
            &coll.refcat,
            &coll.crefcat,
            &coll.blocks,
            coll,
            index_interval,
            seed_size,
            seed_bits_lz,
            &mut fwd_counts,
            &mut rev_counts,
        );

        // ── Pass 2: Compute cutoff and prefix sums ────────────────────
        let mut total_counts: Vec<u32> = fwd_counts
            .iter()
            .zip(rev_counts.iter())
            .map(|(&f, &r)| f + r)
            .collect();

        let mut sorted_counts = total_counts.clone();
        sorted_counts.sort_unstable();

        let cutoff_idx = ((total_kmers as f64) * (1.0 - max_kmer_ratio)) as usize;
        let max_kmer_num = if cutoff_idx > 0 {
            sorted_counts[cutoff_idx.saturating_sub(1)]
        } else {
            u32::MAX
        };

        // Build index2 entries: n[1]=fwd count, n[0]=rev count
        let mut index2: Vec<KmerLoc2> = Vec::with_capacity(total_kmers as usize);
        let mut total_positions: u32 = 0;

        for i in 0..total_kmers as usize {
            let total = total_counts[i];
            if total > 0 && total <= max_kmer_num {
                // n[1] = forward count, n[0] = reverse count (C++ KmerLoc2 semantics)
                index2.push(KmerLoc2 {
                    n: [rev_counts[i], fwd_counts[i]],
                    loc1: Vec::new(),
                });
                total_positions += total;
            } else {
                index2.push(KmerLoc2 {
                    n: [0, 0],
                    loc1: Vec::new(),
                });
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
                let total = total_counts[i];
                if total > 0 && total <= max_kmer_num {
                    fwd_write_offsets[i] = running_offset;
                    rev_write_offsets[i] = running_offset + fwd_counts[i];
                    running_offset += fwd_counts[i] + rev_counts[i];
                }
                // For filtered-out k-mers, offsets remain 0 (unused)
            }
        }

        // Save start offsets for O(1) lookup
        let start_offsets = fwd_write_offsets.clone();

        // Fill forward chain positions (chain=0)
        fill_positions_chain(
            &coll.refcat,
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
            &coll.crefcat,
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

        Self {
            total_kmers,
            max_kmer_num,
            index2,
            positions,
            start_offsets,
            rrbs_index: None,
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
        let total_kmers = 3u32.pow(seed_size);

        // 解析消化位点
        let sites: Vec<DigestionSite> = digest_sites
            .iter()
            .filter_map(|s| DigestionSite::parse(s))
            .collect();

        if sites.is_empty() {
            return Self {
                total_kmers,
                max_kmer_num: u32::MAX,
                index2: Vec::new(),
                positions: Vec::new(),
                start_offsets: Vec::new(),
                rrbs_index: None,
            };
        }

        // 计算最大种子段数
        let max_seed_seg = ((max_insert / seed_size) + 2) as usize;

        // 对每条染色体构建 RRBS 索引，同时记录每个位置的染色体 ID
        let mut ccgg_index: Vec<Vec<Vec<(u32, u32)>>> = vec![Vec::new(); max_seed_seg];
        for seg in 0..max_seed_seg {
            ccgg_index[seg] = Vec::new();
        }

        for (chr_idx, r) in refs.iter().enumerate() {
            let chr_id = chr_idx as u32;
            // 计算反义链偏移量
            let total_bases = ((r.len + SEGLEN as u32 - 1) / SEGLEN as u32
                + crate::param::BINSEQPAD as u32)
                * SEGLEN as u32;
            let rc_offset = total_bases;

            let chr_ccgg = build_rrbs_index(
                &r.seq,
                chr_id,
                r.len,
                rc_offset,
                seed_size,
                max_seed_seg,
                &sites,
                min_insert,
                max_insert,
            );

            // 合并到全局 ccgg_index，同时记录染色体 ID
            for seg in 0..max_seed_seg {
                if seg < chr_ccgg.len() {
                    for chain in 0..2u32 {
                        if chain as usize >= chr_ccgg[seg].len() {
                            continue;
                        }
                        let block_id = chr_id * 2 + chain;
                        let positions: Vec<(u32, u32)> = chr_ccgg[seg][chain as usize]
                            .iter()
                            .map(|&pos| (block_id, pos))
                            .collect();
                        if chain as usize >= ccgg_index[seg].len() {
                            ccgg_index[seg].push(Vec::new());
                        }
                        ccgg_index[seg][chain as usize].extend(positions);
                    }
                }
            }
        }

        // 使用 ccgg_index 构建 KmerLoc 索引
        let mut rrbs_index: Vec<KmerLoc> = vec![
            KmerLoc {
                n1: 0,
                loc1: Vec::new(),
            };
            total_kmers as usize
        ];

        let seed_bits_lz = (SEGLEN as u32 - seed_size) * 2;

        // 遍历所有种子段和链，填充 KmerLoc
        for seg in 0..max_seed_seg {
            if seg >= ccgg_index.len() {
                break;
            }
            for chain in 0..2u32 {
                if chain as usize >= ccgg_index[seg].len() {
                    continue;
                }
                let entries = &ccgg_index[seg][chain as usize];
                for &(block_id, pos) in entries {
                    let words = if chain == 0 { &coll.refcat } else { &coll.crefcat };
                    let margin_offset = (REF_MARGIN * SEGLEN) as u32;
                    if let Some(hash) = try_make_seed(words, (pos + margin_offset) * 2, seed_bits_lz) {
                        if (hash as usize) < rrbs_index.len() {
                            rrbs_index[hash as usize].n1 += 1;
                            rrbs_index[hash as usize].loc1.push(Hit {
                                chr: block_id,
                                loc: pos,
                            });
                        }
                    }
                }
            }
        }

        Self {
            total_kmers,
            max_kmer_num: u32::MAX,
            index2: Vec::new(),
            positions: Vec::new(),
            start_offsets: Vec::new(),
            rrbs_index: Some(rrbs_index),
        }
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
        if self.index2.is_empty() || self.start_offsets.is_empty() {
            return (&[], &[]);
        }
        let idx = seed_hash as usize;
        if idx >= self.index2.len() {
            return (&[], &[]);
        }
        let entry = &self.index2[idx];
        let fwd_count = entry.n[1] as usize;
        let rev_count = entry.n[0] as usize;

        if fwd_count + rev_count == 0 {
            return (&[], &[]);
        }

        let start = self.start_offsets[idx] as usize;
        let fwd_slice = &self.positions[start..start + fwd_count];
        let rev_slice = &self.positions[start + fwd_count..start + fwd_count + rev_count];
        (fwd_slice, rev_slice)
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
    _coll: &BinSeqCollection,
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

        // Prefetch buffer
        let mut dbs = [0u32; PREFETCH_CAL_UNIT];

        // Fill initial prefetch window
        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        let margin_offset = (REF_MARGIN * SEGLEN) as u32;
        for j in 0..PREFETCH_CAL_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, (pos + margin_offset) * 2, seed_bits_lz);
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
                let next_hash = make_seed(words, (next_pos + margin_offset) * 2, seed_bits_lz);
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
/// `total_counts` and `max_kmer_num` are used to skip over-represented k-mers.
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

        let mut dbs = [0u32; PREFETCH_CRT_UNIT];

        let start_pos = (block.begin / index_interval) * index_interval;
        let mut ptr: u32 = 0;
        let mut pos = start_pos;
        let margin_offset = (REF_MARGIN * SEGLEN) as u32;
        for j in 0..PREFETCH_CRT_UNIT {
            if pos <= end_seedable {
                dbs[j] = make_seed(words, (pos + margin_offset) * 2, seed_bits_lz);
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
                let next_hash = make_seed(words, (next_pos + margin_offset) * 2, seed_bits_lz);
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
