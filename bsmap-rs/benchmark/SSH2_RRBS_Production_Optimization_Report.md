# SSH2 RRBS 生产级优化报告

## 目标判定

SSH2 完成标准：

- Rust 与 C++ 使用完全相同参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1`，抽样时双方增加相同 `-E N`。
- Rust standalone index 不计入与 C++ 单样本 align 时间比较。
- Rust/C++ SAM 字段 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。
- Rust RSS 低于或相当于 C++。
- Rust wall time 小于等于 C++ wall time 的 50%。

## SSH2 起点

基线分支：`codex/ssh2-rrbs-production-optimization`，从 SSH1 `d7373f8` 创建。

SSH1 已知结果：

| 场景 | Rust | C++ | 判定 |
|---|---:|---:|---|
| 10K SE mapped | 2,423 | 2,423 | 字段 diff 为 0 |
| 10K SE Rust stage | 1.41 s | 66.77 s | 10K 受 C++ normal invocation 固定成本影响，不代表 full |
| full SE wall | 3,778.00 s | 536.04 s 旧基线 | Rust 慢约 7 倍 |
| full SE RSS | 913,116 KiB | 约 2.87 GiB | Rust 内存明显更低 |

SSH2 不再接受 10K 单次噪声作为性能结论；新增 100K/1M 中等抽样用于筛选优化。

## 新增工具

- `benchmark/ssh2/run_server_rrbs_subset.sh`
  - 输入：full RRBS R1 `/workspace/00_data/rrbs/Ctrl_R1.fq.gz`
  - read range：通过 `SSH2_LIMITS` 设置，例如 `10000 100000 1000000`
  - 参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1 -E <limit>`
  - Rust：使用已有 `.bsi` warm index
  - C++：normal invocation
  - 输出：metadata、binary/input SHA、time/RSS/CPU、SAM stats、streaming field diff、summary JSON

## Baseline 结果

运行命令：

```bash
SSH2_LIMITS="10000 100000" \
bash bsmap-rs/benchmark/ssh2/run_server_rrbs_subset.sh \
  /tmp/ssh1_sparse_20260627T153127Z_68025/repo \
  /workspace/benchmark_results/ssh2
```

运行路径：`/workspace/benchmark_results/ssh2/20260627T164000Z-73428/summary.json`。

| limit | Rust wall | Rust RSS KiB | C++ wall | C++ RSS KiB | Rust/C++ wall | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---|---|
| 10,000 | 1.41 s | 893,488 | 65.93 s | 2,057,220 | 0.021 | streaming diff 0 | 通过 |
| 100,000 | 10.44 s | 911,620 | 76.20 s | 2,117,748 | 0.137 | streaming diff 受输出顺序影响；sorted multiset 仅 2 条真实差异 | 未通过 correctness gate |

100K 进一步用排序后的 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` multiset 比较：

- C++ records：24,236
- Rust records：24,236
- exact multiset records：24,234
- C++ only records：2
- Rust only records：2
- C++ only QNAME：0
- Rust only QNAME：0

这说明 100K 的大面积 streaming diff 主要来自输出顺序不同，但仍有 2 条真实 C++ 语义差异。SSH2 下一步必须先定位这 2 条差异，再继续速度优化。

## seed mask 修复复测

提交：`9709b55 fix: match C++ seed mask ordering for RRBS`

运行命令：

```bash
CPP_BINARY=/workspace/02_software/bsmap-2.90/bsmap \
SSH2_LIMITS="10000 100000" SSH2_PROFILE_RRBS=0 \
bash bsmap-rs/benchmark/ssh2/run_server_rrbs_subset.sh \
  /tmp/ssh1_sparse_20260627T153127Z_68025/repo \
  /workspace/benchmark_results/ssh2
```

运行路径：`/workspace/benchmark_results/ssh2/20260627T171551Z-1352/summary.json`。

Rust binary SHA256：`cd988290b088fc7e905c620d1a544aba68ddb9b156db366ff906b779111620f2`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 10,000 | 1.41 s | 623% | 893,088 | 67.38 s | 100% | 2,057,224 | 0.021 | streaming diff 0；sorted multiset diff 0 | 通过 |
| 100,000 | 10.85 s | 730% | 911,576 | 75.72 s | 114% | 2,117,748 | 0.143 | streaming diff 0；sorted multiset diff 0 | 通过 |

SAM 摘要：

| limit | Rust mapped | C++ mapped | Rust SAM SHA256 | C++ SAM SHA256 |
|---:|---:|---:|---|---|
| 10,000 | 2,423 | 2,423 | `420e34a3fa39086effbff8341cde5bacf90fde9bf57a32b39e0cb48eeedd9ad0` | `fdc40e1d1ad42b786e1b093cad5efe614b547a5bea0a2630f5e8fae69423a64a` |
| 100,000 | 24,236 | 24,236 | `f6c9728a8da70785a9ebe98a00d355110a87cc14226af3fe0d29ba322b6177ba` | `6149fecd52e5219befe9a9c028dabbd1afd15b88db98b0707391d600f7dec357` |

修复原因：

- Rust 旧实现没有保留 C++ `xseedreg_array` 等价的 seed mask 排序语义，RRBS seed candidate 计数把含 `N` 的 seed 当成普通 seed。
- C++ `CountSeeds()` 对含 `N` 的 seed 使用 `<< 12` 权重惩罚，使这类 seed 在排序中后移；但 `SnpAlign()` 并不会因为该 seed mask 跳过后续扫描。
- `9709b55` 从 `EncodedRead` 提取 seed mask，用 mask 权重参与 RRBS seed 排序，同时移除了扩展阶段对 `reg_mask == 0` 的错误跳过。

针对 baseline 中 2 条真实差异的单 read 诊断也已通过：

- read `LH00128:190:22GYNKLT4:5:1101:3581:29730`：默认输出与 C++ 完全一致；`-r 2` 下 Rust/C++ 均为 100 条，排序 multiset 完全一致。
- read `LH00128:190:22GYNKLT4:5:1103:50581:11268`：默认输出与 C++ 完全一致；`-r 2` 排序 multiset 完全一致。

## 优化日志

### 2026-06-28：SSH2 基线准备

- 从 SSH1 `d7373f8` 新建 SSH2 分支。
- 新增 SSH2 计划文档和 subset runner。
- 目标从“改善”提升为明确生产门槛：Rust full SE wall `<= C++ / 2`，且 RSS 不高于或相当于 C++。
- 服务器 10K/100K subset baseline 已完成。10K 完全一致；100K mapped 数一致但存在 2 条 sorted multiset 差异，correctness gate 尚未通过。

### 2026-06-28：RRBS seed mask 排序修复

- 提交 `9709b55` 对齐 C++ `CountSeeds()` 的 seed mask 权重语义。
- 本地 `cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。
- 服务器 Docker 同步到 `9709b55` 后 release build 通过。
- 10K 与 100K RRBS SE 均达到 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` streaming diff 0，sorted multiset diff 0。
- 100K 当前 Rust wall 10.85 s，C++ wall 75.72 s；Rust 约为 C++ 的 14.3%，满足 SSH2 subset 性能门槛。

## 未解决项

- 100K correctness gate 已在 `9709b55` 清零；尚未重跑 SSH2 1M baseline。
- full SE 的 C++ 最新 wall/RSS 仍需 SSH2 runner 复测，不能只沿用旧 `536.04s`。
- Rust full SE 与 C++ full SE 旧结果存在 `+124` mapped 差异，SSH2 full acceptance 前必须用 streaming diff 复核并解释。
