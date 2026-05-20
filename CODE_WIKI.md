# BSMAP 项目 Code Wiki

## 目录
1. [项目概述](#项目概述)
2. [项目结构](#项目结构)
3. [核心模块](#核心模块)
4. [关键类与函数](#关键类与函数)
5. [依赖关系](#依赖关系)
6. [测试数据](#测试数据)
7. [测试进度](#测试进度)
8. [项目运行方式](#项目运行方式)
9. [已知问题与解决方案](#已知问题与解决方案)

---

## 项目概述

### BSMAP 简介
**BSMAP** (Bisulfite Sequence MAPping) 是用于亚硫酸氢盐测序 (BS-seq) 的短序列比对工具，专门用于 DNA 甲基化研究。

### 核心功能
- 支持全基因组亚硫酸氢盐测序 (WGBS)
- 支持简化代表性亚硫酸氢盐测序 (RRBS)
- 支持单端 (SE) 和双端 (PE) 读段比对
- 支持 gzip 压缩的 FASTA/FASTQ 文件
- SAM/BAM 输出格式
- 多线程并行处理

### 技术栈
- **原始版本**: C++ + Python 2.x (BSMAP 2.90)
- **新版本**: Rust (bsmap-rs) - 完全重写

### 对比优势
| 特性 | 原始 C++/Python 版本 | Rust 版本 |
|------|---------------------|----------|
| 原始人类基因组 methratio 内存需求 | ~26 GB | < 1 GB |
| 双端比对稳定性 | 存在 buffer overflow 崩溃 | 稳定运行 |
| 依赖 | Python 2 + samtools | 纯 Rust 实现 |

---

## 项目结构

### 总体目录结构
```
BSMAP/
├── bsmap-original/           # 原始 C++ 版本 (BSMAP 2.90)
│   ├── bsmap-2.90/
│   │   ├── bsmap            # 主程序
│   │   ├── methratio.py     # 甲基化比例计算
│   │   ├── methdiff.py      # 差异甲基化分析
│   │   ├── bsp2sam.py       # BSP 转 SAM 转换
│   │   ├── samtools/        # 内置 samtools 库
│   │   └── gzstream/        # gzip 流处理库
│   └── README.md
├── bsmap-rs/                # Rust 重写版本 (当前开发重点)
│   ├── bsmap/               # 核心比对器
│   ├── methratio/           # 甲基化比例计算 (Rust 版本)
│   ├── bsp2sam/             # BSP 转 SAM (Rust 版本)
│   ├── benchmark/           # 性能基准测试
│   ├── tests/               # 测试数据与报告
│   ├── tools/               # 外部工具
│   ├── docs/                # 文档
│   ├── Cargo.toml           # 工作区配置
│   └── CLAUDE.md
└── .trae/
    └── documents/
```

### bsmap-rs 详细结构
```
bsmap-rs/
├── bsmap/
│   └── src/
│       ├── main.rs              # 主程序入口
│       ├── lib.rs               # 库入口
│       ├── cli.rs               # 命令行参数解析
│       ├── param.rs             # 参数与数据结构
│       ├── alphabet.rs          # DNA 编码与位操作
│       ├── utils.rs             # 工具函数
│       ├── align/               # 比对引擎
│       │   ├── mod.rs
│       │   ├── engine.rs        # 单端比对引擎
│       │   ├── seed.rs          # 种子提取
│       │   ├── mismatch.rs      # 错配计数 (含 AVX2 SIMD)
│       │   ├── gap.rs           # 缺口比对
│       │   ├── extend.rs        # 种子扩展
│       │   └── output.rs        # SAM/BAM/BSP 输出
│       ├── pairs/               # 双端处理
│       │   ├── mod.rs
│       │   ├── pair.rs          # 配对逻辑
│       │   └── output.rs        # 配对输出
│       ├── reads/               # 读段处理
│       │   ├── mod.rs
│       │   ├── fastq.rs         # FASTQ 解析
│       │   ├── bam.rs           # SAM/BAM 解析
│       │   ├── batch.rs         # 批量处理
│       │   └── encode.rs        # 读段编码
│       └── reference/           # 参考序列处理
│           ├── mod.rs
│           ├── fasta.rs         # FASTA 加载
│           ├── binseq.rs        # 二进制序列表示
│           ├── index.rs         # k-mer 索引构建
│           ├── index_io.rs      # 索引读写 (序列化)
│           ├── rrbs.rs          # RRBS 酶切位点处理
│           └── storage.rs
├── methratio/
│   └── src/
│       ├── main.rs              # CLI 入口
│       ├── lib.rs               # 库入口
│       ├── input.rs             # 输入解析 (SAM/BAM/BSP)
│       ├── counter.rs           # 甲基化计数核心
│       └── output.rs            # 输出格式化
├── bsp2sam/
│   └── src/
│       └── main.rs              # BSP → SAM 转换
├── tests/
│   ├── data/                    # 测试数据集
│   ├── reports/                 # 测试报告
│   └── README.md
└── benchmark/
    ├── data/                    # 基准测试数据
    ├── logs/                    # 测试日志
    ├── report/                  # 基准测试报告
    └── results/                 # 比对结果
```

---

## 核心模块

### 1. 比对引擎 (align/)

**职责**: 核心比对算法实现

主要子模块:
- `engine.rs`: 单端比对主引擎
- `seed.rs`: k-mer 种子提取与重排序
- `mismatch.rs`: 位并行错配计数 (含 AVX2 SIMD 优化)
- `gap.rs`: 缺口 (gap) 比对
- `extend.rs`: 种子扩展与命中收集
- `output.rs`: SAM/BAM/BSP 输出格式化

### 2. 双端处理 (pairs/)

**职责**: 双端读段配对逻辑

主要子模块:
- `pair.rs`: 配对算法、insert size 过滤、双指针优化
- `output.rs`: 配对 SAM/BAM 输出

### 3. 读段处理 (reads/)

**职责**: 读段输入解析与预处理

主要子模块:
- `fastq.rs`: FASTQ/FASTA 解析 (needletail)
- `bam.rs`: SAM/BAM 解析 (noodles)
- `batch.rs`: 批量处理、质量修剪、适配器修剪、N 碱基过滤
- `encode.rs`: 读段二进制编码

### 4. 参考序列 (reference/)

**职责**: 参考序列加载与索引

主要子模块:
- `fasta.rs`: FASTA 参考序列加载
- `binseq.rs`: 2-bit 编码、Watson/Crick 链转换
- `index.rs`: WGBS/RRBS k-mer 索引构建 (3-pass 算法)
- `index_io.rs`: 索引持久化 (bincode + serde)
- `rrbs.rs`: RRBS 酶切位点识别 (MspI 等)

### 5. 甲基化分析 (methratio/)

**职责**: 从比对结果计算甲基化比例

主要特点:
- 稀疏 HashMap 计数 (替代原 Python 的密集数组)
- 人类基因组内存从 26 GB 降至 < 1 GB
- 按染色体并行处理
- 支持 CpG 合并、C→T SNP 校正

### 6. BSP 转换 (bsp2sam/)

**职责**: BSP 格式 → SAM 格式转换

主要特点:
- 保留配对信息
- 支持管道输入/输出

---

## 关键类与函数

### BSMAP 核心数据结构

#### `AlignConfig` (param.rs)
比对配置，包含所有命令行参数

```rust
struct AlignConfig {
    seed_size: u32,           // 种子大小 (default: 16)
    max_mismatch: f64,        // 最大错配率/数
    max_gap: u32,             // 最大缺口大小
    num_threads: u32,         // 线程数
    paired_end: bool,         // 双端模式
    rrbs_mode: bool,          // RRBS 模式
    // ... 更多参数
}
```

#### `GHit` (param.rs)
基因组命中记录 (8 bytes，紧凑存储)

#### `ReadInf` (reads/)
读段信息

```rust
struct ReadInf {
    name: String,    // 读段名称
    seq: Vec<u8>,    // 序列
    qual: Vec<u8>,   // 质量值
}
```

#### `BinSeqCollection` (reference/)
二进制参考序列集合 (2-bit 编码)

### 核心算法

#### k-mer 索引构建 (3-pass 算法)
1. **Pass 1**: 统计 k-mer 出现频率
2. **Pass 2**: 分配内存位置
3. **Pass 3**: 填充位置数组

#### 位并行错配计数
- 使用位操作和 popcount 快速计算错配数
- 可选 AVX2 SIMD 优化

#### 配对算法 (双端)
- 按 read chain 分离 hits
- insert size 过滤
- 双指针法高效配对

---

## 依赖关系

### Rust Workspace 依赖 (Cargo.toml)
| 依赖 | 用途 |
|------|------|
| clap | 命令行参数解析 (derive) |
| anyhow + thiserror | 错误处理 |
| rayon | 并行计算 (work-stealing) |
| crossbeam-channel | 高性能并发通道 |
| flate2 | gzip 压缩/解压 |
| needletail | FASTA/FASTQ 零拷贝解析 |
| noodles | 纯 Rust SAM/BAM 处理 (替代 samtools) |
| log + env_logger | 日志记录 |
| indicatif | 进度条 |
| serde + bincode | 索引序列化 |
| memmap2 | 内存映射 (待优化) |

### 原始 C++ 版本依赖
- pthread (多线程)
- samtools (内置 0.1.18)
- gzstream (gzip 流处理)
- Python 2.x (脚本)

---

## 测试数据

### 测试数据集概览

| 数据集 | 用途 | 参考基因组 | 读段类型 |
|--------|------|-----------|---------|
| ex1_small | 基础功能/快速验证 | 2 条序列 (3,158 bp) | SE 32bp |
| lambda_wgbs | 双端 WGBS 功能测试 | Lambda (48,502 bp) | PE 150bp |
| lambda_wgbs_sim | Rust vs C++ 验证 | Lambda (48,502 bp) | PE 150bp |
| lambda_rrbs | RRBS 模式测试 | Lambda (48,502 bp) | PE 150bp |
| rrbs_random_v2 | RRBS 随机基因组 | 随机 (48,500 bp) | PE 150bp |
| realistic | 真实基因组片段测试 | 真实片段 | SE 32bp |

### Lambda WGBS Sim 数据集 (主要验证集)
- 参考: Lambda phage NC_001416.1 (48,502 bp)
- Read pairs: 4,850
- 生成工具: BSBolt
- 验证结果: **Rust vs C++ SAM 输出 0 差异** (所有 13 个字段)

### 基准测试数据集 (benchmark/data/)
- chr22_tail_1M.fa (1,000,000 bp)
- random_genome.fa (1,000,000 bp)
- WGBS 与 RRBS 多个覆盖度测试集 (10x, 20x)

---

## 测试进度

### 已完成验证 (Phase 0-6 + 8) ✅

| 阶段 | 内容 | 状态 |
|------|------|------|
| Phase 0 | Bug 修复与基础完善 | ✅ |
| Phase 1.5 | 索引优化与持久化 | ✅ |
| Phase 2 | 读段加载模块 | ✅ |
| Phase 3 | 比对引擎 | ✅ |
| Phase 4 | 双端处理 | ✅ |
| Phase 5 | 主程序集成与管道 | ✅ |
| Phase 6 | methratio 子 crate | ✅ |
| Phase 8 | bsp2sam 子 crate | ✅ |

### 验证结果总结

#### 单端比对验证
- Lambda SE150: 9,700 reads, **100% 匹配** (0 diff) ✅

#### 双端比对验证
- Lambda WGBS PE: 9,700 pairs, **100% 匹配** (仅 1 对多重命中随机选择差异) ✅

#### SAM/BAM 输出
- 与原版完全兼容，samtools 验证通过 ✅

#### methratio 验证
- 24,170 行输出，**0 diff** vs Python 原版 ✅
- 人类基因组内存从 ~26 GB 降至 < 1 GB ✅

### 待完成 (Phase 7, 9) ⬜

| 阶段 | 内容 |
|------|------|
| Phase 7 | methdiff 子 crate |
| Phase 9 | 高级优化 (SIMD, mmap, NUMA) |

### 已知问题 (待修复)

| 问题 | 严重程度 | 说明 |
|------|---------|------|
| RRBS 模式 OOM | 高 | seed_size=12 时索引构建需 ~3.7 GB 内存 |
| 原始 C++ PE buffer overflow | 高 | 原版 BSMAP 2.90 在当前环境下 PE 模式崩溃 |
| N 碱基 mismatch 计数不一致 | 低 | 行为差异，通常不影响结果 |

---

## 项目运行方式

### bsmap-rs 构建

```bash
cd bsmap-rs
cargo build --release
```

### 核心命令

#### 1. 索引构建
```bash
# WGBS 模式
bsmap index -d ref.fa

# RRBS 模式
bsmap index -d ref.fa -s 12 -D C-CGG
```

#### 2. 读段比对
```bash
# 单端比对
bsmap align -a reads.fq.gz -d ref.fa -o out.sam

# 双端比对
bsmap align -a R1.fq.gz -b R2.fq.gz -d ref.fa -o out.sam

# BAM 输出
bsmap align -a reads.fq.gz -d ref.fa -o out.bam

# 向后兼容 (无子命令 = align)
bsmap -a reads.fq.gz -d ref.fa -o out.sam
```

#### 3. 甲基化比例计算
```bash
# 从 SAM/BAM 计算
methratio -d ref.fa -o meth.txt in.sam

# 管道模式
bsmap align -a reads.fq.gz -d ref.fa | methratio -d ref.fa -o meth.txt -
```

#### 4. BSP → SAM 转换
```bash
bsp2sam -d ref.fa -o out.sam in.bsp

# 管道模式
bsmap align -a reads.fq.gz -d ref.fa -o out.bsp | bsp2sam -d ref.fa
```

### 常用参数

| 参数 | 说明 | 默认值 |
|------|------|--------|
| -a | 读段文件 1 | (required) |
| -b | 读段文件 2 (双端) | - |
| -d | 参考基因组 FASTA | (required) |
| -o | 输出文件 | stdout |
| -s | 种子大小 | 16 |
| -v | 最大错配 (率/数) | 0.08 |
| -p | 线程数 | CPU 核数 |
| -n | 链匹配模式 (0/1) | 0 |
| -D | RRBS 酶切位点 | - |
| -r | 重复 hit 报告模式 | 1 |

### 与原版 BSMAP 兼容性
bsmap-rs 保持与原版 BSMAP 2.90 命令行参数完全兼容，可直接替换使用。

---

## 已知问题与解决方案

### 1. bsmap-rs RRBS 模式 OOM
**问题**: RRBS 模式下索引构建内存过高
**临时方案**: 使用 WGBS 模式 + 后续过滤
**长期方案**: 优化索引构建内存 (Phase 9)

### 2. 原始 C++ 双端 buffer overflow
**问题**: 原版 BSMAP 2.90 在当前环境下 PE 模式崩溃
**解决方案**: 使用 bsmap-rs (稳定)

### 3. N 碱基 mismatch 计数差异
**问题**: Rust 与 C++ 对 N 碱基的 mismatch 计数行为不一致
**影响**: 通常不影响结果 (含 N 读段会被 max_ns 过滤)
**优先级**: 低

---

## 性能基准 (2026-05-16)

### WGBS 单端 75bp 10x
| 指标 | BSMAP C++ | bsmap-rs |
|------|-----------|----------|
| 比对率 | 49.6% | 49.6% |
| 耗时 | 3.36s | 8.90s |
| 峰值内存 | 849 MB | 1,814 MB |

### bsmap-rs 优势
- **双端稳定性**: C++ 崩溃，Rust 正常运行
- **methratio 内存**: 26 GB → < 1 GB
- **纯 Rust 实现**: 无 Python 2/samtools 依赖

---

## 参考文献
- 原版 BSMAP 论文: Yuanxin Xi and Wei Li, "BSMAP: whole genome bisulfite sequence MAPping program" (2009)
- BSMAP-rs 重构计划: [bsmap-rs-refactor-plan.md](./bsmap-rs-refactor-plan.md)

---

*文档生成日期: 2026-05-16*
