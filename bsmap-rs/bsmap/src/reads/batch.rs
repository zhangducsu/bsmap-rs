//! 批量读段管理、质量修剪、N 过滤、adapter 修剪。
//!
//! 将原始读段（`RawRead`）转换为可用于比对的 `ReadInf`，
//! 同时应用各种过滤和修剪策略。
//! 对应 C++ `reads.cpp` 中的 `LoadBatchReads()` 后处理逻辑。

use crate::param::{AlignConfig, ReadInf};

use super::fastq::RawRead;

/// 将一批原始读段转换为 `ReadInf`，应用过滤和修剪。
///
/// 对每条读段依次执行：
/// 1. 截断超过 `max_read_len` 的读段
/// 2. 3'-end 质量修剪
/// 3. Adapter 修剪
/// 4. N 计数过滤
/// 5. 最小长度过滤
///
/// # 参数
/// - `raw_reads`：原始读段列表
/// - `read_set`：读段集合编号（0=single-end, 1=PE read1, 2=PE read2）
/// - `config`：比对配置参数
///
/// # 返回值
/// 通过所有过滤的 `ReadInf` 列表。
pub fn process_batch(
    raw_reads: Vec<RawRead>,
    read_set: u32,
    config: &AlignConfig,
) -> Vec<ReadInf> {
    let mut result = Vec::with_capacity(raw_reads.len());

    for (i, raw) in raw_reads.into_iter().enumerate() {
        // 转换为可变序列
        let mut seq = raw.seq;
        let mut qual = raw.qual;
        let name = String::from_utf8(raw.name).unwrap_or_else(|e| {
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        });

        // 1. 截断超过 max_read_len 的读段
        let max_len = config.max_read_len as usize;
        if seq.len() > max_len {
            seq.truncate(max_len);
            if qual.len() > max_len {
                qual.truncate(max_len);
            }
        }

        // 跳过空序列
        if seq.is_empty() {
            continue;
        }

        // 2. 3'-end 质量修剪
        if !qual.is_empty() && config.qual_threshold > 0 {
            trim_quality(&mut seq, &mut qual, config.qual_threshold, config.zero_qual);
        }

        // 跳过修剪后为空的序列
        if seq.is_empty() {
            continue;
        }

        // 3. Adapter 修剪
        for adapter in &config.adapters {
            let adapter_bytes = adapter.as_bytes();
            trim_adapter(&mut seq, &mut qual, adapter_bytes);
        }

        // 跳过修剪后为空的序列
        if seq.is_empty() {
            continue;
        }

        // 4. N 计数过滤
        let ns = count_ns(&seq);
        if ns as u32 > config.max_ns {
            continue;
        }

        // 5. 最小长度过滤
        if !min_length_check(&seq, config.min_read_size) {
            continue;
        }

        // 如果 FASTA 格式没有质量值，填充默认质量
        let qual = if qual.is_empty() {
            vec![config.zero_qual; seq.len()]
        } else {
            qual
        };

        result.push(ReadInf {
            index: i as u32,
            read_set,
            name,
            seq,
            qual,
        });
    }

    result
}

/// 3'-end 质量修剪：从 3' 端开始，去除质量值低于阈值的碱基。
///
/// 逐个检查 3' 端碱基，如果质量值（ASCII 值 - zero_qual 偏移）低于阈值，
/// 则去除该碱基。遇到第一个合格碱基时停止。
///
/// # 参数
/// - `seq`：序列（将被原地截断）
/// - `qual`：质量值（将被原地截断）
/// - `threshold`：质量阈值（Phred 分数）
/// - `zero_qual`：基础质量偏移（如 '!' = 33）
fn trim_quality(seq: &mut Vec<u8>, qual: &mut Vec<u8>, threshold: u8, zero_qual: u8) {
    while let Some(&q) = qual.last() {
        // 计算实际 Phred 分数
        if q < zero_qual + threshold {
            seq.pop();
            qual.pop();
        } else {
            break;
        }
    }
}

/// 统计序列中 N（或 n）碱基的数量。
///
/// # 参数
/// - `seq`：序列字节切片
///
/// # 返回值
/// N 碱基的数量。
fn count_ns(seq: &[u8]) -> usize {
    seq.iter().filter(|&&b| b == b'N' || b == b'n').count()
}

/// Adapter 修剪：在 3' 端查找 adapter 序列并修剪。
///
/// 在序列的 3' 端查找 adapter 的前缀匹配。如果找到足够长的匹配
/// （至少 adapter 长度的一半），则从匹配位置截断序列。
///
/// # 参数
/// - `seq`：序列（将被原地截断）
/// - `qual`：质量值（将被原地截断）
/// - `adapter`：adapter 序列字节切片
fn trim_adapter(seq: &mut Vec<u8>, qual: &mut Vec<u8>, adapter: &[u8]) {
    if adapter.is_empty() || seq.len() < adapter.len() / 2 {
        return;
    }

    // 最小匹配长度：adapter 长度的一半，至少 4 个碱基
    let min_match = (adapter.len() / 2).max(4);
    let adapter_len = adapter.len();

    // 在序列中搜索 adapter 的前缀
    // 从可能的位置开始搜索（序列末尾 - adapter 长度 + 1）
    let search_start = if seq.len() > adapter_len {
        seq.len() - adapter_len + 1
    } else {
        0
    };

    for i in search_start..seq.len() {
        let remaining = &seq[i..];
        let match_len = remaining
            .iter()
            .zip(adapter.iter())
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();

        if match_len >= min_match {
            // 找到足够长的匹配，截断
            let trim_pos = i;
            seq.truncate(trim_pos);
            qual.truncate(trim_pos);
            return;
        }
    }
}

/// 最小长度检查：判断序列是否满足最小长度要求。
///
/// # 参数
/// - `seq`：序列字节切片
/// - `min_len`：最小长度要求
///
/// # 返回值
/// 如果序列长度 >= min_len，返回 true；否则返回 false。
fn min_length_check(seq: &[u8], min_len: u32) -> bool {
    seq.len() as u32 >= min_len
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个宽松配置，min_read_size 设为较小值以便测试。
    fn test_config() -> AlignConfig {
        let mut config = AlignConfig::default();
        config.min_read_size = 4; // 降低最小长度要求以便测试
        config
    }

    #[test]
    fn test_trim_quality_basic() {
        // 使用 Phred+33 编码的质量值
        // zero_qual=33, threshold=20 → 截止值 53
        // 'I' = 73 (高质量), '#' = 35 (低质量)
        let mut seq = b"ACGTACGT".to_vec();
        let mut qual = vec![73u8, 73, 73, 73, 73, 73, 35, 35]; // 最后两个低质量
        trim_quality(&mut seq, &mut qual, 20, 33);

        assert_eq!(seq, b"ACGTAC");
        assert_eq!(qual, vec![73u8, 73, 73, 73, 73, 73]);
    }

    #[test]
    fn test_trim_quality_all_low() {
        let mut seq = b"ACGT".to_vec();
        let mut qual = vec![35u8, 35, 35, 35]; // 全部低于 33+20=53
        trim_quality(&mut seq, &mut qual, 20, 33);

        assert!(seq.is_empty());
        assert!(qual.is_empty());
    }

    #[test]
    fn test_trim_quality_none_low() {
        let mut seq = b"ACGT".to_vec();
        let mut qual = vec![73u8, 73, 73, 73]; // 全部高质量
        trim_quality(&mut seq, &mut qual, 20, 33);

        assert_eq!(seq, b"ACGT");
        assert_eq!(qual, vec![73u8, 73, 73, 73]);
    }

    #[test]
    fn test_trim_quality_with_zero_qual_offset() {
        // zero_qual = 64 ( '@' )，质量值 64+30=94 以上为合格
        let mut seq = b"ACGTACGT".to_vec();
        let mut qual = vec![100u8, 100, 100, 100, 100, 100, 70, 65]; // 最后两个低质量
        trim_quality(&mut seq, &mut qual, 30, 64);

        assert_eq!(seq, b"ACGTAC");
    }

    #[test]
    fn test_count_ns() {
        assert_eq!(count_ns(b"ACGT"), 0);
        assert_eq!(count_ns(b"ACNGT"), 1);
        assert_eq!(count_ns(b"ACNNGT"), 2);
        assert_eq!(count_ns(b"NNNN"), 4);
        assert_eq!(count_ns(b"acngt"), 1); // 只有 1 个小写 n
        assert_eq!(count_ns(b"ACnNgt"), 2); // 大小写各一个
    }

    #[test]
    fn test_trim_adapter_exact_match() {
        let mut seq = b"ACGTACGTAGATCGGAAGAG".to_vec(); // 末尾有 adapter
        let mut qual = vec![40u8; 19];
        let adapter = b"AGATCGGAAGAGC";

        trim_adapter(&mut seq, &mut qual, adapter);

        // adapter 从位置 8 开始匹配
        assert_eq!(seq, b"ACGTACGT");
        assert_eq!(qual.len(), 8);
    }

    #[test]
    fn test_trim_adapter_partial_match() {
        let mut seq = b"ACGTACGTAGATCGG".to_vec(); // 部分 adapter
        let mut qual = vec![40u8; 15];
        let adapter = b"AGATCGGAAGAGC"; // 13 个碱基，min_match = 6

        trim_adapter(&mut seq, &mut qual, adapter);

        // "AGATCGG" 是 adapter 前 7 个碱基的匹配，>= 6
        assert_eq!(seq, b"ACGTACGT");
    }

    #[test]
    fn test_trim_adapter_no_match() {
        let mut seq = b"ACGTACGT".to_vec();
        let mut qual = vec![40u8; 8];
        let adapter = b"TTTTTTTT";

        trim_adapter(&mut seq, &mut qual, adapter);

        // 没有匹配，不应修剪
        assert_eq!(seq, b"ACGTACGT");
        assert_eq!(qual.len(), 8);
    }

    #[test]
    fn test_trim_adapter_empty() {
        let mut seq = b"ACGT".to_vec();
        let mut qual = vec![40u8; 4];

        trim_adapter(&mut seq, &mut qual, b"");

        assert_eq!(seq, b"ACGT");
    }

    #[test]
    fn test_min_length_check() {
        assert!(min_length_check(b"ACGTACGT", 8));
        assert!(min_length_check(b"ACGTACGT", 4));
        assert!(!min_length_check(b"ACG", 4));
        assert!(min_length_check(b"A", 1));
        assert!(!min_length_check(b"", 1));
    }

    #[test]
    fn test_process_batch_basic() {
        let config = test_config();
        let raw_reads = vec![
            RawRead {
                name: b"read1".to_vec(),
                seq: b"ACGTACGT".to_vec(),
                qual: vec![73u8; 8],
            },
            RawRead {
                name: b"read2".to_vec(),
                seq: b"TGCA".to_vec(),
                qual: vec![73u8; 4],
            },
        ];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "read1");
        assert_eq!(result[0].seq, b"ACGTACGT");
        assert_eq!(result[0].read_set, 0);
        assert_eq!(result[1].name, "read2");
    }

    #[test]
    fn test_process_batch_filter_too_many_ns() {
        let mut config = test_config();
        config.max_ns = 1;

        let raw_reads = vec![
            RawRead {
                name: b"good".to_vec(),
                seq: b"ACGT".to_vec(),
                qual: vec![73u8; 4],
            },
            RawRead {
                name: b"bad".to_vec(),
                seq: b"ACNNGT".to_vec(),
                qual: vec![73u8; 6],
            },
        ];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "good");
    }

    #[test]
    fn test_process_batch_quality_trim() {
        let mut config = test_config();
        config.qual_threshold = 20;
        config.zero_qual = 33;

        let raw_reads = vec![RawRead {
            name: b"read1".to_vec(),
            seq: b"ACGTACGT".to_vec(),
            qual: vec![73u8, 73, 73, 73, 73, 73, 35, 35],
        }];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].seq, b"ACGTAC");
    }

    #[test]
    fn test_process_batch_min_length_filter() {
        let mut config = test_config();
        config.min_read_size = 10;
        config.qual_threshold = 20;
        config.zero_qual = 33;

        let raw_reads = vec![RawRead {
            name: b"short".to_vec(),
            seq: b"ACGTACGT".to_vec(), // 8 个碱基
            qual: vec![73u8; 8],
        }];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 0, "短于 min_read_size 的读段应被过滤");
    }

    #[test]
    fn test_process_batch_truncate_long_read() {
        let mut config = test_config();
        config.max_read_len = 5;

        let raw_reads = vec![RawRead {
            name: b"long".to_vec(),
            seq: b"ACGTACGTACGT".to_vec(), // 12 个碱基
            qual: vec![73u8; 12],
        }];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].seq, b"ACGTA");
        assert_eq!(result[0].qual.len(), 5);
    }

    #[test]
    fn test_process_batch_fasta_default_qual() {
        let config = test_config();

        let raw_reads = vec![RawRead {
            name: b"fasta_read".to_vec(),
            seq: b"ACGTACGT".to_vec(),
            qual: vec![], // FASTA 无质量值
        }];

        let result = process_batch(raw_reads, 0, &config);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].qual.len(), 8);
        // 默认质量值应为 zero_qual ('!' = 33)
        assert!(result[0].qual.iter().all(|&q| q == 33));
    }

    #[test]
    fn test_process_batch_read_set() {
        let config = test_config();

        let raw_reads = vec![RawRead {
            name: b"pe_read".to_vec(),
            seq: b"ACGTACGT".to_vec(),
            qual: vec![73u8; 8],
        }];

        let result = process_batch(raw_reads, 1, &config);

        assert_eq!(result[0].read_set, 1);
    }

    #[test]
    fn test_process_batch_empty_input() {
        let config = test_config();
        let raw_reads: Vec<RawRead> = Vec::new();

        let result = process_batch(raw_reads, 0, &config);

        assert!(result.is_empty());
    }
}
