//! 配对哈希索引模块。
//!
//! ## P4-4 优化内容
//!
//! 使用哈希表加速配对查找，将O(n²)复杂度降低到O(n)。
//!
//! 原始算法：双重循环遍历所有命中组合
//! 优化算法：构建read_a命中的哈希索引，快速查找匹配的read_b命中

use crate::param::GHit;
use std::collections::HashMap;

/// 配对索引键。
///
/// 用于快速查找配对的键：(染色体, 链方向)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PairIndexKey {
    chr: u32,
    strand: u8,
}

impl From<&GHit> for PairIndexKey {
    fn from(hit: &GHit) -> Self {
        Self {
            chr: hit.chr,
            strand: hit.strand,
        }
    }
}

/// 配对索引。
///
/// 为read_a的命中构建索引，支持O(1)查找匹配的read_b命中。
pub struct PairIndex<'a> {
    /// 按(染色体, 链方向)分组的命中
    index: HashMap<PairIndexKey, Vec<&'a GHit>>,
}

impl<'a> PairIndex<'a> {
    /// 从read_a的命中构建索引。
    ///
    /// # 参数
    /// * `hits_a` - read_a的命中列表
    ///
    /// # 时间复杂度
    /// O(n)，其中n是hits_a的长度
    pub fn build(hits_a: &'a [GHit]) -> Self {
        let mut index: HashMap<PairIndexKey, Vec<&'a GHit>> = HashMap::new();
        
        for hit in hits_a {
            let key = PairIndexKey::from(hit);
            index.entry(key).or_default().push(hit);
        }
        
        Self { index }
    }
    
    /// 查找与read_b命中匹配的配对。
    ///
    /// # 参数
    /// * `hit_b` - read_b的命中
    /// * `insert_range` - 允许的insert size范围 (min, max)
    /// * `read_len_a` - read_a的长度
    /// * `read_len_b` - read_b的长度
    ///
    /// # 返回
    /// 匹配的read_a命中列表
    ///
    /// # 时间复杂度
    /// O(1)平均查找 + O(k)遍历匹配项，k是该组的命中数
    pub fn find_matches(
        &self,
        hit_b: &GHit,
        insert_range: (u32, u32),
        read_len_a: u32,
        read_len_b: u32,
    ) -> Vec<&'a GHit> {
        let key = PairIndexKey::from(hit_b);
        
        match self.index.get(&key) {
            Some(hits_a) => {
                let (min_insert, max_insert) = insert_range;
                
                hits_a
                    .iter()
                    .filter(|&&hit_a| {
                        let insert = calculate_insert_size(
                            hit_a,
                            hit_b,
                            read_len_a,
                            read_len_b,
                        );
                        insert >= min_insert && insert <= max_insert
                    })
                    .copied()
                    .collect()
            }
            None => Vec::new(),
        }
    }
    
    /// 批量查找所有配对。
    ///
    /// # 参数
    /// * `hits_b` - read_b的命中列表
    /// * `insert_range` - 允许的insert size范围
    /// * `read_len_a` - read_a的长度
    /// * `read_len_b` - read_b的长度
    ///
    /// # 返回
    /// (read_a命中, read_b命中)配对列表
    pub fn find_all_pairs(
        &self,
        hits_b: &[GHit],
        insert_range: (u32, u32),
        read_len_a: u32,
        read_len_b: u32,
    ) -> Vec<(&'a GHit, &GHit)> {
        let mut pairs = Vec::new();
        
        for hit_b in hits_b {
            let matches = self.find_matches(hit_b, insert_range, read_len_a, read_len_b);
            for hit_a in matches {
                pairs.push((hit_a, hit_b));
            }
        }
        
        pairs
    }
    
    /// 获取索引中的命中总数。
    pub fn len(&self) -> usize {
        self.index.values().map(|v| v.len()).sum()
    }
    
    /// 检查索引是否为空。
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

/// 计算insert size。
///
/// 根据链方向和位置计算配对间的距离。
#[inline]
fn calculate_insert_size(
    hit_a: &GHit,
    hit_b: &GHit,
    read_len_a: u32,
    read_len_b: u32,
) -> u32 {
    let ref_chain_a = hit_a.strand >> 1;
    
    if ref_chain_a == 0 {
        // 正向参考链
        hit_b.loc.saturating_add(read_len_b).saturating_sub(hit_a.loc)
    } else {
        // 反向参考链
        hit_a.loc.saturating_add(read_len_a).saturating_sub(hit_b.loc)
    }
}

/// 优化的配对查找函数。
///
/// 使用哈希索引替代双重循环，显著提升性能。
///
/// # 参数
/// * `hits_a` - read_a的命中
/// * `hits_b` - read_b的命中
/// * `insert_range` - insert size范围
/// * `read_len_a` - read_a长度
/// * `read_len_b` - read_b长度
///
/// # 返回
/// 配对列表
pub fn find_pairs_optimized(
    hits_a: &[GHit],
    hits_b: &[GHit],
    insert_range: (u32, u32),
    read_len_a: u32,
    read_len_b: u32,
) -> Vec<(GHit, GHit)> {
    // 选择较短的列表构建索引
    let (index_hits, probe_hits, len_a, len_b) = if hits_a.len() <= hits_b.len() {
        (hits_a, hits_b, read_len_a, read_len_b)
    } else {
        (hits_b, hits_a, read_len_b, read_len_a)
    };
    
    let index = PairIndex::build(index_hits);
    let mut pairs = Vec::new();
    
    for hit_b in probe_hits {
        let matches = index.find_matches(hit_b, insert_range, len_a, len_b);
        for hit_a in matches {
            // 恢复原始顺序
            if hits_a.len() <= hits_b.len() {
                pairs.push((*hit_a, *hit_b));
            } else {
                pairs.push((*hit_b, *hit_a));
            }
        }
    }
    
    pairs
}

/// 带提前终止的配对查找。
///
/// 找到第一个有效配对后立即返回，用于快速检查是否存在配对。
pub fn has_valid_pair(
    hits_a: &[GHit],
    hits_b: &[GHit],
    insert_range: (u32, u32),
    read_len_a: u32,
    read_len_b: u32,
) -> bool {
    let index = PairIndex::build(hits_a);
    
    for hit_b in hits_b {
        let matches = index.find_matches(hit_b, insert_range, read_len_a, read_len_b);
        if !matches.is_empty() {
            return true;
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_hit(chr: u32, loc: u32, strand: u8) -> GHit {
        GHit {
            loc,
            chr,
            strand,
            gap_size: 0,
            gap_pos: 0,
            snps: 0,
        }
    }
    
    #[test]
    fn test_pair_index_build() {
        let hits = vec![
            make_hit(0, 100, 0),
            make_hit(0, 200, 0),
            make_hit(1, 150, 1),
        ];
        
        let index = PairIndex::build(&hits);
        assert_eq!(index.len(), 3);
    }
    
    #[test]
    fn test_find_matches() {
        let hits_a = vec![
            make_hit(0, 100, 0),  // chr=0, strand=0
            make_hit(0, 200, 0),  // chr=0, strand=0
        ];
        
        let index = PairIndex::build(&hits_a);
        
        // 匹配的hit_b
        let hit_b = make_hit(0, 300, 0);
        let matches = index.find_matches(&hit_b, (50, 500), 50, 50);
        assert_eq!(matches.len(), 2);
        
        // 不匹配的hit_b（不同染色体）
        let hit_b2 = make_hit(1, 300, 0);
        let matches2 = index.find_matches(&hit_b2, (50, 500), 50, 50);
        assert_eq!(matches2.len(), 0);
    }
    
    #[test]
    fn test_find_pairs_optimized() {
        let hits_a = vec![
            make_hit(0, 100, 0),
            make_hit(0, 200, 0),
        ];
        
        let hits_b = vec![
            make_hit(0, 300, 0),
            make_hit(0, 400, 0),
        ];
        
        let pairs = find_pairs_optimized(&hits_a, &hits_b, (50, 500), 50, 50);
        
        // 应该有4对：(100,300), (100,400), (200,300), (200,400)
        assert_eq!(pairs.len(), 4);
    }
    
    #[test]
    fn test_insert_size_calculation() {
        let hit_a = make_hit(0, 100, 0);  // 正向链
        let hit_b = make_hit(0, 300, 0);
        
        let insert = calculate_insert_size(&hit_a, &hit_b, 50, 50);
        assert_eq!(insert, 250);  // 300 + 50 - 100 = 250
        
        // 反向链
        let hit_a_rev = make_hit(0, 300, 2);  // strand=2表示反向链
        let hit_b_rev = make_hit(0, 100, 2);
        
        let insert_rev = calculate_insert_size(&hit_a_rev, &hit_b_rev, 50, 50);
        assert_eq!(insert_rev, 250);  // 300 + 50 - 100 = 250
    }
}
