use std::collections::HashMap;

/// 统一的比对记录抽象（屏蔽 SAM/BAM/BSP 差异）
/// 对应 methratio.py get_alignment() 返回的 (seq, strand, cr, pos)
#[derive(Debug, Clone)]
pub struct AlignmentRecord {
    /// 读段序列（经过 CIGAR/BSP gap 调整后）
    pub seq: Vec<u8>,
    /// 链方向：'+' 或 '-'（来自 ZS tag 或 BSP strand 字段的首字符）
    pub strand: char,
    /// 染色体名
    pub chrom: String,
    /// 比对起始位置（0-based）
    pub pos: usize,
}

/// 每条染色体的甲基化计数（稀疏 HashMap，替代原版密集 array）
/// 对应 methratio.py 中 meth[cr], depth[cr], meth1[cr], depth1[cr]
#[derive(Debug, Default)]
pub struct ChromosomeCounts {
    /// 甲基化计数（参考 C 位置读段为 C，或参考 G 位置读段为 G）
    pub meth: HashMap<usize, u16>,
    /// 覆盖深度（参考位置有 C 或 G 的总读段数）
    pub depth: HashMap<usize, u16>,
    /// CT_SNP 反向链甲基化计数（可选，CT_SNP > 0 时启用）
    pub meth1: HashMap<usize, u16>,
    /// CT_SNP 反向链深度计数（可选，CT_SNP > 0 时启用）
    pub depth1: HashMap<usize, u16>,
}

/// 运行时配置
/// 对应 methratio.py 中所有 options 变量
#[derive(Debug, Clone)]
pub struct Config {
    pub unique: bool,
    pub pair: bool,
    pub remove_duplicate: bool,
    pub trim_fillin: usize,
    pub combine_cpg: bool,
    pub min_depth: usize,
    pub no_header: bool,
    pub ct_snp: u8,       // 0=no-action, 1=correct, 2=skip
    pub context: Vec<String>, // 空=全部, 或 ["CG"], ["CHG","CHH"] 等
    pub chroms: Vec<String>,  // 空=全部
    pub quiet: bool,
    pub wig_bin: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            unique: false,
            pair: false,
            remove_duplicate: false,
            trim_fillin: 0,
            combine_cpg: false,
            min_depth: 1,
            no_header: false,
            ct_snp: 1, // 默认 correct
            context: vec![],
            chroms: vec![],
            quiet: false,
            wig_bin: 25,
        }
    }
}

/// BS 转换规则
/// 对应 methratio.py: BS_conversion = {'+': ('C','T','G','A'), '-': ('G','A','C','T')}
/// (match_base, convert_base) - 甲基化判定: read_base == match_base
pub const BS_CONVERSION: [(char, char, char); 2] = [
    ('+', 'C', 'T'),  // (strand, match, convert)
    ('-', 'G', 'A'),
];

pub mod input;
pub mod counter;
pub mod output;
