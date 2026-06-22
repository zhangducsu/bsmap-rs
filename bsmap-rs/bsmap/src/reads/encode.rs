//! 读段二进制编码模块。
//!
//! 将读段序列编码为 2-bit 二进制格式（正向 + 反向互补），
//! 并生成有效碱基掩码，用于比对引擎的快速匹配。
//! 对应 C++ `align.cpp` 中的 `ConvertBinarySeq()`。

#[cfg(test)]
use crate::alphabet::{pack_forward, pack_revcomp};
use crate::alphabet::{ALPHABET, REG_ALPHABET, REV_ALPHABET};
use crate::param::{ReadInf, FIXELEMENT, FIXSIZE, SEGLEN};

/// 编码后的读段（用于比对引擎）。
///
/// 包含正向链和反向互补链的二进制编码，以及对应的
/// 有效碱基掩码。对应 C++ `ConvertBinarySeq()` 的输出。
#[derive(Debug, Clone)]
pub struct EncodedRead {
    /// 编码后的 u64 数组（正向链）。
    /// 每个元素包含 32 个碱基的 2-bit 编码。
    pub fwd_words: [u64; FIXELEMENT],
    /// 编码后的 u64 数组（反向互补链）。
    pub rev_words: [u64; FIXELEMENT],
    /// 有效碱基掩码（正向链）。
    /// 每个有效碱基位置为 0b11，无效位置（N 等）为 0b00。
    pub fwd_mask: [u64; FIXELEMENT],
    /// 有效碱基掩码（反向互补链）。
    pub rev_mask: [u64; FIXELEMENT],
    num_words: u8,
    read_len: u16,
    pub low_qual_count: u16,
    pub index: u32,
    pub read_set: u8,
}

impl EncodedRead {
    #[inline]
    pub fn num_words(&self) -> usize {
        self.num_words as usize
    }

    #[inline]
    pub fn read_len(&self) -> u32 {
        self.read_len as u32
    }

    #[inline]
    pub fn fwd_words(&self) -> &[u64] {
        &self.fwd_words[..self.num_words()]
    }

    #[inline]
    pub fn rev_words(&self) -> &[u64] {
        &self.rev_words[..self.num_words()]
    }

    #[inline]
    pub fn fwd_mask(&self) -> &[u64] {
        &self.fwd_mask[..self.num_words()]
    }

    #[inline]
    pub fn rev_mask(&self) -> &[u64] {
        &self.rev_mask[..self.num_words()]
    }
}

/// 将读段编码为 2-bit 二进制格式（正向 + 反向互补）。
///
/// 对读段序列进行以下编码：
/// 1. 正向编码：使用 `ALPHABET` 表将每个碱基编码为 2-bit
/// 2. 反向互补编码：使用 `REV_ALPHABET` 表反向编码
/// 3. 生成有效碱基掩码：使用 `REG_ALPHABET` 表标记有效碱基
///
/// 编码后的 word 数量由读段长度决定：`ceil(len / 32)` 个 u64。
///
/// # 参数
/// - `read`：待编码的读段（`ReadInf`）
///
/// # 返回值
/// 编码后的 `EncodedRead`，包含正向/反向互补的编码和掩码。
///
/// # 示例
///
/// ```
/// // 序列 "ACGT" 编码为 00 01 10 11，左对齐到 64 位
/// // fwd_words[0] = 0b00_01_10_11 << 48
/// ```
pub fn encode_read(read: &ReadInf) -> EncodedRead {
    encode_read_with_quality(read, 0, 0)
}

pub fn encode_read_with_quality(
    read: &ReadInf,
    qual_threshold: u8,
    zero_qual: u8,
) -> EncodedRead {
    let seq = &read.seq;
    let len = seq.len();
    assert!(
        len <= FIXSIZE,
        "read length {} exceeds fixed encoding capacity {}",
        len,
        FIXSIZE,
    );

    // 计算需要的 word 数量
    let num_words = if len == 0 {
        1 // 至少一个 word
    } else {
        (len + SEGLEN - 1) / SEGLEN
    };

    let mut fwd_words = [0u64; FIXELEMENT];
    let mut rev_words = [0u64; FIXELEMENT];
    let mut fwd_mask = [0u64; FIXELEMENT];
    let mut rev_mask = [0u64; FIXELEMENT];

    for word_index in 0..num_words {
        let start = word_index * SEGLEN;
        let chunk_len = len.saturating_sub(start).min(SEGLEN);
        let mut forward = 0u64;
        let mut reverse = 0u64;
        let mut forward_mask = 0u64;
        let mut reverse_mask = 0u64;
        for offset in 0..chunk_len {
            let forward_base = seq[start + offset];
            let reverse_base = seq[len - 1 - (start + offset)];
            forward = (forward << 2) | ALPHABET[forward_base as usize] as u64;
            reverse = (reverse << 2) | REV_ALPHABET[reverse_base as usize] as u64;
            forward_mask =
                (forward_mask << 2) | REG_ALPHABET[forward_base as usize] as u64;
            reverse_mask =
                (reverse_mask << 2) | REG_ALPHABET[reverse_base as usize] as u64;
        }
        if chunk_len > 0 {
            let padding = (SEGLEN - chunk_len) * 2;
            fwd_words[word_index] = forward << padding;
            rev_words[word_index] = reverse << padding;
            fwd_mask[word_index] = forward_mask << padding;
            rev_mask[word_index] = reverse_mask << padding;
        }
    }

    EncodedRead {
        fwd_words,
        rev_words,
        fwd_mask,
        rev_mask,
        num_words: num_words as u8,
        read_len: len as u16,
        low_qual_count: if qual_threshold == 0 {
            0
        } else {
            read.qual
                .iter()
                .filter(|&&quality| quality < qual_threshold.saturating_add(zero_qual))
                .count() as u16
        },
        index: read.index,
        read_set: read.read_set as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_read(name: &str, seq: &[u8]) -> ReadInf {
        ReadInf {
            index: 0,
            read_set: 0,
            name: name.to_string(),
            seq: seq.to_vec(),
            qual: vec![33u8; seq.len()],
        }
    }

    #[test]
    fn test_encode_acgt() {
        let read = make_read("test", b"ACGT");
        let encoded = encode_read(&read);

        // ACGT = 00 01 10 11，左对齐到 32 位（SEGLEN=32）
        // 低 8 位 = 0b00_01_10_11 = 0x1B
        // 左移 (32-4)*2 = 56 位
        let expected: u64 = 0x1Bu64 << 56;
        assert_eq!(encoded.fwd_words[0], expected);
    }

    #[test]
    fn test_encode_fwd_matches_pack_forward() {
        let read = make_read("test", b"ACGTACGTACGTACGT");
        let encoded = encode_read(&read);

        let expected = pack_forward(b"ACGTACGTACGTACGT", 1);
        assert_eq!(encoded.fwd_words(), expected.as_slice());
    }

    #[test]
    fn test_encode_rev_matches_pack_revcomp() {
        let read = make_read("test", b"ACGTACGT");
        let encoded = encode_read(&read);

        let expected = pack_revcomp(b"ACGTACGT", 1);
        assert_eq!(encoded.rev_words(), expected.as_slice());
    }

    #[test]
    fn test_encode_mask_all_valid() {
        let read = make_read("test", b"ACGT");
        let encoded = encode_read(&read);

        // 4 个有效碱基，每个 0b11，左对齐到 32 位
        // 4 * 2 = 8 位，左移 (32-4)*2 = 56 位
        let expected_mask: u64 = 0xFFu64 << 56;
        assert_eq!(encoded.fwd_mask[0], expected_mask);
    }

    #[test]
    fn test_encode_mask_with_n() {
        let read = make_read("test", b"ACNT");
        let encoded = encode_read(&read);

        // A=11, C=11, N=00, T=11 → 0b11_11_00_11 = 0xF3
        // 左对齐到 32 位（4 个碱基，移 56 位）
        let expected_mask: u64 = 0xF3u64 << 56;
        assert_eq!(encoded.fwd_mask[0], expected_mask);
    }

    #[test]
    fn test_encode_rev_mask_with_n() {
        let read = make_read("test", b"ACNT");
        let encoded = encode_read(&read);

        // 反向互补序列：ACNT → 反向 TNCA → 编码掩码
        // T=11, N=00, C=11, A=11 → 0b11_00_11_11 = 0xCF
        // 左对齐到 32 位
        let expected_mask: u64 = 0xCFu64 << 56;
        assert_eq!(encoded.rev_mask[0], expected_mask);
    }

    #[test]
    fn test_encode_multiple_words() {
        // 33 个碱基，需要 2 个 word
        let seq: Vec<u8> = (0..33).map(|i| b"ACGT"[i % 4]).collect();
        let read = make_read("test", &seq);
        let encoded = encode_read(&read);

        assert_eq!(encoded.num_words(), 2);
        assert_eq!(encoded.fwd_words().len(), 2);
        assert_eq!(encoded.rev_words().len(), 2);
        assert_eq!(encoded.fwd_mask().len(), 2);
        assert_eq!(encoded.rev_mask().len(), 2);
    }

    #[test]
    fn test_encode_preserves_minimal_metadata() {
        let read = make_read("my_read", b"ACGT");
        let encoded = encode_read(&read);

        assert_eq!(encoded.read_len(), 4);
        assert_eq!(encoded.index, 0);
        assert_eq!(encoded.read_set, 0);
        assert!(std::mem::size_of::<EncodedRead>() <= 208);
    }

    #[test]
    fn test_encode_summarizes_quality_without_cloning_read() {
        let mut read = make_read("quality", b"ACGT");
        read.qual = vec![33, 34, 35, 40];
        let encoded = encode_read_with_quality(&read, 2, 33);

        assert_eq!(encoded.low_qual_count, 2);
        assert_eq!(encoded.read_len(), 4);
    }

    #[test]
    fn test_encode_empty_sequence() {
        let read = make_read("empty", b"");
        let encoded = encode_read(&read);

        // 空序列仍应有 1 个 word
        assert_eq!(encoded.fwd_words().len(), 1);
        assert_eq!(encoded.rev_words().len(), 1);
        assert_eq!(encoded.fwd_words[0], 0);
        assert_eq!(encoded.rev_words[0], 0);
    }

    #[test]
    fn test_revcomp_consistency() {
        // 使用非回文序列验证正向和反向互补编码不同
        let read = make_read("test", b"ACGTA");
        let encoded = encode_read(&read);

        // 正向 ACGTA 编码：A=0, C=1, G=2, T=3, A=0 = 0b00_01_10_11_00
        // 左对齐 (32-5)*2 = 54 位
        let fwd = &encoded.fwd_words[0];

        // 反向互补：ACGTA → 反向 ATGCA → REV_ALPHABET 编码
        // A→3, T→0, G→1, C→2, A→3 = 0b11_00_01_10_11
        let rev = &encoded.rev_words[0];

        // ACGTA 不是回文，正向和反向互补编码应不同
        assert_ne!(*fwd, *rev, "非回文序列的正向和反向互补编码应不同");
    }

    /// 验证使用 SIMD 编码的 encode_read 与标量版本输出一致。
    /// 注意：由于 encode_read 内部已切换到 SIMD，此测试验证 SIMD 路径的正确性。
    #[test]
    fn test_encode_read_simd_correctness() {
        let test_seqs: Vec<&[u8]> = vec![
            b"ACGT",
            b"ACGTACGTACGTACGTACGTACGTACGTACGT", // 32 bases
            b"ACGTACGTACGTACGTACGTACGTACGTACGTA", // 33 bases
            b"ACNT",
            b"NNNNNNNN",
            b"acgtACGT", // mixed case
        ];

        for seq in &test_seqs {
            let read = make_read("test", seq);
            let encoded = encode_read(&read);

            // 验证正向编码与标量 pack_forward 一致
            let num_words = if seq.is_empty() { 1 } else { (seq.len() + SEGLEN - 1) / SEGLEN };
            let expected_fwd = pack_forward(seq, num_words);
            let expected_rev = pack_revcomp(seq, num_words);
            assert_eq!(encoded.fwd_words(), expected_fwd.as_slice(), "fwd_words mismatch for len={}", seq.len());
            assert_eq!(encoded.rev_words(), expected_rev.as_slice(), "rev_words mismatch for len={}", seq.len());
        }
    }

    #[test]
    #[should_panic(expected = "exceeds fixed encoding capacity")]
    fn test_encode_rejects_reads_beyond_fixed_capacity() {
        let read = make_read("too_long", &vec![b'A'; FIXSIZE + 1]);
        let _ = encode_read(&read);
    }
}
