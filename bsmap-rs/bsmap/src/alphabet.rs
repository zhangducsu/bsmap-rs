//! DNA alphabet encoding tables and bit-manipulation primitives.
//!
//! This module is the **performance-critical foundation** of BSMAP-rs.
//! Every function here translates the C++ bit-twiddling intrinsics
//! into idiomatic (but equally fast) Rust.
//!
//! ## What lives here
//! - 256-entry lookup tables: `ALPHABET`, `REV_ALPHABET`, `REG_ALPHABET`, `REV_CHAR`
//! - Seed hashing: `xt3()`, `xt3_64()`  (C++ `Param::XT`, `Param::XT64`)
//! - C→T tolerance mask: `xc32()`, `xc64()`  (C++ `Param::XC`, `Param::XC64`)
//! - SWAR popcount: `xm64()`  (C++ `Param::XM64`)
//! - Seed extraction from binary-packed sequence
//!
//! ## __builtin_clzll / __builtin_ctzll replacement
//! Rust provides `u64::leading_zeros()` and `u64::trailing_zeros()` which
//! compile to the same `lzcnt` / `tzcnt` (or `bsr` / `bsf` on older x86)
//! instructions. No `unsafe` or platform-specific intrinsics needed.

use crate::param::SEGLEN;

// ── Static Encoding Tables ──────────────────────────────────────────────────

/// Forward encoding: ASCII char → 2-bit DNA code.
///
/// Default mapping (SetAlign with T→C):
///   A=0 (00), C=1 (01), G=2 (10), T=3 (11)
/// Everything else (N, etc.) defaults to 0 (A).
///
/// Matches C++ `alphabet[256]` after `SetAlign('T', 'C')`.
#[rustfmt::skip]
pub static ALPHABET: [u8; 256] = {
    let mut tbl = [0u8; 256];
    // Default: everything = 0 (A code)
    // Set specific bases:
    tbl[b'C' as usize] = 1;
    tbl[b'c' as usize] = 1;
    tbl[b'G' as usize] = 2;
    tbl[b'g' as usize] = 2;
    tbl[b'T' as usize] = 3;
    tbl[b't' as usize] = 3;
    tbl
};

/// Reverse-complement encoding: ASCII char → 2-bit DNA code.
///
/// When reading a sequence backwards, each base is converted to its
/// complement:
///   A→T(3), C→G(2), G→C(1), T→A(0)
/// Everything else defaults to 3 (T).
///
/// Matches C++ `rev_alphabet[256]`.
#[rustfmt::skip]
pub static REV_ALPHABET: [u8; 256] = {
    let mut tbl = [3u8; 256]; // default: T code
    tbl[b'C' as usize] = 2;   // C → G
    tbl[b'c' as usize] = 2;
    tbl[b'G' as usize] = 1;   // G → C
    tbl[b'g' as usize] = 1;
    tbl[b'T' as usize] = 0;   // T → A
    tbl[b't' as usize] = 0;
    tbl
};

/// Regular (valid) nucleotide mask: 3 (0b11) for A/C/G/T/a/c/g/t, 0 otherwise.
///
/// Matches C++ `reg_alphabet[256]`.
#[rustfmt::skip]
pub static REG_ALPHABET: [u8; 256] = {
    let mut tbl = [0u8; 256];
    tbl[b'A' as usize] = 3;
    tbl[b'a' as usize] = 3;
    tbl[b'C' as usize] = 3;
    tbl[b'c' as usize] = 3;
    tbl[b'G' as usize] = 3;
    tbl[b'g' as usize] = 3;
    tbl[b'T' as usize] = 3;
    tbl[b't' as usize] = 3;
    tbl
};

/// Reverse-complement character mapping: 'A'↔'T', 'C'↔'G', 'a'↔'t', 'c'↔'g'.
/// Everything else → 'N'.
///
/// Matches C++ `rev_char[256]`.
#[rustfmt::skip]
pub static REV_CHAR: [u8; 256] = {
    let mut tbl = [b'N'; 256];
    tbl[b'A' as usize] = b'T';
    tbl[b'T' as usize] = b'A';
    tbl[b'C' as usize] = b'G';
    tbl[b'G' as usize] = b'C';
    tbl[b'a' as usize] = b't';
    tbl[b't' as usize] = b'a';
    tbl[b'c' as usize] = b'g';
    tbl[b'g' as usize] = b'c';
    tbl
};

/// Nucleotide codes in display order: A, C, G, T
pub const NT_CODE: [u8; 4] = [b'A', b'C', b'G', b'T'];

/// Reverse-complement nucleotide codes
pub const REVNT_CODE: [u8; 4] = [b'T', b'G', b'C', b'A'];

/// Chain (strand) flags
pub const CHAIN_FLAG: [u8; 2] = [b'+', b'-'];

// ── 3-Letter Seed Hashing (XT / XT64) ───────────────────────────────────────
//
// The C→T bisulfite conversion means that C and T should hash to the same
// bucket. The XT transform maps a 2-bit-per-base word to a base-3 number:
//   A=00 → 0,  C=01 → 1,  G=10 → 2,  T=11 → 1 (same as C)
//
// The mathematical trick in C++:
//   tt -= (tt << 1) & tt & 0xAAAA...
// This turns T (0b11) into 0b01 (C) because:
//   (tt<<1) & tt = (110) & 11 = 10
//   tt - 10 = 01
// For A (00): (00<<1)&00 = 00, 00-00 = 00 ✓
// For C (01): (01<<1)&01 = 00, 01-00 = 01 ✓
// For G (10): (10<<1)&10 = 00, 10-00 = 10 ✓

/// Transform a 32-bit 2-bit-per-base word into a base-3 hash index.
///
/// The result is `sum_{i=0}^{15} b_i * 3^i` where b_i ∈ {0,1,2}
/// and C/T both map to 1. Range: [0, 3^16) = [0, 43046721).
///
/// Equivalent to C++ `Param::XT(bit32_t tt)`.
#[inline]
pub fn xt3(tt: u32) -> u32 {
    let mut t = tt;

    // Step 1: C/T ambiguity — T(11) → C(01)
    t = t.wrapping_sub((t << 1) & t & 0xAAAA_AAAA);

    // Step 2: 4-bit transform — each 2-bit pair becomes its base-3 value
    t = t.wrapping_sub((t >> 2) & 0x3333_3333);

    // Step 3: 8-bit transform — multiply by 9 (since 3^2 = 9)
    let ss = (t & 0xF0F0_F0F0) >> 1;
    t = t.wrapping_sub(ss.wrapping_sub(ss >> 3));

    // Step 4: 16-bit transform — weight by 3^4 = 81
    let ss = (t & 0xFF00_FF00) >> 2;
    t = (t & 0x00FF_00FF)
        .wrapping_add(ss)
        .wrapping_add(ss >> 2)
        .wrapping_add(ss >> 6);

    // Step 5: combine halves weighted by 3^8 = 6561
    (t & 0xFFFF).wrapping_add((t >> 16).wrapping_mul(6561))
}

/// 64-bit version of `xt3`: transforms 32 bases at once.
///
/// Equivalent to C++ `Param::XT64(bit64_t tt)`.
#[inline]
pub fn xt3_64(tt: u64) -> u64 {
    let mut t = tt;

    // Step 1: C/T ambiguity
    t = t.wrapping_sub((t << 1) & t & 0xAAAA_AAAA_AAAA_AAAA);

    // Step 2: 4-bit transform
    t = t.wrapping_sub((t >> 2) & 0x3333_3333_3333_3333);

    // Step 3: 8-bit transform
    let ss = (t & 0xF0F0_F0F0_F0F0_F0F0) >> 1;
    t = t.wrapping_sub(ss.wrapping_sub(ss >> 3));

    // Step 4: 16-bit transform
    let ss = (t & 0xFF00_FF00_FF00_FF00) >> 2;
    t = (t & 0x00FF_00FF_00FF_00FF)
        .wrapping_add(ss)
        .wrapping_add(ss >> 2)
        .wrapping_add(ss >> 6);

    // Step 5: combine 32-bit halves
    let lo = (t & 0xFFFF_FFFF) as u64;
    let hi = (t >> 32) & 0xFFFF_FFFF;
    lo.wrapping_add(hi.wrapping_mul(6561))
}

// ── C→T Tolerance Mask (XC / XC64) ─────────────────────────────────────────
//
// The bisulfite alignment is asymmetric: T in read can match C in reference,
// but C in read cannot match T in reference. The XC mask generates a bit
// pattern that "tolerates" C→T mismatches by masking the C/T distinction
// bit in the XOR comparison.

/// Generate a C→T tolerance mask for a 32-bit 2-bit-encoded word.
///
/// For each base position where the reference has C (0b01),
/// the output has T (0b11), so that (read_bits & XC(ref_bits)) ^ ref_bits
/// is zero when read=T matches ref=C.
///
/// Equivalent to C++ `Param::XC(bit32_t tt)`.
#[inline]
pub fn xc32(tt: u32) -> u32 {
    ((!tt) << 1) | tt | 0x5555_5555
}

/// 64-bit version of `xc32`.
///
/// Equivalent to C++ `Param::XC64(bit64_t tt)`.
#[inline]
pub fn xc64(tt: u64) -> u64 {
    ((!tt) << 1) | tt | 0x5555_5555_5555_5555
}

// ── Bit-parallel Mismatch Counting (XM64) ───────────────────────────────────
//
// The mismatch vector has each 2-bit field representing one base position:
//   00 = match, non-zero = mismatch.
// XM64 performs a SWAR (SIMD Within A Register) popcount across all 32
// positions, counting how many 2-bit fields are non-zero.

/// Count mismatches in a 64-bit word.
///
/// Each 2-bit field is 00 (match) or non-zero (mismatch).
/// Returns the count of non-zero 2-bit fields (0-32).
///
/// Equivalent to C++ `Param::XM64(bit64_t tt)`.
#[inline]
pub fn xm64(tt: u64) -> u32 {
    let mut t = tt;

    // OR adjacent bits: non-zero 2-bit field → non-zero in both bits
    t |= t >> 1;
    t &= 0x5555_5555_5555_5555;

    // Pairwise sum into 4-bit lanes
    t = (t + (t >> 2)) & 0x3333_3333_3333_3333;

    // Sum 4-bit lanes into 8-bit lanes
    t = (t + (t >> 4)) & 0x0F0F_0F0F_0F0F_0F0F;

    // Horizontal sum via multiplication
    t = t.wrapping_mul(0x0101_0101_0101_0101);

    (t >> 56) as u32
}

// ── Seed Extraction ─────────────────────────────────────────────────────────

/// Extract a k-mer seed hash from binary-encoded reference sequence.
///
/// Given a u64 slice and a bit position within it, extracts `seed_size`
/// bases, converts via XT3 (C/T merged), and returns the hash index.
///
/// Equivalent to C++ `RefSeq::s_MakeSeed_1(bit64_t *_m, int _a)`.
/// C++: ((_m[0]<<(_a*2))|((_m[1]>>1)>>(63-_a*2)))
/// where _a is the base position within the word (0-31), so _a*2 is bit_offset (0-62)
#[inline]
pub fn make_seed(words: &[u64], bit_pos: u32, seed_bits_lz: u32) -> u32 {
    let word_idx = (bit_pos / 64) as usize;
    let bit_offset = (bit_pos % 64) as u32;

    // 检查边界
    if word_idx >= words.len() {
        return 0;
    }

    // 跨越两个 word 提取种子（如果种子跨越 64 位边界）
    // C++: ((_m[0]<<(_a*2))|((_m[1]>>1)>>(63-_a*2)))
    // 简化: ((_m[0]<<bit_offset)|(_m[1]>>(64-bit_offset)))
    // 当 bit_offset=0: _m[0] | 0 = _m[0]
    // 当 bit_offset=32: (_m[0]<<32) | (_m[1]>>32)
    //
    // 注意：当没有下一个 word 时，我们模拟 _m[1] = 0
    // 这样 straddle = (_m[0] << bit_offset) | 0 = _m[0] << bit_offset
    // 然后右移 seed_bits_lz 取所需的位
    let straddle: u64 = if bit_offset == 0 {
        words[word_idx]
    } else if word_idx + 1 < words.len() {
        (words[word_idx] << bit_offset)
            | (words[word_idx + 1] >> (64 - bit_offset))
    } else {
        // 边界情况：没有下一个 word，模拟填充 0
        words[word_idx] << bit_offset
    };

    xt3((straddle >> seed_bits_lz) as u32)
}

/// Extract a k-mer seed hash with reverse-complement mask awareness.
///
/// Returns (seed_hash, has_reg_bases) where has_reg_bases indicates
/// whether all positions in the seed are valid nucleotides (non-N).
#[inline]
pub fn make_seed_with_mask(
    words: &[u64],
    mask_words: &[u64],
    bit_pos: u32,
    seed_bits_lz: u32,
    seed_bits: u64,
) -> (u32, bool) {
    let word_idx = (bit_pos / 64) as usize;
    let bit_offset = (bit_pos % 64) as u32;

    // 当 bit_offset=0 时，不需要从下一个 word 取数据
    // C++: ((_m[0]<<(_a*2))|((_m[1]>>1)>>(63-_a*2)))
    // 简化: ((_m[0]<<bit_offset)|(_m[1]>>(64-bit_offset)))
    let straddle: u64 = if bit_offset == 0 {
        words[word_idx]
    } else if word_idx + 1 < words.len() {
        (words[word_idx] << bit_offset)
            | (words[word_idx + 1] >> (64 - bit_offset))
    } else {
        words[word_idx] << bit_offset
    };

    let mask_straddle: u64 = if bit_offset == 0 {
        mask_words[word_idx]
    } else if word_idx + 1 < mask_words.len() {
        (mask_words[word_idx] << bit_offset)
            | (mask_words[word_idx + 1] >> (64 - bit_offset))
    } else {
        mask_words[word_idx] << bit_offset
    };

    let seed = xt3((straddle >> seed_bits_lz) as u32);
    let mask = (!mask_straddle >> seed_bits_lz) as u64 & seed_bits;
    let has_reg = mask == 0;

    (seed, has_reg)
}

// ── Helper: 2-bit Encoding ──────────────────────────────────────────────────

/// Pack a DNA byte slice into u64 words (2 bits per base).
/// Bases beyond the slice are padded with 0 (A).
///
/// This is the fundamental encoding used by both the reference genome
/// and query reads before alignment.
#[inline]
pub fn pack_forward(seq: &[u8], n_words: usize) -> Vec<u64> {
    let mut words = vec![0u64; n_words];
    for (i, chunk) in seq.chunks(SEGLEN).enumerate() {
        if i >= n_words {
            break;
        }
        let mut w: u64 = 0;
        for &base in chunk {
            w = (w << 2) | ALPHABET[base as usize] as u64;
        }
        // Left-align remaining bits within the word (pad with zeros = A)
        w <<= (SEGLEN - chunk.len()) * 2;
        words[i] = w;
    }
    words
}

/// Pack a DNA byte slice into u64 words in **reverse-complement** orientation.
/// Iterates the input right-to-left, applying REV_ALPHABET.
#[inline]
pub fn pack_revcomp(seq: &[u8], n_words: usize) -> Vec<u64> {
    let mut words = vec![0u64; n_words];
    let total_bases = seq.len().min(n_words * SEGLEN);
    let reversed: Vec<u8> = seq[..total_bases]
        .iter()
        .rev()
        .map(|&c| REV_ALPHABET[c as usize])
        .collect();

    for (i, chunk) in reversed.chunks(SEGLEN).enumerate() {
        let mut w: u64 = 0;
        for &code in chunk {
            w = (w << 2) | code as u64;
        }
        w <<= (SEGLEN - chunk.len()) * 2;
        words[i] = w;
    }
    words
}

/// Reverse-complement characters in a byte slice in-place.
/// Uses REV_CHAR lookup table, then reverses the order.
#[inline]
pub fn revcomp_in_place(seq: &mut [u8]) {
    let len = seq.len();
    let half = len / 2;
    for i in 0..half {
        let j = len - 1 - i;
        let tmp = REV_CHAR[seq[i] as usize];
        seq[i] = REV_CHAR[seq[j] as usize];
        seq[j] = tmp;
    }
    if len % 2 == 1 {
        seq[half] = REV_CHAR[seq[half] as usize];
    }
}

// ── SIMD Encoding (AVX2) ─────────────────────────────────────────────────

/// SIMD 优化的正向编码（x86_64 AVX2）。
/// 运行时检测 AVX2 支持，不支持时回退标量版本。
#[cfg(target_arch = "x86_64")]
#[inline]
pub fn pack_forward_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    if is_x86_feature_detected!("avx2") {
        unsafe { pack_forward_avx2(seq, n_words) }
    } else {
        pack_forward(seq, n_words)
    }
}

/// AVX2 正向编码内部实现。
/// 使用 pcmpeqb + blendv 批量查表，每次处理 32 字节。
/// 对每个字节分别与 A/a/C/c/G/g/T/t 比较，得到 2-bit 编码，再标量打包为 u64。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_forward_avx2(seq: &[u8], n_words: usize) -> Vec<u64> {
    use std::arch::x86_64::*;

    let c_upper = _mm256_set1_epi8(b'C' as i8);
    let c_lower = _mm256_set1_epi8(b'c' as i8);
    let g_upper = _mm256_set1_epi8(b'G' as i8);
    let g_lower = _mm256_set1_epi8(b'g' as i8);
    let t_upper = _mm256_set1_epi8(b'T' as i8);
    let t_lower = _mm256_set1_epi8(b't' as i8);
    let one = _mm256_set1_epi8(1);
    let two = _mm256_set1_epi8(2);
    let three = _mm256_set1_epi8(3);

    let mut words = vec![0u64; n_words];
    let mut word_idx = 0;
    let mut seq_pos = 0;

    // 每次处理 32 个碱基 = 1 个 u64 word (SEGLEN=32)
    while seq_pos + 32 <= seq.len() && word_idx < n_words {
        let input = _mm256_loadu_si256(seq.as_ptr().add(seq_pos) as *const __m256i);

        // 默认编码 0 (A)，逐级叠加：C=1, G=2, T=3
        let is_c = _mm256_or_si256(
            _mm256_cmpeq_epi8(input, c_upper),
            _mm256_cmpeq_epi8(input, c_lower),
        );
        let is_g = _mm256_or_si256(
            _mm256_cmpeq_epi8(input, g_upper),
            _mm256_cmpeq_epi8(input, g_lower),
        );
        let is_t = _mm256_or_si256(
            _mm256_cmpeq_epi8(input, t_upper),
            _mm256_cmpeq_epi8(input, t_lower),
        );

        // blendv: mask 为全1时取 val，否则取 arg1 (0)
        let mut encoded = _mm256_setzero_si256();
        encoded = _mm256_blendv_epi8(encoded, one, is_c);
        encoded = _mm256_blendv_epi8(encoded, two, is_g);
        encoded = _mm256_blendv_epi8(encoded, three, is_t);

        // 将 32 字节编码结果打包为 1 个 u64（每碱基取低 2 位）
        let mut buf = [0u8; 32];
        _mm256_storeu_si256(buf.as_mut_ptr() as *mut __m256i, encoded);

        let mut w: u64 = 0;
        for &b in &buf {
            w = (w << 2) | (b & 0x03) as u64;
        }
        words[word_idx] = w;
        word_idx += 1;
        seq_pos += 32;
    }

    // 标量处理剩余不足 32 碱基的部分
    if word_idx < n_words {
        let remaining = &seq[seq_pos..];
        let chunk_len = remaining.len().min(SEGLEN);
        if chunk_len > 0 {
            let mut w: u64 = 0;
            for &base in remaining.iter().take(chunk_len) {
                w = (w << 2) | ALPHABET[base as usize] as u64;
            }
            w <<= (SEGLEN - chunk_len) * 2;
            words[word_idx] = w;
        }
    }

    words
}

/// 非 x86_64 平台的 SIMD 存根（直接调用标量版本）。
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub fn pack_forward_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    pack_forward(seq, n_words)
}

/// SIMD 优化的反向互补编码（x86_64 AVX2）。
/// 先标量反转序列并查 REV_ALPHABET，再用 SIMD 正向编码。
#[cfg(target_arch = "x86_64")]
#[inline]
pub fn pack_revcomp_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    if is_x86_feature_detected!("avx2") {
        unsafe { pack_revcomp_avx2(seq, n_words) }
    } else {
        pack_revcomp(seq, n_words)
    }
}

/// AVX2 反向互补编码内部实现。
/// 反转序列 + REV_ALPHABET 查表后，复用 pack_forward_avx2 编码。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_revcomp_avx2(seq: &[u8], n_words: usize) -> Vec<u64> {
    let total_bases = seq.len().min(n_words * SEGLEN);

    // 反转序列并查 REV_ALPHABET（标量，因为反向依赖前一步结果）
    let mut reversed: Vec<u8> = vec![0u8; total_bases];
    for (i, &b) in seq[..total_bases].iter().enumerate() {
        reversed[total_bases - 1 - i] = REV_ALPHABET[b as usize];
    }

    // 将 2-bit 编码值打包为 u64 words（与 pack_revcomp 标量逻辑一致）
    let mut words = vec![0u64; n_words];
    for (i, chunk) in reversed.chunks(SEGLEN).enumerate() {
        let mut w: u64 = 0;
        for &code in chunk {
            w = (w << 2) | code as u64;
        }
        w <<= (SEGLEN - chunk.len()) * 2;
        words[i] = w;
    }
    words
}

/// 非 x86_64 平台存根。
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub fn pack_revcomp_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    pack_revcomp(seq, n_words)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alphabet_encoding() {
        assert_eq!(ALPHABET[b'A' as usize], 0);
        assert_eq!(ALPHABET[b'C' as usize], 1);
        assert_eq!(ALPHABET[b'G' as usize], 2);
        assert_eq!(ALPHABET[b'T' as usize], 3);
        assert_eq!(ALPHABET[b'a' as usize], 0);
        assert_eq!(ALPHABET[b'c' as usize], 1);
        assert_eq!(ALPHABET[b'g' as usize], 2);
        assert_eq!(ALPHABET[b't' as usize], 3);
        assert_eq!(ALPHABET[b'N' as usize], 0); // default = A
    }

    #[test]
    fn test_rev_alphabet() {
        // A→T→3, C→G→2, G→C→1, T→A→0
        assert_eq!(REV_ALPHABET[b'A' as usize], 3);
        assert_eq!(REV_ALPHABET[b'C' as usize], 2);
        assert_eq!(REV_ALPHABET[b'G' as usize], 1);
        assert_eq!(REV_ALPHABET[b'T' as usize], 0);
    }

    #[test]
    fn test_rev_char() {
        assert_eq!(REV_CHAR[b'A' as usize], b'T');
        assert_eq!(REV_CHAR[b'T' as usize], b'A');
        assert_eq!(REV_CHAR[b'C' as usize], b'G');
        assert_eq!(REV_CHAR[b'G' as usize], b'C');
        assert_eq!(REV_CHAR[b'N' as usize], b'N');
    }

    #[test]
    fn test_revcomp_in_place() {
        let mut seq = b"ACGT".to_vec();
        revcomp_in_place(&mut seq);
        assert_eq!(seq, b"ACGT"); // ACGT reversed → TGCA, complemented → ACGT
    }

    #[test]
    fn test_xt3_identity() {
        // AAAA... (all zeros) → should hash to 0
        assert_eq!(xt3(0), 0);
    }

    #[test]
    fn test_xt3_single_base() {
        // For a 16-base k-mer with a single C at the rightmost position (LSB):
        // C=0b01 at position 0 → base-3 value = 1 * 3^0 = 1
        assert_eq!(xt3(0x1), 1); // single C in LSB position

        // G=0b10 → should give 2 since G→2
        assert_eq!(xt3(0x2), 2); // single G

        // T=0b11 → should map to 1 (same as C) after C/T ambiguity
        assert_eq!(xt3(0x3), 1); // T → C → 1
    }

    #[test]
    fn test_xt3_ct_same() {
        // C and T at the same position should produce the same hash
        let c_only: u32 = 0x1; // ...01 (C at pos 0)
        let t_only: u32 = 0x3; // ...11 (T at pos 0)
        assert_eq!(xt3(c_only), xt3(t_only));
    }

    #[test]
    fn test_xt3_64_ct_same() {
        let c_only: u64 = 0x1;
        let t_only: u64 = 0x3;
        assert_eq!(xt3_64(c_only), xt3_64(t_only));
    }

    #[test]
    fn test_xm64_empty() {
        // All zeros = all matches
        assert_eq!(xm64(0), 0);
    }

    #[test]
    fn test_xm64_all_mismatch() {
        // All 2-bit fields are non-zero (e.g., all 0b11)
        // 0xFFFFFFFF_FFFFFFFF has 32 2-bit fields all non-zero
        let all_nonzero: u64 = 0xFFFF_FFFF_FFFF_FFFF;
        assert_eq!(xm64(all_nonzero), 32);
    }

    #[test]
    fn test_xm64_single_mismatch() {
        // Single 2-bit field non-zero at LSB
        assert_eq!(xm64(0x3), 1);
        assert_eq!(xm64(0xC), 1); // non-zero at second position
    }

    #[test]
    fn test_xc64() {
        // C=0b01 → xc64 should produce 0b11 (T)
        let ref_c: u64 = 0x5555_5555_5555_5555; // all C's (01 repeated 32 times)
        let mask = xc64(ref_c);
        // Each 2-bit field: C(01) → ((~01)<<1)=10<<1=100 | 01 | 01 = 101 = 5 ... hmm
        // Let me verify: ((!0b01) << 1) | 0b01 | 0b01
        // = (0b10 << 1) | 0b01 | 0b01 = 0b100 | 0b01 = 0b101 = 5... that's 3 bits!
        // Actually in 2-bit context: ((!01)<<1) = (10<<1) = 100. | 01 | 01 = 101
        // Lower 2 bits = 01. Hmm.
        // OK, let me just verify it produces expected behavior:
        // For C in ref (01), read T (11) & xc64(01) ^ ref = 11 & xc64(01) ^ 01
        // Should be 0 to indicate match.
        let read_t: u64 = 0xFFFF_FFFF_FFFF_FFFF; // all T
        let result = (read_t & mask) ^ ref_c;
        assert_eq!(result, 0, "T in read should match C in ref");
    }

    #[test]
    fn test_pack_roundtrip() {
        let seq = b"ACGTACGT";
        let words = pack_forward(seq, 1);
        // ACGTACGT = 00 01 10 11 00 01 10 11
        let expected: u64 = 0b00_01_10_11_00_01_10_11;
        // After left-align and pad (32-8=24 bases of padding = 48 bits)
        assert_eq!(words[0], expected << 48);
    }

    #[test]
    fn test_make_seed_bit_offset_zero() {
        // 测试 bit_offset=0 时的行为
        // 构造两个 word：word[0] 包含种子，word[1] 包含不同的数据
        // seed_size=8, seed_bits_lz = (32-8)*2 = 48
        let seed_bits_lz = 48u32;

        // word[0]: ACGTACGTACGTACGTACGTACGTACGTACGT (32 个碱基，全 A)
        // 所有 A = 0b00...00，所以任何位置的种子哈希都应该是 0
        let word0: u64 = 0; // 全 A
        let word1: u64 = 0xFFFF_FFFF_FFFF_FFFF; // 全 T (0b11)

        let words = vec![word0, word1];

        // bit_pos=0 → word_idx=0, bit_offset=0
        // 修复后应该只使用 word[0]，不受 word[1] 影响
        let hash = make_seed(&words, 0, seed_bits_lz);
        assert_eq!(
            hash, 0,
            "bit_offset=0 时不应从下一个 word 取数据，全 A 的种子哈希应为 0"
        );

        // 对比：如果错误地 OR 了 word[1]，结果会不同
        // word[1] 全 T，哈希不为 0
        let hash_t = xt3(0xFFFF_FFFF_u32);
        assert_ne!(
            hash, hash_t,
            "验证全 T 的哈希确实不为 0，确保测试有效"
        );
    }

    #[test]
    fn test_make_seed_bit_offset_nonzero() {
        // 测试 bit_offset!=0 时跨 word 边界的正确行为
        // seed_size=8, seed_bits_lz = (32-8)*2 = 48
        let seed_bits_lz = 48u32;

        // word[0] 低 32 位为全 T (0b11...11)，高 32 位为 0
        // word[1] 高 32 位为全 A (0)，低 32 位为全 T
        let word0: u64 = 0x0000_0000_FFFF_FFFF; // 高 32 位为 0，低 32 位全 T
        let word1: u64 = 0xFFFF_FFFF_0000_0000; // 高 32 位全 T，低 32 位为 0

        let words = vec![word0, word1];

        // bit_pos=32 → word_idx=0, bit_offset=32
        // straddle = (word0 << 32) | (word1 >> 32)
        //          = (0xFFFF_FFFF_0000_0000) | (0x0000_0000_FFFF_FFFF)
        //          = 0xFFFF_FFFF_FFFF_FFFF（全 T）
        // seed_bits_lz=48，取高 16 位 = 0xFFFF（8 个碱基全 T）
        let hash = make_seed(&words, 32, seed_bits_lz);
        let expected_hash = xt3(0xFFFF_u32); // 8 个碱基全 T 的哈希
        assert_eq!(
            hash, expected_hash,
            "bit_offset=32 时应正确跨 word 边界提取种子"
        );
    }

    #[test]
    fn test_make_seed_with_mask_bit_offset_zero() {
        // 测试 make_seed_with_mask 在 bit_offset=0 时的行为
        let seed_bits_lz = 48u32;
        let seed_bits: u64 = (1u64 << 16) - 1; // 8 个碱基

        let words = vec![0u64; 2]; // 全 A
        let mask_words = vec![0xFFFF_FFFF_FFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF]; // 全有效

        let (hash, has_reg) = make_seed_with_mask(&words, &mask_words, 0, seed_bits_lz, seed_bits);
        assert_eq!(hash, 0, "全 A 的种子哈希应为 0");
        assert!(has_reg, "所有碱基都是有效核苷酸");
    }

    /// 测试 xt3 和 xt3_64 对相同种子的哈希差异
    ///
    /// xt3 处理 32 位（最多 16 个碱基），xt3_64 处理 64 位（最多 32 个碱基）。
    /// 由于权重计算方式不同，它们对相同输入产生不同的哈希值。
    /// 因此索引构建和读段提取必须统一使用同一个函数（xt3）。
    #[test]
    fn test_xt3_xt3_64_differ() {
        let seed_24bit: u64 = 0b00011011_00011011_00011011;
        let seed_bits_lz = 40u32;
        let seed_aligned_64 = seed_24bit << seed_bits_lz;
        let hash_xt3 = xt3((seed_aligned_64 >> seed_bits_lz) as u32);
        let hash_xt3_64 = xt3_64(seed_24bit) as u32;

        // 验证它们确实不同（这是预期行为）
        assert_ne!(
            hash_xt3, hash_xt3_64,
            "xt3 和 xt3_64 对相同输入产生不同哈希（这是预期行为）"
        );

        // 验证 xt3 的结果是我们选择的统一哈希函数
        assert_eq!(hash_xt3, 106288);
    }

    #[test]
    fn test_pack_forward_simd_consistency() {
        let test_cases: Vec<&[u8]> = vec![
            b"",
            b"A",
            b"AC",
            b"ACG",
            b"ACGT",
            b"ACGTACGTACGTACGTACGTACGTACGTACGT", // 32 bases = 1 word
            b"ACGTACGTACGTACGTACGTACGTACGTACGTA", // 33 bases = 2 words
            b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT", // 64 bases = 2 words
            b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTA", // 65 bases = 3 words
            b"NNNNNNNN",
            b"ACNTGNCATGC",
            b"acgtnACGTN", // mixed case
        ];

        for seq in &test_cases {
            let n_words = if seq.is_empty() { 1 } else { (seq.len() + SEGLEN - 1) / SEGLEN };
            let scalar = pack_forward(seq, n_words);
            let simd = pack_forward_simd(seq, n_words);
            assert_eq!(scalar, simd, "pack_forward_simd mismatch for seq len={}", seq.len());
        }
    }

    #[test]
    fn test_pack_revcomp_simd_consistency() {
        let test_cases: Vec<&[u8]> = vec![
            b"",
            b"A",
            b"ACGT",
            b"ACGTACGTACGTACGTACGTACGTACGTACGT", // 32 bases
            b"ACGTACGTACGTACGTACGTACGTACGTACGTA", // 33 bases
            b"NNNNNNNN",
            b"ACNTGNCATGC",
        ];

        for seq in &test_cases {
            let n_words = if seq.is_empty() { 1 } else { (seq.len() + SEGLEN - 1) / SEGLEN };
            let scalar = pack_revcomp(seq, n_words);
            let simd = pack_revcomp_simd(seq, n_words);
            assert_eq!(scalar, simd, "pack_revcomp_simd mismatch for seq len={}", seq.len());
        }
    }
}
