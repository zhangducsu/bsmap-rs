//! 2-bit binary encoding of reference genome sequences.
//!
//! Each chromosome is encoded into u64 words (32 bases per word, 2 bits/base)
//! in both forward and reverse-complement orientations. All chromosomes are
//! then concatenated into two contiguous arrays for fast seed extraction.
//!
//! Mirrors C++ `RefSeq::BinSeq()`, `cBinSeq()`, `UnmaskRegion()`,
//! and `Run_ConvertBinseq()`.

use crate::alphabet::{ALPHABET, REV_ALPHABET};
use crate::param::{BINSEQPAD, REF_MARGIN, SEGLEN};

use super::fasta::Reference;
use super::storage::{BinSeqStorage, VecStorage};

// ── Binary Sequence ───────────────────────────────────────────────────────────

/// One chromosome in 2-bit binary encoding.
#[derive(Debug, Clone)]
pub struct BinarySeq {
    /// Number of u64 words.
    pub n: u32,
    /// 2-bit packed sequence words.
    pub words: Vec<u64>,
}

/// Unmasked (valid-nucleotide) region.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    /// Chromosome index × 2 + chain (0=forward, 1=RC).
    pub id: u32,
    /// Start position on reference (0-based, bp).
    pub begin: u32,
    /// End position on reference (exclusive, bp).
    pub end: u32,
}

/// Collection of all chromosomes in concatenated binary form.
///
/// This is the main data structure used by the alignment engine.
/// Forward sequences (`refcat`) and reverse-complement sequences (`crefcat`)
/// are stored in contiguous arrays for cache-friendly seed extraction.
pub struct BinSeqCollection {
    /// Number of chromosomes (×2 counting RC strands).
    pub total_num: u32,
    /// Total genome size in bases.
    pub sum_length: u64,
    /// Concatenated forward-strand binary sequences.
    pub refcat: Box<dyn BinSeqStorage>,
    /// Concatenated reverse-complement binary sequences.
    pub crefcat: Box<dyn BinSeqStorage>,
    /// Anchor positions: ref_anchor[chr/2] + loc → flattened offset.
    /// Length = total_num + 1 (sentinel at end).
    pub ref_anchor: Vec<u32>,
    /// FASTA 中每条染色体的真实长度，不含对齐和二进制存储 padding。
    pub chr_lengths: Vec<u32>,
    /// Unmasked regions sorted by (id, begin).
    pub blocks: Vec<Block>,
    /// Per-chromosome BinarySeq (for other consumers, not used in alignment).
    pub seqs: Vec<BinarySeq>,
    /// Chromosome names (from FASTA headers).
    pub chr_names: Vec<String>,
    /// Pre-computed chromosome accessions (first word of each name).
    /// P11-8: used by get_reference_name() to avoid per-record String allocation.
    pub chr_accessions: Vec<String>,
}

pub struct BinSeqCollectionBuilder {
    refcat: Vec<u64>,
    crefcat: Vec<u64>,
    ref_anchor: Vec<u32>,
    chr_lengths: Vec<u32>,
    blocks: Vec<Block>,
    chr_names: Vec<String>,
    sum_length: u64,
    cpp_padded_reverse: bool,
}

impl BinSeqCollectionBuilder {
    pub fn new() -> Self {
        Self {
            refcat: vec![0; REF_MARGIN],
            crefcat: vec![0; REF_MARGIN],
            ref_anchor: vec![(REF_MARGIN * SEGLEN) as u32],
            chr_lengths: Vec::new(),
            blocks: Vec::new(),
            chr_names: Vec::new(),
            sum_length: 0,
            cpp_padded_reverse: true,
        }
    }

    pub fn new_rrbs() -> Self {
        Self {
            cpp_padded_reverse: true,
            ..Self::new()
        }
    }

    pub fn push(&mut self, reference: &Reference) {
        let chr_id = self.chr_names.len() as u32;
        let words = (reference.len as usize + SEGLEN - 1) / SEGLEN + BINSEQPAD;
        let total_bases = (words * SEGLEN) as u32;

        let mut forward = encode_forward(&reference.seq).words;
        forward.resize(words, 0);
        self.refcat.extend_from_slice(&forward);
        drop(forward);

        let reverse = if self.cpp_padded_reverse {
            encode_revcomp_padded(&reference.seq, words)
        } else {
            let mut reverse = encode_revcomp(&reference.seq).words;
            reverse.resize(words, 0);
            reverse
        };
        self.crefcat.extend_from_slice(&reverse);

        find_blocks(&mut self.blocks, chr_id * 2, &reference.seq, total_bases);
        self.ref_anchor.push((self.refcat.len() * SEGLEN) as u32);
        self.chr_lengths.push(reference.len);
        self.chr_names.push(reference.name.clone());
        self.sum_length += reference.len as u64;
    }

    pub fn finish(mut self) -> BinSeqCollection {
        self.blocks
            .sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.begin.cmp(&b.begin)));
        self.refcat.resize(self.refcat.len() + REF_MARGIN, 0);
        self.crefcat.resize(self.crefcat.len() + REF_MARGIN, 0);
        let chr_accessions = self
            .chr_names
            .iter()
            .map(|name| name.split_whitespace().next().unwrap_or(name).to_string())
            .collect();
        BinSeqCollection {
            total_num: self.chr_names.len() as u32 * 2,
            sum_length: self.sum_length,
            refcat: Box::new(VecStorage::new(self.refcat)),
            crefcat: Box::new(VecStorage::new(self.crefcat)),
            ref_anchor: self.ref_anchor,
            chr_lengths: self.chr_lengths,
            blocks: self.blocks,
            seqs: Vec::new(),
            chr_names: self.chr_names,
            chr_accessions,
        }
    }
}

impl Default for BinSeqCollectionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BinSeqCollection {
    /// Build concatenated binary sequences from FASTA references.
    pub fn from_references(refs: &[Reference]) -> Self {
        let mut builder = BinSeqCollectionBuilder::new();
        for reference in refs {
            builder.push(reference);
        }
        builder.finish()
    }

    /// Map a (chr, loc) hit to a flattened integer offset.
    /// Matches C++ `RefSeq::hit2int()`.
    ///
    /// `chr` is the block.id (chr_idx * 2 + chain), where even = forward, odd = RC.
    /// The flat offset encodes both the strand (via refcat vs crefcat) and position.
    #[inline]
    pub fn hit2int(&self, chr: u32, loc: u32) -> u32 {
        let chr_idx = chr as usize / 2;
        let _chain = chr as usize % 2;
        // ref_anchor is indexed by chromosome (not by strand).
        // Forward chains (chain=0) use refcat offsets, RC chains (chain=1) use crefcat offsets.
        // Since refcat and crefcat are laid out identically (same anchor offsets),
        // we can use the same anchor but the caller must select refcat vs crefcat.
        self.ref_anchor[chr_idx] + loc
    }

    /// Reverse map: flattened integer offset to (chr, loc).
    /// Matches C++ `RefSeq::int2hit()`.
    ///
    /// Returns (chr, loc) where chr is the chromosome index (not block.id).
    /// The caller is responsible for determining the strand.
    #[inline]
    pub fn int2hit(&self, pos: u32) -> (u32, u32) {
        // 找到对应的染色体
        if self.ref_anchor.is_empty() {
            return (0, pos);
        }
        // Handle pos < ref_anchor[0] (shouldn't happen with valid positions)
        if pos < self.ref_anchor[0] {
            return (0, pos);
        }
        for (i, &anchor) in self.ref_anchor.iter().enumerate().skip(1) {
            if pos < anchor {
                let chr = (i - 1) as u32;
                let loc = pos - self.ref_anchor[i - 1];
                return (chr, loc);
            }
        }
        // 如果超出范围，返回最后一个染色体
        let last_idx = self.ref_anchor.len().saturating_sub(2);
        let chr = last_idx as u32;
        let loc = pos - self.ref_anchor[last_idx];
        (chr, loc)
    }

    /// 获取指定染色体的反向链总长度（以碱基为单位）。
    ///
    /// 对应 C++ 的 `ref.title[chr].rc_offset`。
    /// 用于反向链位置翻转：`flipped_loc = rc_offset - read_len - loc`
    pub fn total_len_for_chr(&self, chr_idx: usize) -> u32 {
        // C++: rc_offset = ((length + SEGLEN - 1) / SEGLEN + BINSEQPAD) * SEGLEN
        // ref_anchor[chr_idx+1] - ref_anchor[chr_idx] 包含了 margin，
        // 所以需要减去 margin
        if chr_idx + 1 < self.ref_anchor.len() {
            self.ref_anchor[chr_idx + 1] - self.ref_anchor[chr_idx]
        } else {
            // 最后一个染色体
            self.ref_anchor[chr_idx] // fallback
        }
    }

    /// 从正向 reference 即时生成 C++ padded reverse-chain 的局部窗口。
    pub fn fill_reverse_window(
        &self,
        chr_idx: usize,
        reverse_start: u32,
        base_len: u32,
        output: &mut [u64],
    ) -> bool {
        output.fill(0);
        let Some(&chr_len) = self.chr_lengths.get(chr_idx) else {
            return false;
        };
        if chr_idx + 1 >= self.ref_anchor.len() {
            return false;
        }
        let anchor = self.ref_anchor[chr_idx];
        let rc_offset = self.ref_anchor[chr_idx + 1] - anchor;
        let leading_padding = rc_offset.saturating_sub(chr_len);
        let forward = self.refcat.as_slice();
        let output_bases = base_len.min((output.len() * SEGLEN) as u32);

        for output_pos in 0..output_bases {
            let Some(reverse_pos) = reverse_start.checked_add(output_pos) else {
                return false;
            };
            let code = if reverse_pos < leading_padding {
                3u64
            } else if reverse_pos < rc_offset {
                let source_pos = rc_offset - 1 - reverse_pos;
                let flat_pos = anchor + source_pos;
                let word_index = flat_pos as usize / SEGLEN;
                let bit_offset = (flat_pos as usize % SEGLEN) * 2;
                let Some(&word) = forward.get(word_index) else {
                    return false;
                };
                3 - ((word >> (62 - bit_offset)) & 0b11)
            } else if chr_idx + 1 < self.chr_lengths.len() {
                // 下一个 chromosome 以至少 BINSEQPAD 个 T-coded reverse padding words 开头。
                3u64
            } else {
                // 最后一个 chromosome 后是 REF_MARGIN 个零 word。
                0u64
            };
            let word_index = output_pos as usize / SEGLEN;
            let bit_offset = (output_pos as usize % SEGLEN) * 2;
            output[word_index] |= code << (62 - bit_offset);
        }
        true
    }
}

// ── Encoding ──────────────────────────────────────────────────────────────────

/// Encode a DNA byte slice in forward orientation (left-to-right, ALPHABET).
/// Pads leftover bases in the last word with A (0).
pub fn encode_forward(seq: &[u8]) -> BinarySeq {
    let n_words = (seq.len() + SEGLEN - 1) / SEGLEN;
    let mut words = vec![0u64; n_words];

    for (i, chunk) in seq.chunks(SEGLEN).enumerate() {
        let mut w: u64 = 0;
        for &base in chunk {
            w = (w << 2) | ALPHABET[base as usize] as u64;
        }
        let used = chunk.len() * 2;
        w <<= SEGLEN * 2 - used;
        words[i] = w;
    }

    BinarySeq {
        n: n_words as u32,
        words,
    }
}

/// Encode in reverse-complement orientation (right-to-left, REV_ALPHABET).
pub fn encode_revcomp(seq: &[u8]) -> BinarySeq {
    let n_words = (seq.len() + SEGLEN - 1) / SEGLEN;
    let mut words = vec![0u64; n_words];

    let rev_seq: Vec<u8> = seq.iter().rev().copied().collect();

    for (i, chunk) in rev_seq.chunks(SEGLEN).enumerate() {
        let mut w: u64 = 0;
        for &base in chunk {
            w = (w << 2) | REV_ALPHABET[base as usize] as u64;
        }
        let used = chunk.len() * 2;
        w <<= SEGLEN * 2 - used;
        words[i] = w;
    }

    BinarySeq {
        n: n_words as u32,
        words,
    }
}

/// Match C++ `cBinSeq()`: reverse-complement the sequence after forward
/// storage has been padded to its complete word count. Padding therefore
/// appears before the reverse-complemented biological sequence.
fn encode_revcomp_padded(seq: &[u8], word_count: usize) -> Vec<u64> {
    let total_bases = word_count * SEGLEN;
    let leading_padding = total_bases.saturating_sub(seq.len());
    let mut words = vec![0u64; word_count];

    for position in 0..total_bases {
        let code = if position < leading_padding {
            REV_ALPHABET[b'N' as usize]
        } else {
            let source = total_bases - 1 - position;
            REV_ALPHABET[seq[source] as usize]
        };
        let word = position / SEGLEN;
        words[word] = (words[word] << 2) | code as u64;
    }
    words
}

// ── Unmasked Region Detection ─────────────────────────────────────────────────

const USEFUL_NT: &[u8] = b"ACGTacgt";
const NX_NT: &[u8] = b"NXnx";

/// Find unmasked (valid nucleotide) regions on forward strand.
/// Mirrors C++ `RefSeq::UnmaskRegion()`.
fn find_blocks(blocks: &mut Vec<Block>, id: u32, seq: &[u8], total_bases: u32) {
    let len = seq.len() as u32;
    let mut begin: u32 = 0;

    loop {
        // Find next useful nucleotide
        begin = match seq[begin as usize..]
            .iter()
            .position(|c| USEFUL_NT.contains(c))
        {
            Some(p) => begin + p as u32,
            None => break,
        };

        if begin > len {
            break;
        }

        // Find next N/non-standard base
        let end = match seq[begin as usize..]
            .iter()
            .position(|c| NX_NT.contains(c))
        {
            Some(p) => (begin + p as u32).min(len),
            None => len,
        };

        if end - begin < 30 {
            begin = end;
            continue;
        }

        // Merge with previous block if adjacent
        if let Some(last) = blocks.last_mut() {
            if last.id == id && begin - last.end < 5 {
                last.end = end;
                begin = end;
                continue;
            }
        }

        // Forward block
        blocks.push(Block { id, begin, end });

        // RC block
        let cb_begin = total_bases - end;
        let cb_end = total_bases - begin;
        blocks.push(Block {
            id: id + 1,
            begin: cb_begin,
            end: cb_end,
        });

        begin = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_forward_simple() {
        let seq = b"ACGT"; // A=00, C=01, G=10, T=11
        let bs = encode_forward(seq);
        // ACGT = 00 01 10 11, left-aligned: needs 32-4=28 bases padding (56 bits)
        let expected: u64 = 0b00011011u64 << 56;
        assert_eq!(bs.words[0], expected);
    }

    #[test]
    fn test_encode_revcomp_simple() {
        let seq = b"ACGT";
        // 反向互补: ACGT → 反向 TGCA → REV_ALPHABET: T(0) G(1) C(2) A(3)
        // 编码: 00 01 10 11, left-aligned to 64 bits
        let bs = encode_revcomp(seq);
        let expected: u64 = 0b00011011u64 << 56;
        assert_eq!(bs.words[0], expected);
    }

    #[test]
    fn test_encode_revcomp_padded_matches_cpp_layout() {
        let words = encode_revcomp_padded(b"ACGT", 3);
        assert_eq!(words, vec![u64::MAX, u64::MAX, 0xffff_ffff_ffff_ff1b]);
    }

    #[test]
    fn generated_reverse_windows_match_materialized_crefcat() {
        let refs = vec![
            Reference {
                name: "chr1".to_string(),
                seq: b"ACGTNACGTACGTACGTACGTACGTACGTACGT".to_vec(),
                len: 34,
            },
            Reference {
                name: "chr2".to_string(),
                seq: b"TGCAACG".to_vec(),
                len: 7,
            },
        ];
        let collection = BinSeqCollection::from_references(&refs);
        let crefcat = collection.crefcat.as_slice();

        for chr_idx in 0..refs.len() {
            let anchor = collection.ref_anchor[chr_idx];
            let rc_offset = collection.ref_anchor[chr_idx + 1] - anchor;
            let leading_padding = rc_offset - collection.chr_lengths[chr_idx];
            let starts = [
                0,
                leading_padding.saturating_sub(1),
                leading_padding,
                rc_offset.saturating_sub(5),
                rc_offset.saturating_sub(1),
            ];
            for reverse_start in starts {
                let mut generated = [0u64; 2];
                assert!(collection.fill_reverse_window(chr_idx, reverse_start, 40, &mut generated));
                for offset in 0..40u32 {
                    let generated_word = generated[offset as usize / SEGLEN];
                    let generated_shift = 62 - (offset as usize % SEGLEN) * 2;
                    let generated_code = (generated_word >> generated_shift) & 0b11;
                    let flat_pos = anchor + reverse_start + offset;
                    let expected_word = crefcat[flat_pos as usize / SEGLEN];
                    let expected_shift = 62 - (flat_pos as usize % SEGLEN) * 2;
                    let expected_code = (expected_word >> expected_shift) & 0b11;
                    assert_eq!(
                        generated_code, expected_code,
                        "chr={chr_idx}, reverse_start={reverse_start}, offset={offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_all_builders_use_cpp_reverse_padding() {
        let reference = Reference {
            name: "chr1".into(),
            seq: b"ACGT".to_vec(),
            len: 4,
        };
        for mut builder in [BinSeqCollectionBuilder::new(), BinSeqCollectionBuilder::new_rrbs()] {
            builder.push(&reference);
            let collection = builder.finish();
            assert_eq!(
                &collection.crefcat.as_slice()[REF_MARGIN..REF_MARGIN + 3],
                &[u64::MAX, u64::MAX, 0xffff_ffff_ffff_ff1b]
            );
        }
    }

    #[test]
    fn test_encode_roundtrip_via_pack() {
        // Verify encode_forward matches alphabet::pack_forward
        let seq = b"ACGTACGTACGTACGTACGTACGTACGTACGT"; // exactly 32 bases
        let bs = encode_forward(seq);
        let packed = crate::alphabet::pack_forward(seq, 1);
        assert_eq!(bs.words[0], packed[0]);
    }

    #[test]
    fn test_binseq_collection_construction() {
        let refs = vec![
            Reference {
                name: "chr1".into(),
                seq: b"ACGTACGTACGTACGTACGTACGTACGTACGT".to_vec(),
                len: 32,
            },
            Reference {
                name: "chr2".into(),
                seq: b"TGCA".to_vec(),
                len: 4,
            },
        ];
        let coll = BinSeqCollection::from_references(&refs);
        assert_eq!(coll.total_num, 4); // 2 chr × 2 strands
        assert_eq!(coll.sum_length, 36);
        assert_eq!(coll.chr_lengths, vec![32, 4]);
        // P12: ref_anchor is per-chromosome (not per-strand), so length = num_chr + 1
        assert_eq!(coll.ref_anchor.len(), 3);
    }
}
