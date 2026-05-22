# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

BSMAP is a bisulfite sequencing read aligner for DNA methylation analysis. This repo contains both the original C++ implementation (BSMAP 2.90) and a complete Rust rewrite (`bsmap-rs/`). The Rust version is the active development target — it matches the C++ output exactly (validated 0-diff on 9,010-record Lambda test set) with dramatically lower memory for methylation ratio extraction (26 GB → < 1 GB).

## Build & Test Commands

```bash
cd bsmap-rs

# Build all workspace crates
cargo build --release

# Run unit tests (all crates)
cargo test

# Run tests for a specific crate
cargo test -p bsmap
cargo test -p methratio

# Run a single test
cargo test -p bsmap -- engine::tests

# Type-check only (faster than full build)
cargo check

# Check a specific crate
cargo check -p bsmap
```

There is no linting setup beyond `cargo check` / `cargo clippy`. Rust toolchain must be installed via `rustup`.

## Workspace Architecture

```
bsmap-rs/
├── Cargo.toml              # workspace root (resolver = "2")
├── bsmap/                   # core aligner binary — "bsmap"
├── methratio/               # methylation ratio calculator — "methratio"
├── bsp2sam/                 # BSP → SAM converter — "bsp2sam"
├── tests/data/              # test datasets (Lambda WGBS/RRBS, small ex1, realistic)
├── tests/reports/           # C++ vs Rust comparison reports
└── tools/                   # external tool repos (BSBolt, rrbssim, sherman) + Python scripts
```

`methdiff` is listed as a planned workspace member but currently commented out in `Cargo.toml`.

## Core Aligner Module Layout (`bsmap/src/`)

| Module | File(s) | Purpose |
|--------|---------|---------|
| `reference/` | `fasta.rs`, `binseq.rs`, `index.rs`, `index_io.rs`, `rrbs.rs` | FASTA loading → 2-bit binary encoding (Watson/Crick) → k-mer index (3-pass: count→alloc→fill) → `.bsi` file persistence |
| `reads/` | `fastq.rs`, `bam.rs`, `batch.rs`, `encode.rs` | Input parsing (needletail for FASTQ, noodles for SAM/BAM), quality/adapter/N trimming, batch encoding |
| `align/` | `engine.rs`, `seed.rs`, `mismatch.rs`, `gap.rs`, `extend.rs`, `output.rs` | Single-end alignment: seed extraction → index lookup → bit-parallel mismatch counting → gap alignment → SAM/BSP output |
| `pairs/` | `pair.rs`, `output.rs` | Paired-end logic: strand separation, insert-size filtering, two-pointer pairing |
| Root | `main.rs`, `lib.rs`, `cli.rs`, `param.rs`, `alphabet.rs`, `utils.rs` | Entry points, clap CLI (30+ flags, backward-compatible with C++ BSMAP), constants, DNA bit-manipulation, RNG |

## Data Flow

```
FASTA ref → BinSeqCollection (2-bit fwd+RC) → KmerIndex (3-pass, .bsi cache)
FASTQ reads → process_batch (trim/filter) → encode_read → SingleAlign/PairAlign
  → seed lookup in KmerIndex → mismatch count (SWAR popcount) → gap check → format_sam/format_bsp
```

All data is **fully in-memory** after loading. Atomic counters are used for lock-free alignment statistics. Parallelism via `rayon` (thread pool) and `crossbeam-channel` (producer-consumer pipelines between I/O, alignment, and output stages).

## Critical Constants & Bit Primitives

These must match C++ exactly — changes affect alignment correctness:

- `SEGLEN = 32` (bases per u64 word, 2 bits/base)
- `FIXELEMENT = 6` (u64 words per read segment, max 160 bases)
- `MAXSNPS = 15`, `MAXGAPS = 3`, `MAXHITS = 100`
- `BATCH_SIZE = 50000` (reads per batch)
- Default seed_size = 16 (WGBS), k-mer hash space = 3¹⁶ ≈ 43M

Bit primitives in `alphabet.rs` (use wrapping arithmetic to match C++):
- **XT/XT64** (`xt3()`/`xt3_64()`) — 3-letter seed hash, C and T map to same bucket
- **XC/XC64** (`xc32()`/`xc64()`) — C→T tolerance mask
- **XM64** (`xm64()`) — SWAR popcount, counts 0–32 mismatches in one u64
- C++ `__builtin_clzll` / `__builtin_ctzll` → Rust `u64::leading_zeros()` / `u64::trailing_zeros()`

## `.bsi` Index Format

Custom binary format (little-endian) for caching k-mer indices:
- 256-byte header: magic `BSMAPIDX`, version, seed_size, mode (0=WGBS, 1=RRBS), reference names
- Reference names: u16 length-prefixed UTF-8 strings
- Index data: bincode-serialized `KmerIndex`

Indices are written alongside the FASTA (e.g., `genome.fa.bsi`) and auto-detected on subsequent runs. Set `RUST_BACKTRACE=1` for debugging index compatibility issues.

## Key Dependencies

| Dependency | Role |
|-----------|------|
| `clap` (derive) | CLI argument parsing |
| `needletail` | Zero-copy FASTQ/FASTA parsing |
| `noodles` | Pure-Rust SAM/BAM I/O (replaces samtools) |
| `rayon` | Parallel computation |
| `crossbeam-channel` | High-performance channels for pipeline stages |
| `flate2` | gzip decompression |
| `serde` + `bincode` | Index serialization |
| `memmap2` | Memory-mapped index loading |
| `indicatif` | Progress bars |

## 性能优化工作流

每次优化按以下流程逐步推进，每步完成后进行编译验证 + 基准测试 + 报告，确认无回归后再进行下一步。

### 步骤

1. **实施优化**：修改代码，一次只做一项优化
2. **编译测试**：`cargo check -p bsmap` 和 `cargo test -p bsmap`，确保零错误、测试 0 失败
3. **构建 release**：`cargo build --release -p bsmap`
4. **基准测试**：用 example1 和 example2 数据运行 C++ BSMAP 和 Rust 版本（p=1 和 p=4），收集性能数据
5. **SAM 一致性验证**：对比 SAM 输出与上一 P 版本（必须 0 diff），对比与 C++ 的差异
6. **生成报告**：写入 `benchmark/PX_report_X.md`

### 基准测试数据

| 数据 | 文件 | 说明 |
|------|------|------|
| 参考基因组 | `chr22_tail_1M.fa` (1M bases) | `/home/zhang_i5edc0/bsmap_benchmark/data/` |
| Example 1 | `ex1_se75_10x.fastq` | SE 75bp, 10x, 133,334 reads, 66,120 aligned |
| Example 2 | `ex2_pe150_10x_1.fastq` + `ex2_pe150_10x_2.fastq` | PE 150bp, 10x, ~13,334 pairs |

### 基准测试命令

```bash
# C++ BSMAP (SE)
bsmap -a <reads.fastq> -d chr22_tail_1M.fa -o <out.sam> -s 16 -v 0.08 -I 4 -p 1
bsmap -a <reads.fastq> -d chr22_tail_1M.fa -o <out.sam> -s 16 -v 0.08 -I 4 -p 4

# C++ BSMAP (PE)
bsmap -a <reads1.fastq> -b <reads2.fastq> -d chr22_tail_1M.fa -o <out.sam> -s 16 -v 0.08 -I 4 -p 1

# Rust BSMAP (SE)
bsmap align -a <reads.fastq> -d chr22_tail_1M.fa -o <out.sam> -s 16 -v 0.08 -I 4 -p 1

# Rust BSMAP (PE)
bsmap align -a <reads1.fastq> -b <reads2.fastq> -d chr22_tail_1M.fa -o <out.sam> -s 16 -v 0.08 -I 4 -p 1
```

性能数据通过 `/usr/bin/time -v` 收集（Elapsed time、Maximum resident set size、CPU utilization）。

### 对比基线

- **C++ BSMAP 2.90**（`/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap`）
- **上一个 P 版本**（如 P11-7~10 vs P11-12~14）

### 报告要求

每份报告须包含：
- 优化项描述（编号、文件、改动内容、状态）
- 基准测试用的完整代码（命令）和参数、参考基因组和数据
- 性能对比表（耗时、峰值内存、CPU 利用率、比对读段数）
- SAM 详情（读段数、unique/multiple 分布、vs C++ diff 行数、vs 上一版本 diff 行数）
- 增量对比（vs C++、vs 上一 P 版本的速度/内存变化）
- 总结（正确性、性能、降级项说明）

### 计划降级原则

计划中明确标注"建议降级"或"边际（< 1%）"的优化项可直接跳过，在报告中注明原因即可。

### 已知问题

- C++ BSMAP PE 模式在 chr22_tail_1M 测试数据上 buffer overflow 崩溃（0 条 SAM 输出），PE 对比仅限 Rust vs Rust
- Rust vs C++ SE 有 ~3,000 行 SAM 差异，为已知比对逻辑差异（alternative alignment position 选择不同），非回归

## Current Status

**Complete:** Core aligner (SE/PE WGBS + RRBS), methratio, bsp2sam. Validated 0-diff against C++ BSMAP on Lambda WGBS PE dataset. BAM output works via noodles.

**Remaining work:**
- `methdiff` crate (differential methylation — Phase 7, not yet started)
- RRBS paired-end pairing logic bug (Rust finds pairs that C++ misses, but C++ crashes with buffer overflow in RRBS mode)
- RRBS index OOM at seed_size=12 on large genomes (~3.7 GB)
- Advanced optimization: SIMD mismatch, NUMA-aware threading (Phase 9)

## Design Decisions

- **No `unsafe`** unless absolutely required for performance
- **100% backward-compatible** CLI with C++ BSMAP 2.90 — same flags, same output formats (SAM/BAM/BSP)
- Alignment logic matches C++ exactly, including edge cases like N-base handling and strand-specific output
- Python scripts (`methratio.py`, `methdiff.py`, `bsp2sam.py`) are replaced by Rust binaries with identical output
