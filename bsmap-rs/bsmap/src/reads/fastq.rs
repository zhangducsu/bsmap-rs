//! FASTA/FASTQ 读段解析器（needletail 后端）。
//!
//! 使用 `needletail` 的零拷贝解析器流式读取 FASTA 和 FASTQ 文件。
//! 支持普通文件和 gzip 压缩文件。
//! 对应 C++ `reads.cpp` 中 FASTA/FASTQ 的读取逻辑。

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use needletail::{parse_fastx_file, parse_fastx_reader, FastxReader};

/// 读段记录（解析后的原始数据）。
///
/// 包含读段名称、序列和质量值。FASTA 格式没有质量值时，
/// qual 字段填充为默认质量值（由调用方指定）。
#[derive(Debug, Clone)]
pub struct RawRead {
    /// 读段名称（不含 '>' 或 '@' 前缀）。
    pub name: Vec<u8>,
    /// 序列（大写 ACGTN）。
    pub seq: Vec<u8>,
    /// 质量值（ASCII 编码），FASTA 时为空或填充默认值。
    pub qual: Vec<u8>,
}

/// 从 FASTQ/FASTA 文件流式读取读段。
///
/// 使用 needletail 的 `parse_fastx_file` 自动检测格式（FASTA 或 FASTQ），
/// 并支持 gzip 压缩文件的透明解压。
///
/// # 用法
///
/// ```ignore
/// let mut reader = FastqReader::open(&path, false)?;
/// let mut batch = Vec::new();
/// let mut read_start = 1u32;
/// let count = reader.read_batch(&mut batch, 1000, &mut read_start, u32::MAX)?;
/// ```
pub struct FastqReader {
    /// needletail 的 FastxReader，支持 FASTA 和 FASTQ。
    reader: Box<dyn FastxReader>,
    /// 是否为 FASTA 格式（无质量值）。
    is_fasta: bool,
    /// 全局读段计数器，用于 read_start/read_end 范围控制。
    global_index: u32,
}

impl FastqReader {
    /// 打开 FASTA/FASTQ 文件。
    ///
    /// 自动检测文件格式（FASTA 或 FASTQ）。如果 `gz` 为 true，
    /// 使用 flate2 进行 gzip 解压。
    ///
    /// # 参数
    /// - `path`：输入文件路径
    /// - `gz`：是否为 gzip 压缩文件
    pub fn open(path: &Path, gz: bool) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("无法打开读段文件: {}", path.display()))?;

        if gz {
            use flate2::bufread::GzDecoder;
            let buf_reader = BufReader::new(file);
            let gz_decoder = GzDecoder::new(buf_reader);
            let reader = parse_fastx_reader(gz_decoder)
                .context("无法创建 gzip FASTA/FASTQ 解析器")?;
            Ok(Self {
                reader,
                is_fasta: false, // 运行时根据记录判断
                global_index: 0,
            })
        } else {
            let reader = parse_fastx_file(path)
                .with_context(|| format!("无法解析读段文件: {}", path.display()))?;
            Ok(Self {
                reader,
                is_fasta: false,
                global_index: 0,
            })
        }
    }

    /// 从 `std::io::Read` 创建解析器（用于测试）。
    pub fn from_read<R: Read + Send + 'static>(reader: R) -> Result<Self> {
        let fastx = parse_fastx_reader(reader).context("无法从 Read 创建解析器")?;
        Ok(Self {
            reader: fastx,
            is_fasta: false,
            global_index: 0,
        })
    }

    /// 读取一批读段。
    ///
    /// 自动跳过前 `read_start - 1` 条读段（通过 `read_start` 的当前值判断），
    /// 在 `read_end` 处停止。返回实际读取的读段数量。
    ///
    /// # 参数
    /// - `batch`：输出缓冲区，读段将追加到此向量
    /// - `max_count`：本批最大读取数量
    /// - `read_start`：起始读段编号（1-based），函数内部会递增
    /// - `read_end`：结束读段编号（1-based，包含）
    ///
    /// # 返回值
    /// 实际读取并放入 batch 的读段数量。
    pub fn read_batch(
        &mut self,
        batch: &mut Vec<RawRead>,
        max_count: usize,
        read_start: &mut u32,
        read_end: u32,
    ) -> Result<usize> {
        let mut count = 0usize;

        while count < max_count {
            // 检查是否已到达读段范围上限
            if self.global_index >= read_end {
                break;
            }

            // 读取下一条记录
            let record = match self.reader.next() {
                Some(Ok(rec)) => rec,
                Some(Err(e)) => return Err(e).context("读取 FASTA/FASTQ 记录失败"),
                None => break, // 文件结束
            };

            self.global_index += 1;

            // 跳过 read_start 之前的读段
            if self.global_index < *read_start {
                continue;
            }

            // 提取名称（去掉首尾空白）
            let name = record.id().to_vec();

            // 提取序列并转为大写
            let mut seq: Vec<u8> = record.seq().to_vec();
            seq.make_ascii_uppercase();

            // 提取质量值：FASTA 格式没有质量行，qual 为 None
            let qual = record.qual().map(|q| q.to_vec()).unwrap_or_default();
            self.is_fasta = qual.is_empty();

            batch.push(RawRead { name, seq, qual });
            count += 1;
        }

        // 更新 read_start 为下一次读取的起始位置
        *read_start = self.global_index + 1;

        Ok(count)
    }

    /// 获取当前全局读段索引（已读取的总数，含被跳过的）。
    pub fn global_index(&self) -> u32 {
        self.global_index
    }

    /// 判断是否为 FASTA 格式（无质量值）。
    pub fn is_fasta(&self) -> bool {
        self.is_fasta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// 创建一个简单的 FASTQ 数据用于测试。
    fn fastq_data() -> Vec<u8> {
        b"@read1\nACGTACGT\n+\nIIIIIIII\n@read2\nTGCA\n+\n!!!!\n@read3\nAAAA\n+\nIIII\n".to_vec()
    }

    /// 创建一个简单的 FASTA 数据用于测试。
    fn fasta_data() -> Vec<u8> {
        b">read1\nACGTACGT\n>read2\nTGCA\n>read3\nAAAA\n".to_vec()
    }

    #[test]
    fn test_fastq_parse() {
        let cursor = Cursor::new(fastq_data());
        let mut reader = FastqReader::from_read(cursor).unwrap();

        let mut batch = Vec::new();
        let mut start = 1u32;
        let count = reader.read_batch(&mut batch, 100, &mut start, u32::MAX).unwrap();

        assert_eq!(count, 3);
        assert_eq!(batch[0].name, b"read1");
        assert_eq!(batch[0].seq, b"ACGTACGT");
        assert_eq!(batch[0].qual, b"IIIIIIII");
        assert_eq!(batch[1].name, b"read2");
        assert_eq!(batch[1].seq, b"TGCA");
        assert_eq!(batch[1].qual, b"!!!!");
        assert_eq!(batch[2].name, b"read3");
        assert_eq!(batch[2].seq, b"AAAA");
        assert_eq!(batch[2].qual, b"IIII");
    }

    #[test]
    fn test_fasta_parse() {
        let cursor = Cursor::new(fasta_data());
        let mut reader = FastqReader::from_read(cursor).unwrap();

        let mut batch = Vec::new();
        let mut start = 1u32;
        let count = reader.read_batch(&mut batch, 100, &mut start, u32::MAX).unwrap();

        assert_eq!(count, 3);
        assert_eq!(batch[0].name, b"read1");
        assert_eq!(batch[0].seq, b"ACGTACGT");
        assert!(batch[0].qual.is_empty(), "FASTA 格式应无质量值");
        assert!(reader.is_fasta(), "应检测为 FASTA 格式");
    }

    #[test]
    fn test_batch_limit() {
        let cursor = Cursor::new(fastq_data());
        let mut reader = FastqReader::from_read(cursor).unwrap();

        // 只读取 2 条
        let mut batch = Vec::new();
        let mut start = 1u32;
        let count = reader.read_batch(&mut batch, 2, &mut start, u32::MAX).unwrap();
        assert_eq!(count, 2);
        assert_eq!(batch.len(), 2);

        // 继续读取剩余的
        let count2 = reader.read_batch(&mut batch, 2, &mut start, u32::MAX).unwrap();
        assert_eq!(count2, 1);
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn test_read_start_skip() {
        let cursor = Cursor::new(fastq_data());
        let mut reader = FastqReader::from_read(cursor).unwrap();

        // 从第 3 条开始读
        let mut batch = Vec::new();
        let mut start = 3u32;
        let count = reader.read_batch(&mut batch, 100, &mut start, u32::MAX).unwrap();

        assert_eq!(count, 1);
        assert_eq!(batch[0].name, b"read3");
    }

    #[test]
    fn test_read_end_limit() {
        let cursor = Cursor::new(fastq_data());
        let mut reader = FastqReader::from_read(cursor).unwrap();

        // 只读第 1-2 条
        let mut batch = Vec::new();
        let mut start = 1u32;
        let count = reader.read_batch(&mut batch, 100, &mut start, 2).unwrap();

        assert_eq!(count, 2);
        assert_eq!(batch[0].name, b"read1");
        assert_eq!(batch[1].name, b"read2");
    }

    #[test]
    fn test_empty_file() {
        // needletail 对空文件报错，这是已知行为
        let cursor = Cursor::new(b"");
        let result = FastqReader::from_read(cursor);
        assert!(result.is_err(), "空文件应返回解析错误");
    }

    #[test]
    fn test_multiline_fasta() {
        let data = b">read1\nACGT\nTGCA\n>read2\nAAAA\nCCCC\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = FastqReader::from_read(cursor).unwrap();

        let mut batch = Vec::new();
        let mut start = 1u32;
        reader.read_batch(&mut batch, 100, &mut start, u32::MAX).unwrap();

        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].seq, b"ACGTTGCA");
        assert_eq!(batch[1].seq, b"AAAACCCC");
    }

    #[test]
    fn test_lowercase_sequence() {
        let data = b"@read1\nacgtacgt\n+\nIIIIIIII\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = FastqReader::from_read(cursor).unwrap();

        let mut batch = Vec::new();
        let mut start = 1u32;
        reader.read_batch(&mut batch, 100, &mut start, u32::MAX).unwrap();

        assert_eq!(batch[0].seq, b"ACGTACGT", "序列应转为大写");
    }
}
