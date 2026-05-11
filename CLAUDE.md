# CLAUDE.md — BSMAP-rs Project Context

## Project Overview

Refactoring BSMAP v2.90 (bisulfite sequencing read aligner) from C++/Python/Shell into a unified Rust workspace. Original C++ source at `F:/OneDrive/Documents/BSMAP重构/源代码/bsmap-2.90/bsmap-2.90/`.

**Target suite:**

| Component | Status | Description |
|-----------|--------|-------------|
| `bsmap` | Phase 1 done | Core aligner binary |
| `methratio` | Not started | Methylation ratio extraction |
| `methdiff` | Not started | Differential methylation |
| `bsp2sam` | Not started | BSP → SAM converter |

## Core Architecture

### Crate structure (workspace at `bsmap-rs/`)
```
bsmap-rs/
├── Cargo.toml               # workspace root
├── bsmap/                    # core aligner binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs           # entry point
│       ├── lib.rs            # module exports
│       ├── cli.rs            # clap CLI (30 options)
│       ├── param.rs          # AlignConfig, constants, types
│       ├── alphabet.rs       # DNA encoding, bit-manip (XT/XC/XM)
│       ├── utils.rs          # RNG (SplitMix64), Timer, hit comparators
│       ├── reference/        # (Phase 1) FASTA → binary → k-mer index
│       ├── reads/            # (Phase 2) FASTQ/FASTA/SAM/BAM loading
│       ├── align/            # (Phase 3) Single-end alignment engine
│       └── pairs/            # (Phase 4) Paired-end alignment
├── methratio/                # (Phase 5)
├── methdiff/                 # (Phase 6)
└── bsp2sam/                  # (Phase 6)
```

### Key Dependencies
- `clap` (derive) — CLI
- `anyhow`, `thiserror` — error handling
- `rayon` — thread pool
- `crossbeam-channel` — producer-consumer pipelines
- `needletail` — FASTQ/FASTA parsing (zero-copy)
- `noodles` — SAM/BAM I/O (pure Rust)
- `flate2` — gzip support
- `log`, `env_logger` — logging

### Memory & Concurrency Strategy
- **Full in-memory**: `Vec<AtomicU32>` / `Vec<AtomicU16>` for genome-wide counters
- **Lock-free**: atomic operations for alignment statistics and methylation counters
- **Parallelism**: rayon for worker pools, crossbeam-channel for I/O→worker→output pipelines

### Critical Bit-Manipulation Primitives (alphabet.rs)
- **XT/XT64** → `xt3()` / `xt3_64()`: 3-letter seed hash, C/T → same bucket
- **XC/XC64** → `xc32()` / `xc64()`: C→T tolerance mask
- **XM64** → `xm64()`: SWAR popcount, counts mismatches (0-32) in one u64
- **`__builtin_clzll`** → `u64::leading_zeros()` (safe, compiles to `lzcnt`)
- **`__builtin_ctzll`** → `u64::trailing_zeros()` (safe, compiles to `tzcnt`)

### Key Constants
- `SEGLEN = 32` (bases per u64 word, 2 bits/base)
- `FIXELEMENT = 6` (u64 words per read segment, max 160 bases)
- `MAXSNPS = 15`, `MAXGAPS = 3`, `MAXHITS = 100`
- `BATCH_SIZE = 50000` (reads per batch)
- Default seed_size = 16 (WGBS), k-mer hash space = 3^16 ≈ 43M

---

## Current Progress: Phase 1 — COMPLETE

### All source files (11 files, 0 rust-analyzer errors/warnings):

| File | Lines | Purpose |
|------|-------|---------|
| `bsmap/src/main.rs` | ~50 | Entry point: CLI parse → validate → logging init |
| `bsmap/src/lib.rs` | ~10 | Module re-exports (now includes `reference`) |
| `bsmap/src/cli.rs` | ~260 | All 30 BSMAP options, `validate()` method, tests |
| `bsmap/src/param.rs` | ~341 | Constants, `AlignConfig`, `AlignStats`, types (`Hit`, `GHit`, etc.) |
| `bsmap/src/alphabet.rs` | ~483 | Encoding tables, XT/XC/XM bit primitives, pack/unpack, 14 tests |
| `bsmap/src/utils.rs` | ~172 | `Timer`, `myrand()` SplitMix64 RNG, `hit_comp()`, tests |
| `bsmap/src/reference/mod.rs` | ~14 | Module declarations |
| `bsmap/src/reference/fasta.rs` | ~120 | FASTA loader (plain + gzipped via flate2), tests |
| `bsmap/src/reference/binseq.rs` | ~250 | 2-bit binary encoding (fwd + RC), `BinSeqCollection` concatenation, `Block` detection, tests |
| `bsmap/src/reference/index.rs` | ~270 | WGBS/RRBS k-mer index: 3-pass build (count → alloc → fill), tests |
| `bsmap/src/reference/rrbs.rs` | ~230 | IUPAC digestion site parsing, `find_sites()`, RRBS index builder, tests |

### Cargo check status
- **rust-analyzer**: 0 errors, 0 warnings across all 11 files
- **`cargo check`**: Not run — Rust toolchain not installed on this system. Install via `rustup` or winget.

### Phase 1 Implementation Details

**fasta.rs** — Manual FASTA parser:
- Reads byte-by-byte for efficiency, handles multi-FASTA
- Supports gzipped input via `flate2::bufread::GzDecoder`
- Auto-uppercases all sequences
- `Reference { name, seq, len }` struct

**binseq.rs** — Binary encoding + concatenation:
- `encode_forward()`: left-to-right using `ALPHABET`, pads with A (0)
- `encode_revcomp()`: right-to-left using `REV_ALPHABET`, pads with T (3)
- `BinSeqCollection::from_references()`: runs encoding, detects unmasked blocks, concatenates into `refcat`/`crefcat`, builds `ref_anchor`
- `Block { id, begin, end }`: id = chr×2+chain (0=fwd, 1=RC)
- `hit2int(chr, loc)`: maps to flat offset using ref_anchor

**index.rs** — WGBS k-mer index:
- `KmerIndex::build_wgbs()`: 3-pass algorithm
  - Pass 1: `count_frequencies()` — prefetch-based k-mer counting over all blocks
  - Pass 2: Compute prefix sums, apply `max_kmer_ratio` cutoff
  - Pass 3: `fill_positions()` — write (chr, loc) into flat positions array
- `lookup(seed_hash)` → slice of u32 positions
- RRBS path is scaffolded, needs wiring when alignment engine is integrated

**rrbs.rs** — RRBS digestion:
- `DigestionSite::parse("C-CGG")` → IUPAC expansion
- `find_sites()`: naive string search for all digestion patterns
- `build_rrbs_index()`: BSW (forward) and BSC (RC) seed index positions

---

## Next Phase: Phase 2 — Reads Module

### What to build
1. **`bsmap/src/reads/mod.rs`** — module structure
2. **`bsmap/src/reads/fastq.rs`** — FASTQ/FASTA read loading via `needletail`
   - Parse query reads (single-end or paired-end)
   - Handle gzipped input (needletail auto-detects)
   - Return `ReadInf` structs (name, seq, qual)
3. **`bsmap/src/reads/bam.rs`** — SAM/BAM read loading via `noodles`
   - Parse SAM/BAM as query input
   - Extract read sequences and quality scores
4. **`bsmap/src/reads/batch.rs`** — Batch management
   - 50K reads per batch (`BATCH_SIZE`)
   - Quality trimming (3'-end)
   - N-filtering (max N's)
   - Adapter trimming
   - Binary encoding of reads for alignment

### Key C++ references
- `reads.h` / `reads.cpp` — `LoadReads()`, batch loading logic
- `param.h` — `ReadInf` struct
- Look at `align.h` for how reads are prepared before alignment

### Implementation notes
- needletail returns owned data (zero-copy within its buffer); convert to `Vec<u8>` for batch storage
- noodles BAM has a more complex API — focus on SAM first
- Quality trimming: trim from 3' end until quality ≥ threshold
- needletail auto-detects gzip, so no separate gz path needed

---

## User Preferences (from conversation)

- **Language**: 中文 for all communication
- **Dependencies**: needletail for FASTQ, noodles for SAM/BAM
- **No `unsafe`** unless absolutely necessary
- **Memory**: Full in-memory, atomics for lock-free concurrency
- **Consistency**: Bit manipulations must match C++ exactly (wrapping arithmetic)

---

## Verification Checklist for Phase 2
- [ ] needletail FASTQ parsing: handle both plain and gzipped
- [ ] Batch loading: 50K reads at a time
- [ ] Quality trimming: 3'-end, threshold from `-q`
- [ ] N-filtering: max N count from `-f`
- [ ] Adapter trimming: from `-A` sequences
- [ ] Binary encoding of batch reads for alignment
- [ ] All unit tests pass
