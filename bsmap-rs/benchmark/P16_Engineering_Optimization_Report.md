# P16 工程化优化交付报告

## 结论

P16 保留一项低风险工程改动：stderr 非 TTY 时隐藏 progress bar，避免非交互 benchmark/批处理场景维护可见进度条。该改动不改变 SAM、BSP、索引格式或比对选择逻辑。

本轮同时建立了 `benchmark/p16/run_short_validation.sh`，用于固定 P16 短验收口径：Rust standalone index 单独计时，Rust/C++ align 比较只使用 warm `.bsi`。多项看似合理的优化候选被同环境 benchmark 拒绝，没有带入最终代码。

## 保留改动

- `bsmap/src/main.rs`
  - 新增 `progress_bar(len)`。
  - `stderr().is_terminal()` 为 true 时保留原交互进度条。
  - 非 TTY 时使用 `ProgressBar::hidden()`。

- `benchmark/p16/run_short_validation.sh`
  - 固定 WGBS example1/example2 与 mm10 RRBS 10K 验收。
  - 记录 Rust/C++ binary、reference、reads SHA256。
  - 对 Rust standalone index 单独计时，不并入 align。
  - example1 和 RRBS SE 使用 `compare_sam.py` 做完整逐行等价。
  - RRBS PE 使用内置 SAM 统计，记录 FLAG/RNAME 分布。

## 被拒绝的候选

| 候选 | 结果 | 处理 |
|---|---|---|
| PE read-chain 预拆、线性 chr 分组、编码 Vec 复用、写行直写 | 同环境 benchmark 中 example1、RRBS SE/PE wall 变慢 | 已撤回 |
| ThinLTO + `codegen-units=1` + `panic=abort` | WGBS 变快，但 RRBS PE 变慢约 9% | 已撤回 |
| `codegen-units=1` + `panic=abort` | WGBS 变快，但 RRBS SE 变慢约 28%、PE 变慢约 4% | 已撤回 |
| `panic=abort` | example2 略快，但 example1、RRBS SE/PE 变慢 | 已撤回 |

## 正式验证环境

- OS：本地 WSL2 Ubuntu
- 源码分支：`codex/p16-engineering-performance`
- P15 baseline：`93e322d`
- baseline binary SHA256：`a30c6f8de30435c5cba032601d4391c5a86e3e0ab48bab6fde654220d83a6299`
- P16 binary SHA256：`a5a6707424b1b9fb8b2a41c0d370ec7dd38f11e72a4bc9bc859ba764c27b8847`
- P16 正式结果目录：`D:/BSMAP/benchmark-results/p16/progress-hidden-warm-20260623T035342Z`
- P15 baseline 结果目录：`D:/BSMAP/benchmark-results/p16/baseline-warm-20260623T033414Z`

## 固定数据与参数

| 场景 | Reference | Reads | 参数 |
|---|---|---|---|
| WGBS example1 SE | `bsmap-rs/benchmark/data/chr22_tail_1M.fa` | `ex1_se75_10x/simulated.fastq.gz` | `-s 16 -v 0.08 -I 4 -p 1 -S 1` |
| WGBS example2 PE | 同上 | `ex2_pe150_10x/simulated_1.fastq.gz`, `simulated_2.fastq.gz` | `-s 16 -v 0.08 -I 4 -p 1 -S 1` |
| mm10 RRBS SE | `D:/BSMAP/benchmark-data/mm10/mm10.fa` | `Ctrl_10K_R1.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1` |
| mm10 RRBS PE | 同上 | `Ctrl_10K_R1.fq`, `Ctrl_10K_R2.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1` |

## 正确性结果

| 场景 | 结果 |
|---|---|
| WGBS example1 | Rust/C++ 66,120/66,120 完整逐行一致；`RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0 |
| WGBS example2 | Rust exit 0，66,958 records；C++ PE exit 134，0 records，按既有限制记录 |
| mm10 RRBS SE | Rust/C++ 2,423/2,423 完整逐行一致；`RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0 |
| mm10 RRBS PE | Rust exit 0，4,884 records；Top RNAME 为 chr1，380 records，7.7805%，无 chr1 偏斜回退 |

## 性能对比

P16 当前保留项主要是非交互路径清理，单轮 wall time 受 DrvFS/page-cache 抖动影响明显，不把它宣传为稳定速度收益。正式同环境单轮结果如下：

| 场景 | P15 wall | P16 wall | P15 RSS KiB | P16 RSS KiB |
|---|---:|---:|---:|---:|
| WGBS example1 | 1.18 s | 1.41 s | 23,020 | 23,076 |
| WGBS example2 | 1.92 s | 1.76 s | 31,332 | 31,168 |
| mm10 RRBS SE | 7.78 s | 9.42 s | 804,280 | 803,768 |
| mm10 RRBS PE | 9.36 s | 10.21 s | 845,748 | 845,596 |

因此 P16 不声称本轮已获得稳定 wall-time 提升；交付价值在于：

- 明确建立了本地短验收脚本和 warm-index 计时边界。
- 证明多个全局 release profile 优化不能作为默认项。
- 保留非 TTY progress 抑制这个无语义风险的批处理清理。
- 给 P17 留出更明确的优化方向：direct SAM writer、PE scratch/range 重构、真正的 profile-guided hot-path 优化。

## 验证命令

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
python3 -m py_compile benchmark/p15/*.py
python3 -m unittest benchmark/p15/test_tools.py
bash benchmark/p16/run_short_validation.sh . /mnt/d/BSMAP/benchmark-results/p16/<run-id>
```

## 未解决项

- P16 没有达成稳定 wall-time 改善；需要 P17 继续做 direct SAM writer 和 PE pairing scratch 的受控重构。
- 当前本地短测在 DrvFS 与 page-cache 下波动较大；正式性能结论仍应优先使用 ext4 工作区或服务器 ext4 环境。
- SIMD mismatch 已存在但未接入；源码显示 AVX2 路径仍需要落回数组逐个 popcount，接入前必须先做 microbench。
