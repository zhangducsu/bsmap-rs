//! 读段二进制编码模块。
//!
//! 将读段序列编码为 2-bit 二进制格式（正向 + 反向互补），
//! 并生成有效碱基掩码，用于比对引擎的快速匹配。
//! 对应 C++ `align.cpp` 中的 `ConvertBinarySeq()`。

use crate::alphabet::{pack_forward, pack_revcomp, REG_ALPHABET};
use crate::param::{ReadInf, SEGLEN};

/// 编码后的读段（用于比对引擎）。
///
/// 包含正向链和反向互补链的二进制编码，以及对应的
/// 有效碱基掩码。对应 C++ `ConvertBinarySeq()` 的输出。
#[derive(Debug, Clone)]
pub struct EncodedRead {
    /// 编码后的 u64 数组（正向链）。
    /// 每个元素包含 32 个碱基的 2-bit 编码。
    pub fwd_words: Vec<u64>,
    /// 编码后的 u64 数组（反向互补链）。
    pub rev_words: Vec<u64>,
    /// 有效碱基掩码（正向链）。
    /// 每个有效碱基位置为 0b11，无效位置（N 等）为 0b00。
    pub fwd_mask: Vec<u64>,
    /// 有效碱基掩码（反向互补链）。
    pub rev_mask: Vec<u64>,
    /// 原始读段信息。
    pub info: ReadInf,
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
    let seq = &read.seq;
    let len = seq.len();

    // 计算需要的 word 数量
    let num_words = if len == 0 {
        1 // 至少一个 word
    } else {
        (len + SEGLEN - 1) / SEGLEN
    };

    // 正向编码
    let fwd_words = pack_forward(seq, num_words);

    // 反向互补编码
    let rev_words = pack_revcomp(seq, num_words);

    // 正向有效碱基掩码
    let fwd_mask = build_mask(seq, num_words, false);

    // 反向互补有效碱基掩码
    let rev_mask = build_mask(seq, num_words, true);

    EncodedRead {
        fwd_words,
        rev_words,
        fwd_mask,
        rev_mask,
        info: read.clone(),
    }
}

/// 构建有效碱基掩码。
///
/// 对于每个碱基位置，如果是有效碱基（A/C/G/T），掩码为 0b11；
/// 否则（N 等），掩码为 0b00。
///
/// # 参数
/// - `seq`：原始序列
/// - `num_words`：u64 word 数量
/// - `reverse`：是否反向（用于反向互补掩码）
fn build_mask(seq: &[u8], num_words: usize, reverse: bool) -> Vec<u64> {
    let mut mask = vec![0u64; num_words];

    if reverse {
        // 反向互补掩码：序列反向遍历，使用 REV_REG 掩码
        // 对于反向互补，有效碱基掩码也需要反向
        let total_bases = seq.len().min(num_words * SEGLEN);
        let reversed_mask: Vec<u8> = seq[..total_bases]
            .iter()
            .rev()
            .map(|&c| REG_ALPHABET[c as usize])
            .collect();

        for (i, chunk) in reversed_mask.chunks(SEGLEN).enumerate() {
            if i >= num_words {
                break;
            }
            let mut w: u64 = 0;
            for &m in chunk {
                w = (w << 2) | m as u64;
            }
            w <<= (SEGLEN - chunk.len()) * 2;
            mask[i] = w;
        }
    } else {
        // 正向掩码
        for (i, chunk) in seq.chunks(SEGLEN).enumerate() {
            if i >= num_words {
                break;
            }
            let mut w: u64 = 0;
            for &c in chunk {
                w = (w << 2) | REG_ALPHABET[c as usize] as u64;
            }
            w <<= (SEGLEN - chunk.len()) * 2;
            mask[i] = w;
        }
    }

    mask
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
        assert_eq!(encoded.fwd_words, expected);
    }

    #[test]
    fn test_encode_rev_matches_pack_revcomp() {
        let read = make_read("test", b"ACGTACGT");
        let encoded = encode_read(&read);

        let expected = pack_revcomp(b"ACGTACGT", 1);
        assert_eq!(encoded.rev_words, expected);
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

        assert_eq!(encoded.fwd_words.len(), 2);
        assert_eq!(encoded.rev_words.len(), 2);
        assert_eq!(encoded.fwd_mask.len(), 2);
        assert_eq!(encoded.rev_mask.len(), 2);
    }

    #[test]
    fn test_encode_preserves_info() {
        let read = make_read("my_read", b"ACGT");
        let encoded = encode_read(&read);

        assert_eq!(encoded.info.name, "my_read");
        assert_eq!(encoded.info.seq, b"ACGT");
        assert_eq!(encoded.info.index, 0);
        assert_eq!(encoded.info.read_set, 0);
    }

    #[test]
    fn test_encode_empty_sequence() {
        let read = make_read("empty", b"");
        let encoded = encode_read(&read);

        // 空序列仍应有 1 个 word
        assert_eq!(encoded.fwd_words.len(), 1);
        assert_eq!(encoded.rev_words.len(), 1);
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
}
