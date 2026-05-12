//! 配对读段比对模块（Phase 4）。
//!
//! 这是 BSMAP 亚硫酸氢盐测序比对器的配对读段处理模块，对应 C++ 的 pairs.cpp/h。
//! 提供配对比对、insert size 过滤和配对结果输出功能。
//!
//! ## 模块结构
//!
//! - `pair`: 配对逻辑核心，包括双指针配对算法和 insert size 计算
//! - `output`: 配对 SAM 输出格式化
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use bsmap::pairs::{PairAlign, PairResult};
//!
//! let mut pair_aligner = PairAlign::new();
//! let has_pair = pair_aligner.run_pair_align(
//!     &encoded_a, &encoded_b, &index, &coll, &config
//! );
//! ```

pub mod pair;
pub mod output;

pub use pair::{PairAlign, PairHit, PairResult, PairBatchResult};
pub use output::{format_pair_sam, format_unpair_sam, format_unpair_sam_pair};
