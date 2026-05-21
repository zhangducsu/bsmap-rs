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
