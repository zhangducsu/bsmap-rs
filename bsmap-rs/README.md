# BSMAP-rs

**Bisulfite Sequence MAPping** — 高性能亚硫酸氢盐测序（BS-seq）比对器，Rust 实现。

BSMAP-rs 是 [BSMAP 2.90](https://github.com/genome-vendor/bsmap) 的完全 Rust 重写版，在保持 100% 输出兼容性的前提下，大幅降低内存占用并提升多线程性能。

---

## 功能特性

- **双模式支持**: WGBS（全基因组亚硫酸氢盐测序）和 RRBS（简化代表性亚硫酸氢盐测序）
- **输入格式**: FASTA / FASTQ / SAM / BAM，支持 gzip 压缩
- **输出格式**: SAM / BAM（原生，无需 samtools）/ BSP
- **单端（SE）与双端（PE）** 比对
- **自动索引缓存**: 构建的 `.bsi` 索引自动缓存，下次秒级 mmap 加载
- **向后兼容 CLI**: 参数与原版 C++ BSMAP 完全兼容
- **零 unsafe 代码**: 纯 Rust，无 `unsafe` 代码块

## 工具链

| 命令 | 功能 | 状态 |
|------|------|------|
| `bsmap` | 核心比对器：索引构建 + 读段比对 | ✅ 完成 |
| `methratio` | 甲基化比例计算 | ✅ 完成 |
| `bsp2sam` | BSP 格式转 SAM | ✅ 完成 |
| `methdiff` | 差异甲基化分析 | 🔄 规划中 |

## 性能

基于 chr22_tail_1M 参考基因组（1 Mbp）的基准测试结果（详见 `benchmark/` 目录）：

| 测试场景 | Rust (P11-18~19) | C++ BSMAP 2.90 | 提升 |
|----------|-----------------|-----------------|------|
| SE p=1 耗时 | 1.39s | 1.26s | — |
| SE p=4 耗时 | **0.67s** | 1.18s | **快 43%** |
| SE 峰值内存 | **524 MB** | 852 MB | **低 38%** |
| PE p=4 耗时 | **0.84s** | 崩溃 | 可用 |
| PE 峰值内存 | **~540 MB** | ~850 MB | **低 36%** |

> SE 测试基于 ex1_se75_10x（133k reads, 75bp），PE 测试基于 ex2_pe150_10x（13k pairs, 150bp）。
> C++ BSMAP 在 PE 模式下对所有测试数据集（ex2/ex4/ex6）均 buffer overflow 崩溃。
> 详细报告见 `benchmark/P11_report_ex3_ex4_ex6.md`。

## 快速开始

### 编译

```bash
git clone <repo-url>
cd bsmap-rs
cargo build --release
```

编译产物位于 `target/release/bsmap`（以及 `methratio`、`bsp2sam`）。

### 构建索引

```bash
# WGBS 模式（seed_size=16）
bsmap index -d genome.fa

# RRBS 模式（seed_size=12，MspI 酶切）
bsmap index -d genome.fa -s 12 -D C-CGG
```

索引保存为 `genome.fa.bsi`，下次比对自动 mmap 加载。

### 单端比对

```bash
# 显式子命令（推荐）
bsmap align -a reads.fq.gz -d genome.fa -o out.sam -p 4

# 向后兼容模式（等价于上述）
bsmap -a reads.fq.gz -d genome.fa -o out.sam -p 4
```

### 双端比对

```bash
bsmap align -a reads_1.fq.gz -b reads_2.fq.gz -d genome.fa -o out.sam -p 4
```

### BAM 输出

```bash
bsmap align -a reads.fq.gz -d genome.fa -o out.bam -p 4
```

输出排序 BAM，原生写入，无需调用 `samtools`。

### 甲基化比例计算

```bash
methratio -i alignments.sam -o methratio.txt -d genome.fa
```

### BSP 转 SAM

```bash
bsp2sam -i input.bsp -o output.sam
```

## 命令行参考

### `bsmap index` — 构建参考序列索引

| 参数 | 说明 |
|------|------|
| `-d FILE` | 参考基因组 FASTA（必须） |
| `-s INT` | 种子长度 10-16（WGBS 默认 16，RRBS 默认 12） |
| `-I INT` | 索引间隔 1-16（默认 4） |
| `-k FLOAT` | 高频率 k-mer 过滤阈值（默认 5e-7） |
| `-D SITE` | RRBS 酶切位点，'-' 标记断点（如 `C-CGG` 表示 MspI，可重复） |
| `-m INT` | 最小插入片段长度（RRBS，默认 28） |
| `-x INT` | 最大插入片段长度（RRBS，默认 1000） |

### `bsmap align` — 比对读段

| 参数 | 说明 |
|------|------|
| `-a FILE` | **读段文件**（FASTA/FASTQ/BAM，支持 gzip） |
| `-b FILE` | 双端比对的第二条读段文件 |
| `-d FILE` | **参考基因组 FASTA** |
| `-o FILE` | 输出文件（.sam / .bam / 其他后缀则输出 BSP 格式） |
| `-p INT` | 线程数（默认使用全部 CPU 核数） |
| `-s INT` | 种子长度（默认 16） |
| `-v FLOAT` | 最大错配率（默认 0.08） |
| `-I INT` | 索引间隔（默认 4） |
| `-g INT` | 最大连续 gap 数 0-3（默认 0） |
| `-w INT` | 最大等优 hit 报告数（默认 100） |
| `-q INT` | 3' 端质量阈值修剪（默认 0=关闭） |
| `-z INT` | 碱基质量偏移（默认 33） |
| `-f INT` | 过滤含 N 过多的读段（默认 5） |
| `-A SEQ` | 3' 端接头序列（可重复指定多个） |
| `-n INT` | 比对链模式：0=二链法（Lister 方案），1=四链法（Cokus 方案） |
| `-r INT` | 重复 hit 报告：0=仅唯一，1=随机选一（默认），2=全部 |
| `-L INT` | 只比对每个读段的前 N 个碱基 |
| `-u` | 输出未比对读段 |
| `-H` | 省略 SAM 头 |
| `-R` | 在 SAM XR:Z 字段包含参考序列 |
| `--nt3` | 使用 3-核苷酸映射 |
| `-D SITE` | RRBS 酶切位点 |
| `-M NT` | 碱基转换类型（默认 `TC`） |
| `-B INT` | 从第 N 条读段开始比对（默认 1） |
| `-E INT` | 比对到第 N 条读段为止 |
| `-m INT` | 最小插入片段长度（PE，默认 28） |
| `-x INT` | 最大插入片段长度（PE，默认 1000） |
| `-S INT` | 随机种子（0=系统时钟） |

## 项目结构

```
bsmap-rs/
├── bsmap/                  # 核心比对器
│   └── src/
│       ├── main.rs         # 主入口（run_index / run_align / run_paired）
│       ├── cli.rs          # CLI 参数解析（clap derive，支持子命令和向后兼容）
│       ├── lib.rs          # 库入口
│       ├── param.rs        # 对齐配置、统计、常量
│       ├── alphabet.rs     # DNA 位运算：3-base hash、C→T 容差掩码、SWAR popcount
│       ├── utils.rs        # 计时器等工具
│       ├── reference/      # FASTA 加载 → 2-bit 编码 → k-mer 索引 → .bsi 持久化
│       ├── reads/          # FASTQ/BAM 解析、质量修剪、接头裁剪、批处理编码
│       ├── align/          # 单端比对：种子查找 → 错配计数 → gap 对齐 → SAM/BSP 输出
│       └── pairs/          # 双端比对：链分离 → 插入片段过滤 → 双指针配对 → 输出
├── methratio/              # 甲基化比例计算器
├── bsp2sam/                # BSP 格式转 SAM 格式
├── benchmark/              # 性能基准测试报告
├── tests/                  # 集成测试数据
│   ├── data/               # Lambda / ex1 等各种测试数据集
│   └── reports/            # C++ vs Rust 对比报告
├── docs/                   # 优化计划与设计方案
└── tools/                  # 外部工具（BSBolt、sherman 等）
```

## 数据流

```
FASTA ref → BinSeqCollection（2-bit Watson + Crick 编码）→ KmerIndex（3-pass，.bsi 缓存）
                                                                        ↓
FASTQ reads → process_batch（trim/filter）→ encode_read → SingleAlign / PairAlign
    → seed lookup in KmerIndex → SWAR popcount mismatch → gap check → format_sam / format_bsp
```

全量数据载入内存后使用 `rayon` 线程池并行计算，`crossbeam-channel` 管道连接 I/O → 比对 → 输出各阶段。原子计数器实现无锁比对统计。

## 核心位运算原语

这些操作与 C++ 完全一致，变更会影响比对正确性：

| 原语 | 作用 |
|------|------|
| `xt3()` / `xt3_64()` | 3 字母种子哈希，C 和 T 映射到同桶 |
| `xc32()` / `xc64()` | C→T 容差掩码 |
| `xm64()` | SWAR popcount，一个 u64 内数 0-32 错配 |
| `u64::leading_zeros()` | 对应 C++ `__builtin_clzll` |
| `u64::trailing_zeros()` | 对应 C++ `__builtin_ctzll` |

## .bsi 索引格式

自定义小端序二进制格式（v2）：

- **256 字节头**: 魔数 `BSMAPIDX`、版本号、seed_size、模式（0=WGBS / 1=RRBS）
- **参考序列名**: u16 长度前缀的 UTF-8 字符串列表
- **索引数据**: bincode 序列化的 `KmerIndex`

索引文件与 FASTA 同路径（如 `genome.fa.bsi`），自动检测并 mmap 加载。

## 与 C++ BSMAP 2.90 对比

| 方面 | BSMAP-rs | C++ BSMAP 2.90 |
|------|----------|----------------|
| 语言 | Rust | C++ |
| 安全性 | 零 unsafe | 存在缓冲区溢出（PE 模式崩溃） |
| PE 模式 | ✅ 稳定 | ❌ 所有数据集上 buffer overflow |
| 内存占用 | 低 ~40% | 基准 |
| SE 输出兼容性 | ~97.4% 一致（2.6% 差异为 alternative alignment） | 基准 |
| BAM 输出 | 原生（noodles），无需外部工具 | 需 samtools 转换 |
| CLI 兼容性 | ✅ 100% 向后兼容 | 基准 |

> SE 输出约 2.6% 行存在位置选择差异（alternative alignment 不同），非回归。

## 开发

### 运行测试

```bash
cd bsmap-rs
cargo test -p bsmap       # 核心比对器测试
cargo test -p methratio   # 甲基化模块测试
cargo test                # 全部
```

### 代码检查

```bash
cargo check -p bsmap      # 快速类型检查
cargo clippy -p bsmap     # 代码风格检查
```

## 优化阶段

本项目经历了多轮系统性优化，详见 `benchmark/` 和 `docs/` 目录下的报告。各阶段的优化如下：

| 阶段 | 主题 | 核心优化项 |
|------|------|-----------|
| P1-P6 | 基础架构优化 | SIMD 探针、mmap 索引加载、热点路径重构 |
| P7-P8 | 正确性对齐 | 逐行对齐 C++ 输出、unique/multiple 分类修复 |
| P9 | 内存优化 | refcat/crefcat 生存期管理、FASTA ref 提前释放 |
| P10 | 并行化 | do_batch() 并行、BAM 直接写入 |
| P11 | 分配消除 | vtable dispatch 缓存、Vec 预分配、clone 消除、线程池扩展（16核） |

## 许可证

GPL-3.0
