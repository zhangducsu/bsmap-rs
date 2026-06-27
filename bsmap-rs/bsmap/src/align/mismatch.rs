//! 位并行 mismatch 计数模块。
//!
//! 这是比对引擎的核心热路径，使用位并行技术快速计算读段与参考序列的 mismatch 数量。
//! 对应 C++ align.cpp 中的 `CountMismatch()`、`MismatchPattern0()` 和 `MismatchPattern1()`。
//!
//! ## 核心算法
//!
//! 1. **位并行 XOR**: 读段与参考序列异或，非零位表示 mismatch
//! 2. **C→T 容忍掩码**: 应用 `xc64` 掩码，允许读段 T 匹配参考 C
//! 3. **SWAR popcount**: 使用 `xm64` 统计非零 2-bit 字段数量
//!
//! ## 性能优化
//!
//! - 函数内联（`#[inline]`）
//! - 提前终止：mismatch 数超过阈值立即返回
//! - SIMD 优化版本（x86_64 AVX2）

use crate::alphabet::{xc64, xm64};
use crate::param::SEGLEN;

/// Mismatch 计数结果。
#[derive(Debug, Clone, Copy, Default)]
pub struct MismatchResult {
    /// Mismatch 数量。
    pub count: u32,
    /// 是否提前终止（count > threshold）。
    pub aborted: bool,
}

/// 位并行 mismatch 计数。
///
/// 对应 C++ `CountMismatch()` 函数。比较读段编码数组与参考序列，
/// 计算 mismatch 数量，支持提前终止。
///
/// # 参数
/// - `query`: 读段编码数组（2-bit 每碱基）
/// - `offset`: 参考序列偏移（位偏移，0-based）
/// - `ref_seq`: 参考序列数组
/// - `mask`: 有效碱基掩码（标记 N 位置）
/// - `snp_thres`: mismatch 阈值，超过则提前返回
/// - `n_count`: 读段中 N 碱基数（C++ 默认 `N_mis=0`，不计入 mismatch）
/// - `nt3`: 3-核苷酸模式（C+T 共享编码）
///
/// # 返回值
/// Mismatch 数量，如果超过阈值则返回阈值+1
///
/// # 算法
///
/// 1. 计算 XOR 差异：`diff = query ^ ref`
/// 2. 应用 C→T 容忍掩码：`diff &= xc64(ref)`
/// 3. 应用有效碱基掩码：`diff |= !mask`
/// 4. 统计 popcount：`count += xm64(diff)`
/// 5. 如果 `count > snp_thres`，提前返回
#[inline]
pub fn count_mismatch(
    query: &[u64],
    offset: u64,
    ref_seq: &[u64],
    mask: &[u64],
    snp_thres: u32,
    _n_count: u32,
    _nt3: bool,
) -> u32 {
    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;

    let mut total_mismatches: u32 = 0;

    for i in 0..query.len() {
        let ref_word = ref_seq[word_offset + i];
        let q_word = shifted_query_word(query, i, bit_offset);
        let m_word = shifted_query_word(mask, i, bit_offset);
        let diff = ((q_word & xc64(ref_word)) ^ ref_word) & m_word;

        total_mismatches += xm64(diff);
        if total_mismatches > snp_thres {
            return snp_thres + 1;
        }
    }

    total_mismatches
}

#[inline]
fn shifted_query_word(words: &[u64], index: usize, bit_offset: u32) -> u64 {
    if bit_offset == 0 {
        words[index]
    } else {
        let high = if index == 0 {
            0
        } else {
            words[index - 1] << (64 - bit_offset)
        };
        high | (words[index] >> bit_offset)
    }
}

/// 记录所有 mismatch 位置（无 gap）。
///
/// 对应 C++ `MismatchPattern0()` 函数。遍历所有 word，
/// 找到所有 mismatch 位置，返回位置数组。
///
/// # 参数
/// - `query`: 读段编码数组
/// - `ref_seq`: 参考序列数组
/// - `offset`: 参考序列偏移（位偏移）
/// - `map_readlen`: 读段长度（碱基数）
/// - `nt3`: 3-核苷酸模式
///
/// # 返回值
/// Mismatch 位置数组（0-based，从读段起始开始计数）
///
/// # 算法
///
/// 1. 对每个 word 计算 XOR 差异
/// 2. 应用 C→T 容忍掩码
/// 3. 使用 `leading_zeros()` 快速定位最高位的 1
/// 4. 记录位置，清除该位，继续查找
#[inline]
pub fn mismatch_pattern_0(
    query: &[u64],
    ref_seq: &[u64],
    offset: u64,
    map_readlen: u32,
    nt3: bool,
) -> Vec<u32> {
    let mut positions = Vec::new();

    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;

    let mut bases_processed: u32 = 0;

    if bit_offset == 0 {
        for i in 0..query.len() {
            if bases_processed >= map_readlen {
                break;
            }

            let ref_word = ref_seq[word_offset + i];
            let q_word = query[i];

            // 计算差异
            let mut diff = q_word ^ ref_word;

            // 应用 C→T 容忍掩码
            diff &= xc64(ref_word);

            // 提取有效碱基对应的位（每 2-bit 一个碱基）
            // 将 diff 转换为每个碱基的 mismatch 指示
            let bases_in_word = ((map_readlen - bases_processed).min(SEGLEN as u32)) as usize;

            // 遍历每个碱基位置
            for j in 0..bases_in_word {
                let bit_pos = 62 - j * 2; // 从高位开始
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    positions.push(bases_processed + j as u32);
                }
            }

            bases_processed += bases_in_word as u32;
        }
    } else {
        // 提取以 bit_offset 开始的 64-bit 窗口（与 make_seed 一致）
        let shift_left = bit_offset;
        let shift_right = 64 - bit_offset;

        for i in 0..query.len() {
            if bases_processed >= map_readlen {
                break;
            }

            // 从参考序列提取对齐的 word（与 make_seed 相同的移位方向）
            let ref_low = ref_seq[word_offset + i] << shift_left;
            let ref_high = if word_offset + i + 1 < ref_seq.len() {
                ref_seq[word_offset + i + 1] >> shift_right
            } else {
                0
            };
            let ref_word = ref_low | ref_high;

            let q_word = query[i];

            // 计算差异
            let mut diff = q_word ^ ref_word;

            // 应用 C→T 容忍掩码
            diff &= xc64(ref_word);

            let bases_in_word = ((map_readlen - bases_processed).min(SEGLEN as u32)) as usize;

            for j in 0..bases_in_word {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    positions.push(bases_processed + j as u32);
                }
            }

            bases_processed += bases_in_word as u32;
        }
    }

    positions
}

/// 记录所有 mismatch 位置（有 gap，反向遍历）。
///
/// 对应 C++ `MismatchPattern1()` 函数。从后向前遍历，
/// 使用 `trailing_zeros()` 快速定位最低位的 1。
///
/// # 参数
/// - `query`: 读段编码数组
/// - `ref_seq`: 参考序列数组
/// - `offset`: 参考序列偏移（位偏移）
/// - `map_readlen`: 读段长度（碱基数）
/// - `nt3`: 3-核苷酸模式
///
/// # 返回值
/// Mismatch 位置数组（0-based，从读段起始开始计数）
#[inline]
pub fn mismatch_pattern_1(
    query: &[u64],
    ref_seq: &[u64],
    offset: u64,
    map_readlen: u32,
    nt3: bool,
) -> Vec<u32> {
    let mut positions = Vec::new();

    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;

    // 计算读段占用的 word 数
    let num_words = ((map_readlen as usize + SEGLEN - 1) / SEGLEN).max(1);

    if bit_offset == 0 {
        // 反向遍历 word
        for i in (0..num_words).rev() {
            let ref_word = ref_seq[word_offset + i];
            let q_word = query[i];

            // 计算差异
            let mut diff = q_word ^ ref_word;

            // 应用 C→T 容忍掩码
            diff &= xc64(ref_word);

            // 计算这个 word 中的碱基范围
            let word_start_base = i * SEGLEN;
            let bases_in_word = ((map_readlen as usize).saturating_sub(word_start_base)).min(SEGLEN);

            // 反向遍历碱基
            for j in (0..bases_in_word).rev() {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    positions.push((word_start_base + j) as u32);
                }
            }
        }
    } else {
        // 提取以 bit_offset 开始的 64-bit 窗口（与 make_seed 一致）
        let shift_left = bit_offset;
        let shift_right = 64 - bit_offset;

        for i in (0..num_words).rev() {
            // 从参考序列提取对齐的 word（与 make_seed 相同的移位方向）
            let ref_low = ref_seq[word_offset + i] << shift_left;
            let ref_high = if word_offset + i + 1 < ref_seq.len() {
                ref_seq[word_offset + i + 1] >> shift_right
            } else {
                0
            };
            let ref_word = ref_low | ref_high;

            let q_word = query[i];

            // 计算差异
            let mut diff = q_word ^ ref_word;

            // 应用 C→T 容忍掩码
            diff &= xc64(ref_word);

            let word_start_base = i * SEGLEN;
            let bases_in_word = ((map_readlen as usize).saturating_sub(word_start_base)).min(SEGLEN);

            for j in (0..bases_in_word).rev() {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    positions.push((word_start_base + j) as u32);
                }
            }
        }
    }

    positions
}

/// 仅计数 mismatch 数量，不分配 Vec（针对 mismatch_pattern_0 的热路径调用）。
#[inline]
pub fn count_mismatch_positions_0(
    query: &[u64],
    ref_seq: &[u64],
    offset: u64,
    map_readlen: u32,
    _nt3: bool,
) -> u32 {
    let mut count: u32 = 0;
    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;
    let mut bases_processed: u32 = 0;

    if bit_offset == 0 {
        for i in 0..query.len() {
            if bases_processed >= map_readlen {
                break;
            }
            let ref_word = ref_seq[word_offset + i];
            let q_word = query[i];
            let mut diff = q_word ^ ref_word;
            diff &= xc64(ref_word);
            let bases_in_word = ((map_readlen - bases_processed).min(SEGLEN as u32)) as usize;
            for j in 0..bases_in_word {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    count += 1;
                }
            }
            bases_processed += bases_in_word as u32;
        }
    } else {
        let shift_left = bit_offset;
        let shift_right = 64 - bit_offset;
        for i in 0..query.len() {
            if bases_processed >= map_readlen {
                break;
            }
            let ref_low = ref_seq[word_offset + i] << shift_left;
            let ref_high = if word_offset + i + 1 < ref_seq.len() {
                ref_seq[word_offset + i + 1] >> shift_right
            } else {
                0
            };
            let ref_word = ref_low | ref_high;
            let q_word = query[i];
            let mut diff = q_word ^ ref_word;
            diff &= xc64(ref_word);
            let bases_in_word = ((map_readlen - bases_processed).min(SEGLEN as u32)) as usize;
            for j in 0..bases_in_word {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    count += 1;
                }
            }
            bases_processed += bases_in_word as u32;
        }
    }

    count
}

/// 仅计数 mismatch 数量，不分配 Vec（针对 mismatch_pattern_1 的热路径调用）。
#[inline]
pub fn count_mismatch_positions_1(
    query: &[u64],
    ref_seq: &[u64],
    offset: u64,
    map_readlen: u32,
    _nt3: bool,
) -> u32 {
    let mut count: u32 = 0;
    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;
    let num_words = ((map_readlen as usize + SEGLEN - 1) / SEGLEN).max(1);

    if bit_offset == 0 {
        for i in (0..num_words).rev() {
            let ref_word = ref_seq[word_offset + i];
            let q_word = query[i];
            let mut diff = q_word ^ ref_word;
            diff &= xc64(ref_word);
            let word_start_base = i * SEGLEN;
            let bases_in_word = ((map_readlen as usize).saturating_sub(word_start_base)).min(SEGLEN);
            for j in (0..bases_in_word).rev() {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    count += 1;
                }
            }
        }
    } else {
        let shift_left = bit_offset;
        let shift_right = 64 - bit_offset;
        for i in (0..num_words).rev() {
            let ref_low = ref_seq[word_offset + i] << shift_left;
            let ref_high = if word_offset + i + 1 < ref_seq.len() {
                ref_seq[word_offset + i + 1] >> shift_right
            } else {
                0
            };
            let ref_word = ref_low | ref_high;
            let q_word = query[i];
            let mut diff = q_word ^ ref_word;
            diff &= xc64(ref_word);
            let word_start_base = i * SEGLEN;
            let bases_in_word = ((map_readlen as usize).saturating_sub(word_start_base)).min(SEGLEN);
            for j in (0..bases_in_word).rev() {
                let bit_pos = 62 - j * 2;
                let mask = 0b11u64 << bit_pos;
                if (diff & mask) != 0 {
                    count += 1;
                }
            }
        }
    }

    count
}

/// SIMD 优化版本（x86_64 AVX2）。
///
/// 如果目标平台支持 AVX2，使用 256-bit 向量指令加速 mismatch 计数。
/// 回退到标量版本如果不支持。
///
/// # 安全性
/// 内部使用 `unsafe` 块调用 AVX2 指令，但仅在检测到 AVX2 支持时执行。
#[cfg(target_arch = "x86_64")]
#[inline]
pub fn count_mismatch_simd(
    query: &[u64],
    offset: u64,
    ref_seq: &[u64],
    mask: &[u64],
    snp_thres: u32,
    _n_count: u32,
    nt3: bool,
) -> u32 {
    // 检查 AVX2 支持
    if is_x86_feature_detected!("avx2") {
        // SAFETY: 我们已经检查了 AVX2 支持
        unsafe {
            count_mismatch_avx2(query, offset, ref_seq, mask, snp_thres, 0, nt3)
        }
    } else {
        // 回退到标量版本
        count_mismatch(query, offset, ref_seq, mask, snp_thres, 0, nt3)
    }
}

/// AVX2 实现（内部函数）。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn count_mismatch_avx2(
    query: &[u64],
    offset: u64,
    ref_seq: &[u64],
    mask: &[u64],
    snp_thres: u32,
    _n_count: u32,
    nt3: bool,
) -> u32 {
    use std::arch::x86_64::*;

    let word_offset = (offset / 64) as usize;
    let bit_offset = (offset % 64) as u32;

    let mut total_mismatches: u32 = 0;

    // AVX2 每次处理 4 个 u64（256 bits）
    let simd_len = query.len() / 4 * 4;

    if bit_offset == 0 && simd_len > 0 {
        // 对齐情况下使用 SIMD
        for i in (0..simd_len).step_by(4) {
            // 加载 4 个 u64
            let q_vec = _mm256_loadu_si256(query.as_ptr().add(i) as *const __m256i);
            let r_vec = _mm256_loadu_si256(ref_seq.as_ptr().add(word_offset + i) as *const __m256i);
            let m_vec = _mm256_loadu_si256(mask.as_ptr().add(i) as *const __m256i);

            // XOR 计算差异
            let diff_vec = _mm256_xor_si256(q_vec, r_vec);

            // 应用掩码：diff & mask（mask=0 的位置清零不计入）
            let masked_diff = _mm256_and_si256(diff_vec, m_vec);

            // 提取到数组进行 popcount（AVX2 没有原生 popcount）
            let mut diffs = [0u64; 4];
            _mm256_storeu_si256(diffs.as_mut_ptr() as *mut __m256i, masked_diff);

            for j in 0..4 {
                let mut diff = diffs[j];

                // 应用 C→T 容忍掩码
                let ref_word = ref_seq[word_offset + i + j];
                diff &= xc64(ref_word);

                total_mismatches += xm64(diff);

                if total_mismatches > snp_thres {
                    return snp_thres + 1;
                }
            }
        }

        // 处理剩余部分
        for i in simd_len..query.len() {
            let ref_word = ref_seq[word_offset + i];
            let q_word = query[i];
            let m_word = mask[i];

            let mut diff = q_word ^ ref_word;

            diff &= xc64(ref_word);

            diff &= m_word;
            total_mismatches += xm64(diff);

            if total_mismatches > snp_thres {
                return snp_thres + 1;
            }
        }
    } else {
        // 非对齐情况，回退到标量
        return count_mismatch(query, offset, ref_seq, mask, snp_thres, 0, nt3);
    }

    total_mismatches
}

/// 非 x86_64 平台的 SIMD 存根。
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub fn count_mismatch_simd(
    query: &[u64],
    offset: u64,
    ref_seq: &[u64],
    mask: &[u64],
    snp_thres: u32,
    n_count: u32,
    nt3: bool,
) -> u32 {
    // 非 x86_64 平台直接回退到标量版本
    count_mismatch(query, offset, ref_seq, mask, snp_thres, n_count, nt3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alphabet::pack_forward;

    fn make_ref_seq(seq: &[u8]) -> Vec<u64> {
        // 参考序列需要更多 padding 以支持偏移
        let mut words = pack_forward(seq, 10);
        // 扩展以避免边界问题
        words.resize(20, 0);
        words
    }

    fn make_mask(len: usize) -> Vec<u64> {
        let num_words = (len + SEGLEN - 1) / SEGLEN;
        let mut mask = vec![0u64; num_words];
        for i in 0..num_words {
            let bases_in_word = len.saturating_sub(i * SEGLEN).min(SEGLEN);
            // 有效碱基掩码：每碱基 0b11
            let m = if bases_in_word == SEGLEN {
                u64::MAX
            } else {
                ((1u64 << (bases_in_word * 2)) - 1) << ((SEGLEN - bases_in_word) * 2)
            };
            mask[i] = m;
        }
        mask
    }

    #[test]
    fn test_count_mismatch_perfect_match() {
        let seq = b"ACGTACGTACGTACGT";
        let query = pack_forward(seq, 1);
        let ref_seq = make_ref_seq(seq);
        // 使用全 1 mask，表示所有碱基都有效
        let mask = vec![u64::MAX; query.len()];

        let mismatches = count_mismatch(&query, 0, &ref_seq, &mask, 10, 0, false);
        assert_eq!(mismatches, 0, "完全匹配应该返回 0 mismatch");
    }

    #[test]
    fn test_count_mismatch_single_mismatch() {
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ACGTACGTACGTACGA"; // 最后一个 T->A

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        // 使用全 1 mask，表示所有碱基都有效
        let mask = vec![u64::MAX; query.len()];

        let mismatches = count_mismatch(&query, 0, &ref_seq, &mask, 10, 0, false);
        assert_eq!(mismatches, 1, "应该检测到 1 个 mismatch");
    }

    #[test]
    fn test_count_mismatch_ignores_masked_n_by_default() {
        let query = pack_forward(b"ACAT", 10);
        let ref_seq = make_ref_seq(b"ACGT");
        let mut mask = vec![u64::MAX; query.len()];
        mask[0] &= !(0b11u64 << 58);

        let scalar = count_mismatch(&query, 0, &ref_seq, &mask, 10, 1, false);
        let simd = count_mismatch_simd(&query, 0, &ref_seq, &mask, 10, 1, false);

        assert_eq!(scalar, 0);
        assert_eq!(simd, 0);
    }

    #[test]
    fn test_count_mismatch_ct_tolerance() {
        // 读段有 T，参考有 C，在亚硫酸氢盐测序中应该算作匹配
        let query_seq = b"ACGTACGTACGTACGT"; // 位置 3 是 T
        let ref_seq_bytes = b"ACGCACGTACGTACGT"; // 位置 3 是 C

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        // 使用全 1 mask，表示所有碱基都有效
        let mask = vec![u64::MAX; query.len()];

        // 非 nt3 模式：T 应该匹配 C
        let mismatches = count_mismatch(&query, 0, &ref_seq, &mask, 10, 0, false);
        assert_eq!(mismatches, 0, "T 应该匹配 C（C→T 容忍）");

        // nt3 模式：T 和 C 是不同的编码
        let mismatches_nt3 = count_mismatch(&query, 0, &ref_seq, &mask, 10, 0, true);
        assert_eq!(mismatches_nt3, 0, "nt3 模式下 T 仍应匹配 C（C→T 容忍）");
    }

    #[test]
    fn test_count_mismatch_early_abort() {
        let query_seq = b"AAAAAAAAAAAAAAAA"; // 全 A
        let ref_seq_bytes = b"CCCCCCCCCCCCCCCC"; // 全 C（应该全是 mismatch）

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        // 使用全 1 mask，表示所有碱基都有效
        let mask = vec![u64::MAX; query.len()];

        // 设置阈值为 5，应该提前返回
        let mismatches = count_mismatch(&query, 0, &ref_seq, &mask, 5, 0, false);
        assert!(mismatches > 5, "应该提前返回，结果应大于阈值");
    }

    #[test]
    fn test_count_mismatch_with_offset() {
        // 测试 offset 功能：query 和 ref 使用相同的序列
        // offset 为 0 时应该完全匹配
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ACGTACGTACGTACGT";

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        // 使用全 1 mask，表示所有碱基都有效
        let mask = vec![u64::MAX; query.len()];

        // offset = 0 应该完全匹配
        let mismatches = count_mismatch(&query, 0, &ref_seq, &mask, 10, 0, false);
        assert_eq!(mismatches, 0, "offset=0 时应该完全匹配");
    }

    #[test]
    fn count_mismatch_supports_cpp_style_query_shift_offset() {
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"AAACGTACGTACGTACGT";

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        let mask = make_mask(query_seq.len());

        let mismatches = count_mismatch(&query, 4, &ref_seq, &mask, 10, 0, false);
        assert_eq!(mismatches, 0, "offset=4 应该跳过参考前两个 A");
    }

    #[test]
    fn test_mismatch_pattern_0() {
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ACGTACGTACGTACGA"; // 最后一个 T->A

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);

        let positions = mismatch_pattern_0(&query, &ref_seq, 0, 16, false);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], 15); // 最后一个位置（0-based）
    }

    #[test]
    fn test_mismatch_pattern_0_multiple() {
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ATGTACGTACGTACGA"; // 位置 1 和 15 有 mismatch

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);

        let positions = mismatch_pattern_0(&query, &ref_seq, 0, 16, false);
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], 1);
        assert_eq!(positions[1], 15);
    }

    #[test]
    fn test_mismatch_pattern_1() {
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ACGTACGTACGTACGA"; // 最后一个 T->A

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);

        let positions = mismatch_pattern_1(&query, &ref_seq, 0, 16, false);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0], 15);
    }

    #[test]
    fn test_mismatch_pattern_1_order() {
        // 测试反向遍历的顺序
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ATGTACGTACGTACGA"; // 位置 1 和 15

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);

        let positions = mismatch_pattern_1(&query, &ref_seq, 0, 16, false);
        assert_eq!(positions.len(), 2);
        // 反向遍历应该从后往前记录
        assert_eq!(positions[0], 15);
        assert_eq!(positions[1], 1);
    }

    #[test]
    fn test_simd_consistency() {
        // 验证 SIMD 版本和标量版本结果一致
        let query_seq = b"ACGTACGTACGTACGTACGTACGTACGTACGT";
        let ref_seq_bytes = b"ATGTACGTACGTTCGTACGTACGTACGTACGA";

        let query = pack_forward(query_seq, 2);
        let ref_seq = make_ref_seq(ref_seq_bytes);
        // 使用全 1 mask，长度与 query 相同
        let mask = vec![u64::MAX; query.len()];

        let scalar = count_mismatch(&query, 0, &ref_seq, &mask, 100, 0, false);
        let simd = count_mismatch_simd(&query, 0, &ref_seq, &mask, 100, 0, false);

        assert_eq!(scalar, simd, "SIMD 版本和标量版本结果应该一致");
    }

    #[test]
    fn test_mismatch_pattern_ct_tolerance() {
        // 测试 mismatch_pattern 也应用 C→T 容忍
        let query_seq = b"ACGTACGTACGTACGT";
        let ref_seq_bytes = b"ACGCACGTACGTACGT"; // 位置 3: T vs C

        let query = pack_forward(query_seq, 1);
        let ref_seq = make_ref_seq(ref_seq_bytes);

        let positions = mismatch_pattern_0(&query, &ref_seq, 0, 16, false);
        assert!(positions.is_empty(), "C→T 容忍应该过滤掉位置 3 的 mismatch");

        let positions_nt3 = mismatch_pattern_0(&query, &ref_seq, 0, 16, true);
        assert!(positions_nt3.is_empty(), "nt3 模式下 C→T 容忍仍应过滤掉位置 3 的 mismatch");
    }
}
