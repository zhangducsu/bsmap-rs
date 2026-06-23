# P16 工程化性能优化交付报告

## 结论

P16 当前完成一项可保留的工程化优化：`format_sam()` 不再为 SE 热输出路径先构造完整 `AlignmentRecord`，而是直接构造最终 SAM 行，同时借用 reference accession，并使用静态 `ZS` 字符串。该改动不改变比对逻辑、SAM 字段顺序、optional tag 顺序、索引格式或随机多重命中选择。

同环境短验收显示，该优化在 P15 baseline-rerun 上对 WGBS example1、WGBS example2、mm10 RRBS SE、mm10 RRBS PE 四个 Rust 场景均降低 wall time，峰值 RSS 基本持平。Rust standalone index 继续单独计时，不并入 Rust/C++ 单样本 align 对比。

本轮也保留了已提交的非 TTY progress bar 隐藏逻辑，但它只作为批处理输出清理项，不作为稳定速度优化宣传。

## 保留改动

- `bsmap/src/align/output.rs`
  - `format_sam()` 绕过 `AlignmentRecord` 中间对象，直接构造 SAM 字符串。
  - 新增 `make_zs_tag_str()`，热路径使用 `&'static str`，避免每条记录分配 `String`。
  - 新增 `get_reference_name_ref()`，热路径借用 `chr_accessions`，避免每条记录 clone reference name。
  - 旧 `build_record()` 和 `format_sam_record()` 限定为 test-only，用于证明新旧格式化结果一致。
  - 新增单元测试 `test_format_sam_matches_record_formatter`。

- `bsmap/src/main.rs`
  - 已有非 TTY `ProgressBar::hidden()` 保留。
  - 本轮撤回了局部统计累加候选，因为 benchmark 显示 PE 回退。

- `benchmark/p16/run_short_validation.sh`
  - 固定 P16 短验收入口。
  - Rust standalone index 单独计时。
  - Rust/C++ align 对比只使用 warm `.bsi`。
  - example1 和 RRBS SE 做完整逐行 SAM 等价。
  - example2 和 RRBS PE 记录 SAM 统计与 C++ PE 既有限制。

## 验证环境

- OS：本地 WSL2 Ubuntu。
- 源码分支：`codex/p16-engineering-performance`。
- P15 baseline：`93e322d`。
- P15 baseline 结果目录：`D:/BSMAP/benchmark-results/p16/baseline-rerun-20260623T065323Z`。
- P16 结果目录：`D:/BSMAP/benchmark-results/p16/sam-direct-warm-20260623T072000Z`。
- Rust binary：`bsmap-rs/target/release/bsmap`。
- C++ binary：`bsmap-original/bsmap-2.90/bsmap`。
- Rust binary SHA256：`96ac6f102b77245444a40a802132a46148a69a90c4030ecf8ea769341c088186`。
- C++ binary SHA256：`09417edbab04b5552fdd9d3e6a9230b3d22e0660c607781c91c2d13e48bc4da6`。

## 固定数据与参数

| 场景 | Reference | Reads | 参数 |
|---|---|---|---|
| WGBS example1 SE | `bsmap-rs/benchmark/data/chr22_tail_1M.fa` | `data/wgbs/ex1_se75_10x/simulated.fastq.gz` | `-s 16 -v 0.08 -I 4 -p 1 -S 1` |
| WGBS example2 PE | 同上 | `data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz`, `simulated_2.fastq.gz` | `-s 16 -v 0.08 -I 4 -p 1 -S 1` |
| mm10 RRBS SE | `D:/BSMAP/benchmark-data/mm10/mm10.fa` | `Ctrl_10K_R1.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1` |
| mm10 RRBS PE | 同上 | `Ctrl_10K_R1.fq`, `Ctrl_10K_R2.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1` |

输入 SHA256：

| 文件 | SHA256 |
|---|---|
| `chr22_tail_1M.fa` | `e5bdd01f47504f51f3ef3e8ca132f741389d383a17d06d85ea04ab568618f267` |
| `mm10.fa` | `db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7` |
| `Ctrl_10K_R1.fq` | `13769b68c6f83fe476857ceb2936906ea8e9c0a5737ad00c75245f3e29da40dd` |
| `Ctrl_10K_R2.fq` | `839b5c7ca42968a1c5ce65e6ff65beb70c557147922a8674f9672e9c7e5e8a5f` |

## 正确性结果

| 场景 | 结果 |
|---|---|
| WGBS example1 | Rust/C++ 均为 66,120 records；完整逐行一致；`RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0 |
| WGBS example2 | Rust exit 0，66,958 records；C++ PE exit 134，0 records，按既有限制记录 |
| mm10 RRBS SE | Rust/C++ 均为 2,423 records；完整逐行一致；`RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0 |
| mm10 RRBS PE | Rust exit 0，4,884 records；Top RNAME 为 `chr1`，380 records，7.7805%，无 chr1 偏斜回退 |

## 性能对比

Rust standalone index 不并入 warm align wall time。下表只比较 Rust warm align。

| 场景 | P15 wall | P16 wall | 变化 | P15 RSS KiB | P16 RSS KiB |
|---|---:|---:|---:|---:|---:|
| WGBS example1 | 1.47 s | 1.35 s | -8.16% | 23,152 | 23,172 |
| WGBS example2 | 1.82 s | 1.73 s | -4.95% | 31,456 | 31,316 |
| mm10 RRBS SE | 8.42 s | 7.97 s | -5.34% | 803,888 | 804,156 |
| mm10 RRBS PE | 10.28 s | 9.80 s | -4.67% | 845,908 | 845,944 |

P16 Rust CPU 与 standalone index：

| 场景 | Rust user/sys | CPU% | standalone index |
|---|---:|---:|---:|
| WGBS example1 | 0.72 / 0.06 s | 58% | 0.73 s，969,720 KiB |
| WGBS example2 | 0.86 / 0.08 s | 54% | 复用 example1 index |
| mm10 RRBS SE | 6.49 / 5.88 s | 155% | 41.07 s，1,278,704 KiB |
| mm10 RRBS PE | 15.10 / 6.61 s | 221% | 复用 RRBS index |

C++ 对照：

| 场景 | C++ exit | C++ wall | C++ RSS KiB |
|---|---:|---:|---:|
| WGBS example1 | 0 | 2.19 s | 872,184 |
| WGBS example2 | 134 | 1.34 s | 872,304 |
| mm10 RRBS SE | 0 | 89.48 s | 2,061,404 |
| mm10 RRBS PE | 134 | 记录为既有 C++ PE 限制 | 记录为既有 C++ PE 限制 |

## 验证命令

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
python3 -m py_compile benchmark/p15/*.py
python3 -m unittest benchmark/p15/test_tools.py
```

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP
bash bsmap-rs/benchmark/p16/run_short_validation.sh \
  . \
  /mnt/d/BSMAP/benchmark-results/p16/sam-direct-warm-20260623T072000Z
```

## 被拒绝或撤回的候选

| 候选 | 结果 | 处理 |
|---|---|---|
| PE read-chain 预拆、线性 chr 分组、编码 Vec 复用、写行直写 | 同环境 benchmark 中 example1、RRBS SE/PE wall 变慢 | 已撤回 |
| ThinLTO + `codegen-units=1` + `panic=abort` | WGBS 变快，但 RRBS PE 变慢约 9% | 已撤回 |
| `codegen-units=1` + `panic=abort` | WGBS 变快，但 RRBS SE 变慢约 28%、PE 变慢约 4% | 已撤回 |
| `panic=abort` | example2 略快，但 example1、RRBS SE/PE 变慢 | 已撤回 |
| 只把已生成 SAM `String` 改成 ASCII 写入 | 没有减少核心分配，实测无稳定收益 | 已撤回 |
| 局部统计累加 | RRBS SE 略快，但 example1、example2、RRBS PE 回退 | 已撤回 |

## 未解决项

- P16 短验收已经达标，但尚未执行 WGBS 90G 或 RRBS 10G 长测；大样本结论仍需按 P16 计划补线程矩阵和多轮中位数。
- PE paired-output 仍有较多 `String` 分配，后续可继续做 direct writer，但必须按 P16 短验收逐步验证。
- SIMD mismatch 仍未接入默认热路径；接入前需要先做 microbench，并同时证明 WGBS/RRBS 不回退。
- 本地 DrvFS 仍会影响 page fault 与 mmap 行为；正式部署性能应优先在 WSL ext4 或 Linux ext4/overlay 环境复核。
