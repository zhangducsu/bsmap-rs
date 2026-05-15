# Phase 9 SIMD + mmap 优化设计文档

**日期**: 2026-05-15  
**范围**: Phase 9 第 1 项 (SIMD 批量读段编码) + 第 2 项 (mmap 参考序列)  
**不做**: Phase 9 第 3 项 (自适应线程数) + 第 4 项 (NUMA 感知)

---

## 1. 背景与目标

### 1.1 当前状态

| 组件 | 当前实现 | 问题 |
|------|---------|------|
| 读段编码 | `alphabet.rs` 标量逐碱基循环 | 每碱基一次查表+移位+或运算，无并行 |
| 参考序列存储 | `BinSeqCollection.refcat/crefcat: Vec<u64>` | 全量堆内存，人类基因组 ~1.5GB |
| mmap 依赖 | `memmap2 = "0.9"` 已声明 | 代码中零引用，完全未使用 |

### 1.2 优化目标

1. **SIMD 批量读段编码**: 用 AVX2 pshufb 指令一次处理 16 个碱基，预期 2-4x 加速
2. **mmap 参考序列**: 将 refcat/crefcat 映射到文件，按需分页，预期 RSS 降低 ~1.5GB

---

## 2. SIMD 批量读段编码设计

### 2.1 目标函数

需要 SIMD 优化的函数：
- `alphabet::pack_forward()` - 正向编码
- `alphabet::pack_revcomp()` - 反向互补编码
- `encode::build_mask()` - 有效碱基掩码构建

### 2.2 算法设计

#### 2.2.1 核心思想：pshufb 查表法

AVX2 `_mm256_shuffle_epi8` (pshufb) 可以在 256-bit 向量上并行执行 16 次 16 字节查表。

**查找表设计** (`PACK_TABLE[256]`):
```
对于每个 ASCII 字节，高 4 位和低 4 位分别存储编码：
  PACK_TABLE['A'] = 0x00 (A=0, A=0)
  PACK_TABLE['C'] = 0x11 (C=1, C=1)
  PACK_TABLE['G'] = 0x22 (G=2, G=2)
  PACK_TABLE['T'] = 0x33 (T=3, T=3)
  PACK_TABLE['N'] = 0x00 (默认 A)
  ...
```

**编码流程** (处理 32 个碱基 → 2 个 u64):
1. 加载 32 字节 ASCII 序列
2. `_mm256_shuffle_epi8` 查表得到 32 字节编码（每字节高4位+低4位各存一个碱基编码）
3. 分离高低 4 位，分别打包到两个 128-bit 向量
4. 用 `_mm256_slli_epi64` 和 `_mm256_or_si256` 合并为 2 个 u64
5. 处理剩余不足 32 碱基的尾部（标量回退）

#### 2.2.2 接口设计

```rust
// alphabet.rs

/// SIMD 优化的正向编码入口
#[cfg(target_arch = "x86_64")]
#[inline]
pub fn pack_forward_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    if is_x86_feature_detected!("avx2") {
        unsafe { pack_forward_avx2(seq, n_words) }
    } else {
        pack_forward(seq, n_words) // 标量回退
    }
}

/// AVX2 实现（内部 unsafe 函数）
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn pack_forward_avx2(seq: &[u8], n_words: usize) -> Vec<u64> {
    use std::arch::x86_64::*;
    // ... pshufb 实现
}

/// 非 x86_64 平台存根
#[cfg(not(target_arch = "x86_64"))]
#[inline]
pub fn pack_forward_simd(seq: &[u8], n_words: usize) -> Vec<u64> {
    pack_forward(seq, n_words)
}
```

#### 2.2.3 反向互补 SIMD

`pack_revcomp_simd` 需要额外步骤：
1. 加载 32 字节
2. `_mm256_shuffle_epi8` 用 `REV_PACK_TABLE` 查表（A→T, C→G, G→C, T→A）
3. `_mm256_permute2x128_si256` + `_mm256_shuffle_epi8` 反转字节序
4. 后续打包同正向

#### 2.2.4 掩码构建 SIMD

`build_mask_simd` 类似，但查 `REG_ALPHABET` 表（有效碱基=0b11, 无效=0b00）。

### 2.3 测试策略

1. **一致性测试**: SIMD 输出与标量输出逐位对比
2. **边界测试**: 长度 0, 1, 31, 32, 33, 63, 64, 65 等边界
3. **非法字符测试**: N, 小写, 其他 ASCII 字符
4. **平台回退测试**: 非 AVX2 平台确保标量路径正常工作

---

## 3. mmap 参考序列设计

### 3.1 存储抽象层

#### 3.1.1 BinSeqStorage Trait

```rust
// reference/storage.rs

/// 参考序列存储后端抽象
pub trait BinSeqStorage: Send + Sync {
    /// 以 u64 slice 形式访问
    fn as_slice(&self) -> &[u64];
    
    /// 获取长度（u64 word 数）
    fn len(&self) -> usize;
    
    /// 是否为空
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 堆内存存储（现有行为）
pub struct VecStorage {
    data: Vec<u64>,
}

impl BinSeqStorage for VecStorage {
    fn as_slice(&self) -> &[u64] {
        &self.data
    }
    
    fn len(&self) -> usize {
        self.data.len()
    }
}

/// mmap 存储（新增）
pub struct MmapStorage {
    mmap: memmap2::Mmap,
    len: usize,
}

impl BinSeqStorage for MmapStorage {
    fn as_slice(&self) -> &[u64] {
        // SAFETY: mmap 生命周期保证数据有效，且为只读
        unsafe {
            std::slice::from_raw_parts(
                self.mmap.as_ptr() as *const u64,
                self.len
            )
        }
    }
    
    fn len(&self) -> usize {
        self.len
    }
}
```

#### 3.1.2 BinSeqCollection 改造

```rust
// binseq.rs

pub struct BinSeqCollection {
    pub total_num: u32,
    pub sum_length: u64,
    // 改为 Box<dyn BinSeqStorage>
    pub refcat: Box<dyn BinSeqStorage>,
    pub crefcat: Box<dyn BinSeqStorage>,
    pub ref_anchor: Vec<u32>,
    pub blocks: Vec<Block>,
    pub seqs: Vec<BinarySeq>,
    pub chr_names: Vec<String>,
}
```

### 3.2 文件格式扩展

#### 3.2.1 新 .bsi 格式（版本 2）

```
┌─────────────────────────────────────────┐
│ Header (256 bytes)                      │
│   magic:       [u8; 8]   "BSMAPIDX"    │
│   version:     u32        2             │ ← 版本升级到 2
│   seed_size:   u32                      │
│   mode:        u32                      │
│   total_kmers: u32                      │
│   max_kmer_num:u32                      │
│   index_interval: u32                   │
│   max_kmer_ratio: f64                   │
│   num_refs:  u32                        │
│   ref_names_len: u32                    │
│   refcat_len: u64                       │ ← 新增：refcat word 数
│   crefcat_len: u64                      │ ← 新增：crefcat word 数
│   reserved:  [u8; 196]                  │
├─────────────────────────────────────────┤
│ Reference names (ref_names_len bytes)    │
│   each name: u16(len) + UTF-8 bytes     │
├─────────────────────────────────────────┤
│ Index data (bincode-serialized)          │
│   IndexData (KmerIndex)                 │
├─────────────────────────────────────────┤
│ refcat data (refcat_len × 8 bytes)      │ ← 新增数据段
│   raw u64 words, little-endian          │
├─────────────────────────────────────────┤
│ crefcat data (crefcat_len × 8 bytes)    │ ← 新增数据段
│   raw u64 words, little-endian          │
└─────────────────────────────────────────┘
```

#### 3.2.2 向后兼容

- 读取时检查 `version` 字段
- `version == 1`: 旧格式，refcat/crefcat 需要单独从 FASTA 重新构建
- `version == 2`: 新格式，可直接 mmap refcat/crefcat 数据段

### 3.3 API 设计

```rust
// index_io.rs

/// 保存索引（版本 2 格式，包含 refcat/crefcat）
pub fn save_index_v2(
    path: &Path,
    index: &KmerIndex,
    coll: &BinSeqCollection,  // 新增：需要 refcat/crefcat
    seed_size: u32,
    index_interval: u32,
    max_kmer_ratio: f64,
    ref_names: &[String],
    is_rrbs: bool,
) -> Result<()>;

/// 加载索引（自动检测版本，支持 mmap）
pub enum LoadMode {
    /// 全量加载到内存（Vec<u64>）
    Memory,
    /// mmap 参考序列（版本 2 格式）
    Mmap,
}

pub fn load_index_with_mode(
    path: &Path,
    mode: LoadMode,
) -> Result<(BinSeqCollection, KmerIndex, IndexMeta)>;
```

### 3.4 使用场景

**索引构建** (`bsmap index`):
```rust
// 1. 从 FASTA 构建 BinSeqCollection
let coll = BinSeqCollection::from_references(&refs);

// 2. 构建 k-mer 索引
let index = KmerIndex::build_wgbs(&coll, seed_size, index_interval, max_kmer_ratio);

// 3. 保存为版本 2 格式（包含 refcat/crefcat）
save_index_v2(&path, &index, &coll, ...)?;
```

**比对加载** (`bsmap align`):
```rust
// 自动检测版本，优先使用 mmap（版本 2）
let (coll, index, meta) = load_index_with_mode(&path, LoadMode::Mmap)?;
```

### 3.5 测试策略

1. **格式兼容性**: 版本 1 文件仍可加载（回退到 VecStorage）
2. **数据一致性**: mmap 加载后与原始数据逐字节对比
3. **多进程测试**: 多个进程同时 mmap 同一文件
4. **大文件测试**: 人类基因组级别文件 mmap 性能

---

## 4. 性能预期

| 优化项 | 当前 | 预期 | 提升 |
|--------|------|------|------|
| 读段编码 (150bp) | ~450 周期 | ~150 周期 | 3x |
| 人类基因组 RSS | ~1.5GB (refcat+crefcat) | ~100MB (按需) | 15x |
| 索引加载时间 | ~5s (bincode 反序列化) | ~0.1s (mmap) | 50x |

---

## 5. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| SIMD 边界处理错误 | 编码错误 | 完整边界测试 + SIMD/标量一致性验证 |
| mmap 文件损坏 | 崩溃 | 文件头 CRC 校验 + 优雅降级到内存加载 |
| 旧格式兼容性破坏 | 用户无法使用旧索引 | 保留版本 1 加载路径，自动检测格式版本 |
| trait 对象虚函数开销 | 性能下降 | `#[inline]` + 编译器优化，实际开销极小 |

---

## 6. 实现顺序

1. **SIMD 编码**: 先实现 `pack_forward_simd`，验证正确性后再扩展
2. **mmap 存储**: 先实现 `BinSeqStorage` trait + `VecStorage`，再添加 `MmapStorage`
3. **文件格式**: 先实现版本 2 保存，再实现 mmap 加载
4. **集成测试**: 端到端验证 `bsmap index` → `bsmap align` 流程
