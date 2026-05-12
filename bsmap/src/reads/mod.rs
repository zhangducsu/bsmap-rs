//! 读段加载模块（Phase 2）。
//!
//! 对应 C++ 的 reads.cpp/h，负责从 FASTA/FASTQ/SAM/BAM 文件中
//! 流式加载读段数据，并进行质量修剪、N 过滤、adapter 修剪等预处理。
//!
//! ## 子模块
//! - [`fastq`]：使用 needletail 解析 FASTA/FASTQ 文件
//! - [`bam`]：使用 noodles 解析 SAM/BAM 文件
//! - [`batch`]：批量管理、质量修剪、N 过滤、adapter 修剪
//! - [`encode`]：读段二进制编码，用于比对引擎

pub mod batch;
pub mod bam;
pub mod encode;
pub mod fastq;

// 从子模块 re-export 关键类型，方便外部使用
pub use batch::process_batch;
pub use encode::{encode_read, EncodedRead};
pub use fastq::{FastqReader, RawRead};
