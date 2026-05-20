# P3优化：比对引擎优化技术方案

**文档版本**: v1.0  
**创建日期**: 2026-05-18  
**状态**: 待实现  
**负责人**: SOLO AI Assistant

---

## 1. 需求分析

### 1.1 需求概述

根据业务/产品描述，P3优化需要实现以下三个核心目标：

| 序号 | 需求点 | 描述 | 优先级 |
|------|--------|------|--------|
| 1 | SIMD化Smith-Waterman | 对局部比对算法进行SIMD优化 | 高 |
| 2 | 多重比对策略优化 | 参考原版BSMAP处理逻辑，优化多重比对处理 | 高 |
| 3 | 减少内存分配 | 优化内存使用模式，减少不必要的分配 | 中 |

### 1.2 现有代码分析

#### 1.2.1 当前比对引擎架构

```
align/
├── engine.rs      # 比对引擎主逻辑（SingleAlign/PairAlign）
├── extend.rs      # 种子扩展和命中收集（SnpAlign）
├── gap.rs         # Gap比对算法
├── mismatch.rs    # Mismatch计数（已部分SIMD化）
├── seed.rs        # 种子提取和重排序
└── output.rs      # 输出格式化
```

#### 1.2.2 当前性能瓶颈

| 模块 | 瓶颈描述 | 影响 |
|------|---------|------|
| `extend.rs` | `snp_align_for_chain` 中大量小Vec分配 | 内存分配开销 |
| `gap.rs` | `try_all_gaps` 三重循环 + 多次mismatch_pattern调用 | CPU密集 |
| `extend.rs` | `dedup_hits` 使用HashSet去重 | 内存+CPU开销 |
| `extend.rs` | `count_unique_hits` 每次都创建HashSet | 重复分配 |

---

## 2. 技术方案

### 2.1 方案概述

```
┌─────────────────────────────────────────────────────────────┐
│                      P3优化架构                            │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: SIMD加速层                                       │
│    ├── AVX2/AVX512 Smith-Waterman优化                      │
│    └── 批量Mismatch计数优化                                │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: 算法优化层                                       │
│    ├── 多重比对策略优化（参考原版BSMAP）                      │
│    ├── 命中去重优化（排序+去重代替HashSet）                   │
│    └── 提前终止策略优化                                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: 内存优化层                                       │
│    ├── 对象池/内存复用                                      │
│    ├── 预分配缓冲区                                        │
│    └── 栈上分配小对象                                      │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 SIMD化Smith-Waterman局部比对

#### 2.2.1 算法背景

BSMAP中的局部比对主要用于：
1. Gap比对中的mismatch pattern检测
2. 种子扩展后的精细比对

当前实现：
- `mismatch_pattern_0()` / `mismatch_pattern_1()` 使用标量遍历
- 每次调用生成新的Vec<u32>存储位置

#### 2.2.2 SIMD优化方案

**优化目标**: 使用AVX2指令批量处理mismatch检测

**核心优化**:

```rust
/// AVX2优化的mismatch pattern检测
#[target_feature(enable = "avx2")]
unsafe fn mismatch_pattern_avx2(
    query: &[u64],
    ref_seq: &[u64],
    offset: u32,
    map_readlen: u32,
    nt3: bool,
    positions: &mut Vec<u32>,  // 预分配缓冲区
    pos_offset: u32,           // 位置偏移
) -> usize {
    // 使用AVX2同时处理4个u64（256 bits）
    // 批量检测mismatch位置
    // 直接写入预分配的positions缓冲区
}
```

**优化收益**: 预计加速2-4x

### 2.3 多重比对处理策略优化

#### 2.3.1 原版BSMAP策略分析

根据C++版本分析，原版策略包括：

| 策略 | 描述 |
|------|------|
| **Early Stop** | 找到唯一比对后提前终止 |
| **Seed优先级** | 根据seed质量排序，优先处理高质量seed |
| **Hit去重** | 按位置去重，保留最佳mismatch |
| **Max Hits限制** | 达到阈值后停止处理 |

#### 2.3.2 优化方案

**优化1: 提前终止策略**

```rust
/// 判断是否应提前终止（参考原版BSMAP逻辑）
fn should_stop_early(
    seg_idx: usize, 
    hits: &[ExtHit], 
    snp_thres: u32,
    max_hits: usize,
) -> bool {
    // 如果已找到足够好的唯一比对，提前终止
    if hits.len() > 0 && is_unique_hit(hits) && seg_idx > 1 {
        return true;
    }
    // 如果达到最大命中数限制
    if hits.len() >= max_hits {
        return true;
    }
    false
}
```

**优化2: 命中去重优化**

```rust
/// 优化的命中去重（排序+去重，O(n log n)）
fn dedup_hits_fast(hits: &mut Vec<ExtHit>) {
    if hits.len() <= 1 {
        return;
    }
    // 按位置排序
    hits.sort_unstable_by_key(|h| (h.chr, h.loc, h.strand));
    // 保留mismatch数最小的hit
    let mut last_pos = (u32::MAX, u32::MAX, u8::MAX);
    hits.retain(|h| {
        let pos = (h.chr, h.loc, h.strand);
        if pos == last_pos {
            false  // 重复位置，保留第一个（mismatch最小）
        } else {
            last_pos = pos;
            true
        }
    });
}
```

### 2.4 减少不必要的内存分配

#### 2.4.1 对象池模式

```rust
/// 命中对象池
struct HitPool {
    hits: Vec<ExtHit>,
    pos: usize,
}

impl HitPool {
    fn with_capacity(cap: usize) -> Self {
        Self {
            hits: Vec::with_capacity(cap),
            pos: 0,
        }
    }
    
    fn get(&mut self) -> &mut ExtHit {
        if self.pos >= self.hits.len() {
            self.hits.push(ExtHit::default());
        }
        let idx = self.pos;
        self.pos += 1;
        &mut self.hits[idx]
    }
    
    fn reset(&mut self) {
        self.pos = 0;
    }
    
    fn len(&self) -> usize {
        self.pos
    }
    
    fn iter(&self) -> impl Iterator<Item = &ExtHit> {
        self.hits[..self.pos].iter()
    }
}
```

#### 2.4.2 预分配缓冲区

在`extend.rs`中使用预分配：

```rust
pub fn snp_align_for_chain_optimized(
    encoded: &EncodedRead,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    segments: &[SeedSegment],
    read_chain: u8,
    snp_thres: u32,
    gap_size: u32,
    nt3: bool,
    _is_rrbs: bool,
    hits_buffer: &mut Vec<ExtHit>,  // 预分配缓冲区
) {
    hits_buffer.clear();
    // 使用hits_buffer而不是每次创建新Vec
}
```

---

## 3. 实施计划

### 3.1 任务分解

| 序号 | 任务 | 子任务 | 复杂度 | 预估时间 |
|------|------|--------|--------|----------|
| 3.1 | SIMD化mismatch pattern | AVX2实现 | 高 | 8h |
| 3.2 | 多重比对策略优化 | Early stop实现 | 中 | 4h |
| 3.3 | 多重比对策略优化 | 去重算法优化 | 中 | 4h |
| 3.4 | 内存优化 | HitPool实现 | 低 | 2h |
| 3.5 | 内存优化 | 预分配缓冲区集成 | 中 | 4h |
| 3.6 | 测试与验证 | 单元测试 | 中 | 4h |
| 3.7 | 测试与验证 | 性能基准测试 | 中 | 4h |

### 3.2 实施顺序

```
阶段1: 内存优化（低风险，快速见效）
    └─ 3.4 → 3.5

阶段2: 算法优化（中等风险）
    └─ 3.2 → 3.3

阶段3: SIMD优化（高风险，高收益）
    └─ 3.1

阶段4: 测试验证
    └─ 3.6 → 3.7
```

---

## 4. 代码修改清单

### 4.1 修改文件

| 文件 | 修改类型 | 说明 |
|------|---------|------|
| `align/extend.rs` | 修改 | 优化snp_align_for_chain，使用预分配缓冲区 |
| `align/extend.rs` | 修改 | 优化dedup_hits和count_unique_hits |
| `align/mismatch.rs` | 修改 | 添加AVX2优化的mismatch_pattern |
| `align/gap.rs` | 修改 | 使用优化的mismatch_pattern |
| `align/engine.rs` | 修改 | 集成对象池和预分配 |

### 4.2 新增文件

| 文件 | 说明 |
|------|------|
| `align/pool.rs` | 对象池实现 |
| `align/simd.rs` | SIMD工具函数 |

---

## 5. 性能预期

| 优化项 | 预期收益 | 验证指标 |
|--------|---------|----------|
| 对象池 | 减少50%内存分配 | 分配次数统计 |
| 预分配缓冲区 | 减少30%内存分配 | 分配次数统计 |
| SIMD mismatch | 2-4x加速 | 基准测试时间 |
| 去重优化 | 1.5-2x加速 | 去重耗时 |
| 提前终止 | 视数据而定 | 平均处理segment数 |

---

## 6. 测试计划

### 6.1 单元测试

| 测试项 | 测试内容 |
|--------|---------|
| 对象池 | 重复使用、reset功能 |
| SIMD一致性 | SIMD与标量结果一致 |
| 去重正确性 | 去重后无重复命中 |
| 提前终止 | 正确识别唯一比对 |

### 6.2 基准测试

| 测试用例 | 配置 |
|----------|------|
| Ex1 SE 75bp | 4线程 |
| Ex2 PE 150bp | 4线程 |
| 全基因组模拟 | 4线程 |

---

## 7. 风险评估

| 风险 | 等级 | 缓解措施 |
|------|------|----------|
| SIMD兼容性 | 中 | 检测CPU特性，回退到标量 |
| 正确性风险 | 中 | 严格的单元测试验证 |
| 性能回归 | 低 | 基准测试监控 |

---

**文档版本**: v1.0  
**创建日期**: 2026-05-18  
**状态**: 待实现
