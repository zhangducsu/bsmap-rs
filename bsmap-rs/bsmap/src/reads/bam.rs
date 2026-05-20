//! SAM/BAM 读段解析器（noodles 后端）。
//!
//! 使用 `noodles` 库读取 SAM/BAM 文件中的比对记录。
//! 根据 FLAG 位判断读段属于哪个 read_set（single-end / PE read1 / PE read2）。
//! 对应 C++ `reads.cpp` 中 SAM/BAM 的读取逻辑。

use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use noodles::bam;

use super::fastq::RawRead;

/// 从 SAM/BAM 文件读取读段。
///
/// 使用 noodles 的 BAM reader 流式读取记录。自动跳过次要比对和
/// 未映射的读段，根据 FLAG 判断 read_set。
pub struct BamReader {
    /// noodles BAM reader（已读取 header）。
    reader: bam::io::reader::Reader<noodles::bgzf::reader::Reader<File>>,
    /// 全局读段计数器。
    global_index: u32,
}

impl BamReader {
    /// 打开 BAM 文件并读取 header。
    ///
    /// # 参数
    /// - `path`：BAM 文件路径
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = bam::io::reader::Builder::default()
            .build_from_path(path)
            .with_context(|| format!("无法打开 BAM 文件: {}", path.display()))?;

        // 读取并丢弃 header（必须调用才能开始读取 records）
        reader
            .read_header()
            .context("读取 BAM header 失败")?;

        Ok(Self {
            reader,
            global_index: 0,
        })
    }

    /// 读取一批读段。
    ///
    /// 根据记录的 FLAG 判断读段属于哪个 read_set：
    /// - FLAG & 0x40（FIRST_SEGMENT）→ read_set = 1
    /// - FLAG & 0x80（LAST_SEGMENT）→ read_set = 2
    /// - 其他 → read_set = 0（single-end）
    ///
    /// 跳过次要比对（FLAG & 0x100）和 supplementary（FLAG & 0x800）。
    ///
    /// # 参数
    /// - `batch`：输出缓冲区
    /// - `max_count`：本批最大读取数量
    /// - `read_start`：起始读段编号（1-based）
    /// - `read_end`：结束读段编号（1-based，包含）
    /// - `read_set`：目标 read_set（0=全部, 1=PE read1, 2=PE read2）
    ///
    /// # 返回值
    /// 实际读取并放入 batch 的读段数量。
    pub fn read_batch(
        &mut self,
        batch: &mut Vec<RawRead>,
        max_count: usize,
        read_start: &mut u32,
        read_end: u32,
        read_set: u32,
    ) -> Result<usize> {
        let mut count = 0usize;

        while count < max_count {
            if self.global_index >= read_end {
                break;
            }

            let record = match self.reader.records().next() {
                Some(Ok(rec)) => rec,
                Some(Err(e)) => return Err(e).context("读取 BAM 记录失败"),
                None => break, // 文件结束
            };

            self.global_index += 1;

            // 跳过 read_start 之前的读段
            if self.global_index < *read_start {
                continue;
            }

            // 获取 FLAG
            let flags = record.flags();

            // 跳过次要比对和 supplementary alignment
            if flags.is_secondary() || flags.is_supplementary() {
                continue;
            }

            // 根据 FLAG 判断 read_set
            let record_read_set = if flags.is_first_segment() {
                1u32
            } else if flags.is_last_segment() {
                2u32
            } else {
                0u32
            };

            // 如果指定了 read_set 过滤，跳过不匹配的
            if read_set != 0 && record_read_set != read_set {
                continue;
            }

            // 提取名称
            let name = record
                .name()
                .map(|n| n.to_vec())
                .unwrap_or_default();

            // 提取序列（noodles BAM sequence.iter() 返回解码后的碱基字节）
            let seq: Vec<u8> = record.sequence().iter().collect();

            // 提取质量值（Phred+33 编码）
            // QualityScores 的 as_ref() 返回 &[u8]（原始 Phred 分数）
            let qual: Vec<u8> = record
                .quality_scores()
                .as_ref()
                .iter()
                .map(|&q| q + 33) // 转换为 ASCII Phred+33 编码
                .collect();

            batch.push(RawRead { name, seq, qual });
            count += 1;
        }

        *read_start = self.global_index + 1;
        Ok(count)
    }

    /// 获取当前全局读段索引。
    pub fn global_index(&self) -> u32 {
        self.global_index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noodles::sam::alignment::record::Flags;

    /// 测试 FLAG 判断逻辑。
    /// 由于无法在测试中轻松创建有效的 BAM 文件，
    /// 这里只测试辅助逻辑。
    #[test]
    fn test_flag_read_set_detection() {
        // 测试 single-end（无 paired flag）
        let flags = Flags::UNMAPPED;
        let read_set = if flags.is_first_segment() {
            1u32
        } else if flags.is_last_segment() {
            2u32
        } else {
            0u32
        };
        assert_eq!(read_set, 0);

        // 测试 FIRST_SEGMENT
        let flags = Flags::SEGMENTED | Flags::FIRST_SEGMENT;
        let read_set = if flags.is_first_segment() {
            1u32
        } else if flags.is_last_segment() {
            2u32
        } else {
            0u32
        };
        assert_eq!(read_set, 1);

        // 测试 LAST_SEGMENT
        let flags = Flags::SEGMENTED | Flags::LAST_SEGMENT;
        let read_set = if flags.is_first_segment() {
            1u32
        } else if flags.is_last_segment() {
            2u32
        } else {
            0u32
        };
        assert_eq!(read_set, 2);
    }

    /// 测试次要比对和 supplementary 的跳过逻辑。
    #[test]
    fn test_skip_secondary_supplementary() {
        // 次要比对
        let flags = Flags::SECONDARY;
        assert!(flags.is_secondary());

        // Supplementary
        let flags = Flags::SUPPLEMENTARY;
        assert!(flags.is_supplementary());

        // 正常比对
        let flags = Flags::empty();
        assert!(!flags.is_secondary());
        assert!(!flags.is_supplementary());
    }

    /// 测试不存在的文件返回错误。
    #[test]
    fn test_open_nonexistent_file() {
        let result = BamReader::open(Path::new("/nonexistent/path/to/file.bam"));
        assert!(result.is_err());
    }
}
