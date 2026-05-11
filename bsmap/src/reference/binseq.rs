//! 2-bit binary encoding of reference genome sequences.
//!
//! Each chromosome is encoded into u64 words (32 bases per word, 2 bits/base)
//! in both forward and reverse-complement orientations. All chromosomes are
//! then concatenated into two contiguous arrays for fast seed extraction.
//!
//! Mirrors C++ `RefSeq::BinSeq()`, `cBinSeq()`, `UnmaskRegion()`,
//! and `Run_ConvertBinseq()`.

use crate::alphabet::{ALPHABET, REG_ALPHABET, REV_ALPHABET};
use crate::param::{BINSEQPAD, REF_MARGIN, SEGLEN};

use super::fasta::Reference;

// ── Binary Sequence ───────────────────────────────────────────────────────────

/// One chromosome in 2-bit binary encoding.
#[derive(Debug, Clone)]
pub struct BinarySeq {
    /// Number of u64 words.
    pub n: u32,
    /// 2-bit packed sequence words.
    pub words: Vec<u64>,
    /// REG_ALPHABET mask words (0b11 for valid bases, 0 for N).
    pub mask: Vec<u64>,
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
    pub refcat: Vec<u64>,
    /// Concatenated reverse-complement binary sequences.
    pub crefcat: Vec<u64>,
    /// Anchor positions: ref_anchor[chr/2] + loc → flattened offset.
    /// Length = total_num + 1 (sentinel at end).
    pub ref_anchor: Vec<u32>,
    /// Unmasked regions sorted by (id, begin).
    pub blocks: Vec<Block>,
    /// Per-chromosome BinarySeq (for other consumers, not used in alignment).
    pub seqs: Vec<BinarySeq>,
}

impl BinSeqCollection {
    /// Build concatenated binary sequences from FASTA references.
    pub fn from_references(refs: &[Reference]) -> Self {
        let total_num = refs.len() as u32 * 2; // forward + RC for each
        let mut seqs: Vec<BinarySeq> = Vec::with_capacity(total_num as usize);
        let mut blocks: Vec<Block> = Vec::new();

        for (chr_idx, r) in refs.iter().enumerate() {
            let chr_id = chr_idx as u32;
            let total_bases = ((r.len + SEGLEN as u32 - 1) / SEGLEN as u32 + BINSEQPAD as u32)
                * SEGLEN as u32;

            // Forward strand
            let mut fwd = encode_forward(&r.seq);
            fwd.n = (r.len + SEGLEN as u32 - 1) / SEGLEN as u32 + BINSEQPAD as u32;
            pad_to_len(&mut fwd.words, fwd.n as usize);
            pad_to_len(&mut fwd.mask, fwd.n as usize);

            // Reverse-complement strand
            let mut rev = encode_revcomp(&r.seq);
            rev.n = fwd.n;
            pad_to_len(&mut rev.words, rev.n as usize);
            pad_to_len(&mut rev.mask, rev.n as usize);

            // Find unmasked regions on forward strand
            find_blocks(&mut blocks, chr_id * 2, &r.seq, total_bases);
            find_blocks_rc(&mut blocks, chr_id * 2 + 1, &r.seq, total_bases);

            seqs.push(fwd);
            seqs.push(rev);
        }

        // Sort blocks by (id, begin)
        blocks.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.begin.cmp(&b.begin)));

        // Compute anchor positions and concatenate
        let mut ref_anchor = Vec::with_capacity(total_num as usize + 1);
        ref_anchor.push((REF_MARGIN * SEGLEN as usize) as u32);

        let mut total_words: usize = 0;
        for i in 0..total_num as usize {
            total_words += seqs[i].n as usize;
            ref_anchor.push(((total_words + REF_MARGIN) * SEGLEN as usize) as u32);
        }

        let refcat_len = total_words + REF_MARGIN * 2;
        let mut refcat = vec![0u64; refcat_len];
        let mut crefcat = vec![0u64; refcat_len];

        let mut fwd_ptr = REF_MARGIN;
        let mut rev_ptr = REF_MARGIN;

        for i in 0..total_num as usize {
            let s = &seqs[i];
            if i % 2 == 0 {
                refcat[fwd_ptr..fwd_ptr + s.n as usize].copy_from_slice(&s.words[..s.n as usize]);
                fwd_ptr += s.n as usize;
            } else {
                crefcat[rev_ptr..rev_ptr + s.n as usize]
                    .copy_from_slice(&s.words[..s.n as usize]);
                rev_ptr += s.n as usize;
            }
        }

        let sum_length: u64 = refs.iter().map(|r| r.len as u64).sum();

        Self {
            total_num,
            sum_length,
            refcat,
            crefcat,
            ref_anchor,
            blocks,
            seqs,
        }
    }

    /// Map a (chr, loc) hit to a flattened integer offset.
    /// Matches C++ `RefSeq::hit2int()`.
    #[inline]
    pub fn hit2int(&self, chr: u32, loc: u32) -> u32 {
        self.ref_anchor[chr as usize / 2] + loc
    }
}

// ── Encoding ──────────────────────────────────────────────────────────────────

/// Encode a DNA byte slice in forward orientation (left-to-right, ALPHABET).
/// Pads leftover bases in the last word with A (0).
pub fn encode_forward(seq: &[u8]) -> BinarySeq {
    let n_words = (seq.len() + SEGLEN - 1) / SEGLEN;
    let mut words = vec![0u64; n_words];
    let mut mask = vec![0u64; n_words];

    for (i, chunk) in seq.chunks(SEGLEN).enumerate() {
        let mut w: u64 = 0;
        let mut m: u64 = 0;
        for &base in chunk {
            w = (w << 2) | ALPHABET[base as usize] as u64;
            m = (m << 2) | REG_ALPHABET[base as usize] as u64;
        }
        // Left-align: pad remaining positions with 0 (A code, mask 0)
        let used = chunk.len() * 2;
        w <<= SEGLEN * 2 - used;
        m <<= SEGLEN * 2 - used;
        words[i] = w;
        mask[i] = m;
    }

    BinarySeq {
        n: n_words as u32,
        words,
        mask,
    }
}

/// Encode in reverse-complement orientation (right-to-left, REV_ALPHABET).
pub fn encode_revcomp(seq: &[u8]) -> BinarySeq {
    let n_words = (seq.len() + SEGLEN - 1) / SEGLEN;
    let mut words = vec![0u64; n_words];
    let mut mask = vec![0u64; n_words];

    // Build RC bytes in natural order for chunking
    let rev_seq: Vec<u8> = seq.iter().rev().copied().collect();
    let rev_mask: Vec<u8> = seq
        .iter()
        .rev()
        .map(|&c| REG_ALPHABET[c as usize])
        .collect();

    for (i, (chunk, mask_chunk)) in rev_seq
        .chunks(SEGLEN)
        .zip(rev_mask.chunks(SEGLEN))
        .enumerate()
    {
        let mut w: u64 = 0;
        let mut m: u64 = 0;
        for (&base, &reg) in chunk.iter().zip(mask_chunk.iter()) {
            w = (w << 2) | REV_ALPHABET[base as usize] as u64;
            m = (m << 2) | reg as u64;
        }
        let used = chunk.len() * 2;
        w <<= SEGLEN * 2 - used;
        m <<= SEGLEN * 2 - used;
        words[i] = w;
        mask[i] = m;
    }

    BinarySeq {
        n: n_words as u32,
        words,
        mask,
    }
}

/// Pad word vector to exactly `target_len` (for BINSEQPAD).
fn pad_to_len(words: &mut Vec<u64>, target_len: usize) {
    while words.len() < target_len {
        words.push(0);
    }
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

/// Detect blocks on reverse-complement strand.
/// These come from the forward strand's RC counterpart,
/// generated by `find_blocks()` above.
fn find_blocks_rc(_blocks: &mut Vec<Block>, _id: u32, _seq: &[u8], _total_bases: u32) {
    // RC blocks are already created by find_blocks() as the cb_begin/cb_end
    // mirrored versions. Nothing extra to do here.
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
        // Mask: each base is valid (3=11), padded with 0
        let expected_mask: u64 = 0b11111111u64 << 56;
        assert_eq!(bs.mask[0], expected_mask);
    }

    #[test]
    fn test_encode_revcomp_simple() {
        let seq = b"ACGT"; // rev: TGCA → rev_alphabet: A(3) C(2) G(1) T(0) → 11 01 10 00
        let bs = encode_revcomp(seq);
        // TGCA = 11 01 10 00, left-aligned
        let expected: u64 = 0b11011000u64 << 56;
        assert_eq!(bs.words[0], expected);
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
        // ref_anchor should have total_num + 1 entries
        assert_eq!(coll.ref_anchor.len(), 5);
    }
}
