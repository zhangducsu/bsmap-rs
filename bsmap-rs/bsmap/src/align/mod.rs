//! 比对引擎模块（Phase 3）。
//!
//! 这是 BSMAP 亚硫酸氢盐测序比对器的核心比对引擎，对应 C++ 的 align.cpp/h。
//! 提供种子扩展比对、gap 检测、命中收集和 SAM/BSP 输出格式化功能。
//!
//! ## 模块结构
//!
//! - `mismatch`: 位并行 mismatch 计数（核心热路径）
//! - `seed`: 种子提取、重排序和索引查找
//! - `gap`: Gap 比对算法
//! - `extend`: 种子扩展和命中收集
//! - `engine`: 单端比对引擎主控
//! - `output`: SAM/BSP 输出格式化
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use bsmap::align::{SingleAlign, AlignConfig, AlignmentResult};
//!
//! let mut engine = SingleAlign::new();
//! let has_hits = engine.run_align(&encoded_read, &index, &coll, &config);
//! ```

pub mod engine;
pub mod extend;
pub mod gap;
pub mod mismatch;
pub mod output;
pub mod profile;
pub mod seed;

// 重新导出核心类型，方便外部使用
pub use crate::param::AlignConfig;
pub use engine::{AlignmentResult, SingleAlign};
pub use crate::param::{GHit, Hit};
pub use gap::GapResult;
pub use mismatch::MismatchResult;
pub use output::{format_bsp, format_sam, OutputFormat};
pub use seed::SeedSegment;

/// 比对链标识。
///
/// BSMAP 支持四种链组合：
/// - `PlusPlus`: 参考正义链 + 读段正义链（BSW++）
/// - `PlusMinus`: 参考正义链 + 读段反义链（BSC+-）
/// - `MinusPlus`: 参考反义链 + 读段正义链（BSW-+）
/// - `MinusMinus`: 参考反义链 + 读段反义链（BSC--）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Chain {
    /// 参考正义链 + 读段正义链（BSW++）
    PlusPlus = 0,
    /// 参考正义链 + 读段反义链（BSC+-）
    PlusMinus = 1,
    /// 参考反义链 + 读段正义链（BSW-+）
    MinusPlus = 2,
    /// 参考反义链 + 读段反义链（BSC--）
    MinusMinus = 3,
}

impl Chain {
    /// 从 strand 编码创建 Chain。
    ///
    /// strand 编码：`ref_chain << 1 | read_chain`
    /// - ref_chain: 0=正义链, 1=反义链
    /// - read_chain: 0=正义链, 1=反义链
    #[inline]
    pub fn from_strand(strand: u8) -> Self {
        match strand & 0b11 {
            0 => Chain::PlusPlus,
            1 => Chain::PlusMinus,
            2 => Chain::MinusPlus,
            3 => Chain::MinusMinus,
            _ => unreachable!(),
        }
    }

    /// 转换为 strand 编码。
    #[inline]
    pub fn to_strand(self) -> u8 {
        self as u8
    }

    /// 获取参考链（0=正义链, 1=反义链）。
    #[inline]
    pub fn ref_chain(self) -> u8 {
        (self as u8) >> 1
    }

    /// 获取读段链（0=正义链, 1=反义链）。
    #[inline]
    pub fn read_chain(self) -> u8 {
        (self as u8) & 1
    }

    /// 判断是否为正义链参考。
    #[inline]
    pub fn is_ref_forward(self) -> bool {
        self.ref_chain() == 0
    }

    /// 判断是否为正义链读段。
    pub fn is_read_forward(self) -> bool {
        self.read_chain() == 0
    }
}

/// 比对方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Strand {
    /// 正义链（forward）
    Forward = 0,
    /// 反义链（reverse）
    Reverse = 1,
}

impl Strand {
    /// 从另一个 strand 翻转。
    #[inline]
    pub fn flip(self) -> Self {
        match self {
            Strand::Forward => Strand::Reverse,
            Strand::Reverse => Strand::Forward,
        }
    }
}

/// 计算需要的 u64 word 数量。
#[inline]
pub fn num_words_for_len(len: usize) -> usize {
    use crate::param::SEGLEN;
    if len == 0 {
        1
    } else {
        (len + SEGLEN - 1) / SEGLEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_from_strand() {
        assert_eq!(Chain::from_strand(0b00), Chain::PlusPlus);
        assert_eq!(Chain::from_strand(0b01), Chain::PlusMinus);
        assert_eq!(Chain::from_strand(0b10), Chain::MinusPlus);
        assert_eq!(Chain::from_strand(0b11), Chain::MinusMinus);
    }

    #[test]
    fn test_chain_to_strand() {
        assert_eq!(Chain::PlusPlus.to_strand(), 0);
        assert_eq!(Chain::PlusMinus.to_strand(), 1);
        assert_eq!(Chain::MinusPlus.to_strand(), 2);
        assert_eq!(Chain::MinusMinus.to_strand(), 3);
    }

    #[test]
    fn test_chain_ref_read() {
        assert_eq!(Chain::PlusPlus.ref_chain(), 0);
        assert_eq!(Chain::PlusPlus.read_chain(), 0);
        assert_eq!(Chain::MinusMinus.ref_chain(), 1);
        assert_eq!(Chain::MinusMinus.read_chain(), 1);
    }

    #[test]
    fn test_strand_flip() {
        assert_eq!(Strand::Forward.flip(), Strand::Reverse);
        assert_eq!(Strand::Reverse.flip(), Strand::Forward);
    }

    #[test]
    fn test_num_words_for_len() {
        use crate::param::SEGLEN;

        assert_eq!(num_words_for_len(0), 1);
        assert_eq!(num_words_for_len(1), 1);
        assert_eq!(num_words_for_len(SEGLEN), 1);
        assert_eq!(num_words_for_len(SEGLEN + 1), 2);
        assert_eq!(num_words_for_len(SEGLEN * 2), 2);
        assert_eq!(num_words_for_len(SEGLEN * 2 + 1), 3);
    }
}
