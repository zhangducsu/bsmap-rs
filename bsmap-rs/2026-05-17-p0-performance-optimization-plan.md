# BSMAP-rs P0 性能优化实施计划

> **目标：** 优化 BSMAP-rs 性能，目标达到 BSMAP C++ 的 2-3x 内
> **基于：** 深度代码研究报告.md
> **优化日期：** 2026-05-17

---

## 执行摘要

根据之前的基准测试和代码分析，BSMAP-rs 比 BSMAP C++ 慢 4.6x-6.2x，主要瓶颈在于：

1. **SIMD 优化缺失**：核心哈希计算函数无 SIMD 优化
2. **索引存储分散**：WGBS 索引分离存储导致缓存不友好

---

## P0-1: SIMD 优化核心哈希函数

### 优化目标
- 对 `xt3`、`xc32`/`xc64`、`xm64` 等核心哈希函数进行 SIMD 优化
- 预期收益：**性能提升 2-4x**

### 关键函数清单

| 函数 | 位置 | 当前实现 | 优化目标 |
|------|------|---------|---------|
| `xt3` | `alphabet.rs` | 标量位操作 | AVX2 SIMD |
| `xt3_64` | `alphabet.rs` | 标量位操作 | AVX2 SIMD |
| `xc32` | `alphabet.rs` | 标量位操作 | AVX2 SIMD |
| `xc64` | `alphabet.rs` | 标量位操作 | AVX2 SIMD |
| `xm64` | `alphabet.rs` | SWAR popcount | AVX2 popcnt |

### 文件清单
- 修改：`bsmap/src/alphabet.rs`
- 测试：`bsmap/tests/simd_benchmark.rs`

### 实施步骤

#### 步骤 1: 创建 SIMD 基准测试

```bash
# 创建 SIMD 性能测试
cat > bsmap/tests/simd_benchmark.rs << 'EOF'
#[cfg(test)]
mod simd_bench {
    use bsmap::alphabet::{xt3, xc32, xm64};
    
    #[test]
    fn test_xt3_consistency() {
        for i in 0..10000 {
            let val = xt3(i);
            assert_eq!(val, xt3_ref(i)); // 对比标量实现
        }
    }
    
    // 标量参考实现
    fn xt3_ref(tt: u32) -> u32 {
        let mut t = tt;
        t = t.wrapping_sub((t << 1) & t & 0xAAAA_AAAA);
        t = t.wrapping_sub((t >> 2) & 0x3333_3333);
        t = (t & 0x0F0F_0F0F) * 0x0101_0101 >> 8;
        t % 43
    }
}
EOF
```

#### 步骤 2: 实现 SIMD 优化

```rust
// alphabet.rs - 添加 SIMD 优化版本
#[cfg(target_arch = "x86_64")]
pub fn xt3_simd(values: &[u32]) -> Vec<u32> {
    if is_x86_feature_detected!("avx2") {
        unsafe { xt3_avx2(values) }
    } else {
        xt3_scalar(values) // 回退到标量
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn xt3_avx2(values: &[u32]) -> Vec<u32> {
    use std::arch::x86_64::*;
    
    let mut results = Vec::with_capacity(values.len());
    let chunks = values.chunks(8);
    
    for chunk in chunks {
        let mut arr = [0u32; 8];
        arr[..chunk.len()].copy_from_slice(chunk);
        
        let v = _mm256_loadu_si256(arr.as_ptr() as *const __m256i);
        // SIMD 操作...
        _mm256_storeu_si256(results.as_ptr() as *mut __m256i, v);
    }
    results
}
```

#### 步骤 3: 验证正确性

```bash
cargo test --package bsmap --test simd_benchmark
```

#### 步骤 4: 性能对比

```bash
cargo bench --package bsmap --test simd_benchmark
```

---

## P0-2: 整合 WGBS 索引存储结构

### 优化目标
- 将分散的 `index2`、`positions`、`start_offsets` 整合为单一数据结构
- 预期收益：**内存减少 10-20%，缓存命中率提升**

### 当前数据结构

```rust
// reference/index.rs - 当前实现
pub struct KmerIndex {
    pub index2: Vec<KmerLoc2>,        // 分离存储
    pub positions: Vec<u32>,          // 分离存储
    pub(crate) start_offsets: Vec<u32>, // 分离存储
}

pub struct KmerLoc2 {
    pub n: [u32; 2],
}
```

### 优化后数据结构

```rust
// reference/index.rs - 优化后实现
pub struct KmerIndex {
    pub entries: Vec<KmerEntry>, // 整合存储
}

pub struct KmerEntry {
    pub n: [u32; 2],    // [forward, reverse] count
    pub start: u32,     // 起始偏移
}
```

### 实施步骤

#### 步骤 1: 定义新数据结构

```rust
// reference/index.rs
#[derive(Debug, Clone)]
pub struct KmerEntry {
    pub n: [u32; 2],  // [forward, reverse]
    pub start: u32,   // positions 数组的起始索引
}

pub struct KmerIndex {
    pub entries: Vec<KmerEntry>,
    pub positions: Vec<u32>, // 紧凑存储
}
```

#### 步骤 2: 修改索引构建逻辑

```rust
// reference/index.rs - build_wgbs_index
fn build_wgbs_index(&self, seq: &BinSeqSet) -> KmerIndex {
    // 1. 统计频率
    let mut counts = vec![(0u32, 0u32); self.max_kmer_num as usize];
    self.count_wgbs_kmers(seq, &mut counts);
    
    // 2. 计算总位置数
    let total_positions: usize = counts.iter()
        .map(|(f, r)| (f + r) as usize)
        .sum();
    
    // 3. 分配紧凑存储
    let mut entries = Vec::with_capacity(self.max_kmer_num as usize);
    let mut positions = Vec::with_capacity(total_positions);
    
    // 4. 构建 entries 和 positions
    let mut current_offset = 0u32;
    for (f, r) in counts {
        entries.push(KmerEntry {
            n: [f, r],
            start: current_offset,
        });
        current_offset += f + r;
    }
    
    KmerIndex { entries, positions }
}
```

#### 步骤 3: 修改查询逻辑

```rust
// reference/index.rs - get_wgbs_positions
pub fn get_wgbs_positions(&self, hash: u32, strand: bool) -> &[u32] {
    let strand_idx = if strand { 1 } else { 0 };
    let entry = &self.entries[hash as usize];
    let count = entry.n[strand_idx];
    let start = entry.start;
    &self.positions[start as usize..(start + count) as usize]
}
```

#### 步骤 4: 验证正确性

```bash
cargo test --package bsmap --lib reference::index
```

---

## P0-3: 消除热点路径边界检查

### 优化目标
- 在性能关键路径使用 unsafe 代码，消除不必要的安全检查
- 预期收益：**性能提升 5-10%**

### 关键位置

1. `alphabet.rs::make_seed` - 种子提取
2. `align/engine.rs` - 比对引擎热点循环

### 实施步骤

#### 步骤 1: 分析热点路径

```rust
// alphabet.rs - 当前实现
#[inline]
pub fn make_seed(words: &[u64], bit_pos: u32, seed_bits_lz: u32) -> u32 {
    // 所有访问都有边界检查
    if word_idx >= words.len() {
        return 0;
    }
    // ...
}
```

#### 步骤 2: 添加 unsafe 版本

```rust
// alphabet.rs
#[inline]
pub unsafe fn make_seed_unchecked(
    words: *const u64,
    words_len: usize,
    bit_pos: u32,
    seed_bits_lz: u32
) -> u32 {
    // 直接指针操作，无边界检查
    let word_idx = (bit_pos / 64) as isize;
    let bit_offset = (bit_pos % 64) as u32;
    
    let straddle = if bit_offset == 0 {
        *words.add(word_idx as usize)
    } else {
        (*words.add(word_idx as usize) << bit_offset) |
        (*words.add(word_idx as usize + 1) >> (64 - bit_offset))
    };
    
    xt3((straddle >> seed_bits_lz) as u32)
}
```

#### 步骤 3: 在索引查询时使用 unsafe

```rust
// reference/index.rs
pub fn query_index_unchecked(&self, seed: u32, strand: bool) -> &[u32] {
    // 仅在索引构建后确认安全时使用
    unsafe {
        let entry = self.entries.get_unchecked(seed as usize);
        let count = entry.n[strand as usize];
        let start = entry.start;
        std::slice::from_raw_parts(
            self.positions.as_ptr().add(start as usize),
            count as usize
        )
    }
}
```

---

## 实施时间线

| 阶段 | 任务 | 预计时间 | 优先级 |
|------|------|---------|--------|
| 1 | SIMD 优化核心哈希函数 | 2-4 小时 | **P0** |
| 2 | 整合 WGBS 索引存储 | 1-2 小时 | **P0** |
| 3 | 消除热点路径边界检查 | 1 小时 | P1 |
| 4 | 验证和测试 | 1 小时 | - |
| 5 | 重新基准测试 | 30 分钟 | - |

---

## 测试策略

### 单元测试
```bash
cargo test --package bsmap
```

### 集成测试
```bash
# 使用 Ex1 & Ex2 数据
cargo test --package bsmap --test integration
```

### 基准测试
```bash
# 重新运行 Ex1 & Ex2
./run_ex1_ex2.sh
```

### 性能对比指标

| 指标 | 优化前 | 目标 | 期望提升 |
|------|--------|------|---------|
| Ex1 运行时间 | 18.45s | 6-9s | 2-3x |
| Ex2 运行时间 | 15.15s | 5-7s | 2-3x |
| 内存使用 | 1.8 GB | 1.5 GB | 减少 15-20% |

---

## 风险和缓解

| 风险 | 影响 | 缓解策略 |
|------|------|---------|
| SIMD 实现错误 | 正确性 | 完整单元测试覆盖 |
| 内存布局变化 | 向后兼容 | 版本化索引格式 |
| unsafe 代码引入 | 安全性 | 最小化使用范围 |

---

## 成功标准

✅ **必须满足：**
1. 所有单元测试通过
2. Ex1 & Ex2 比对结果与优化前一致
3. SAM 一致性 ≥ 98%
4. 性能提升 ≥ 2x

✅ **期望满足：**
1. 内存使用减少 ≥ 15%
2. 性能提升 ≥ 3x

---

**计划制定时间：** 2026-05-17
**计划版本：** v1.0
