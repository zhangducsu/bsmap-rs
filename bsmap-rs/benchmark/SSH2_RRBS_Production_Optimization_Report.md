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

### 2026-06-28：1M baseline 与 reverse cache 优化

1M baseline 运行路径：`/workspace/benchmark_results/ssh2/20260627T175425Z-2219/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 1,000,000 | 108.50 s | 748% | 912,408 | 99.22 s | 292% | 2,486,844 | 1.094 | sorted multiset 仅 3 条 ZP/ZL 边界差异 |

1M baseline 判定：

- Rust RSS 明显低于 C++，但 wall time 慢于 C++，未达到 `<= C++ / 2` 的 SSH2 目标。
- SAM mapped 数均为 253,102；FLAG/RNAME/NM/ZP/ZL 统计一致。
- sorted multiset 仅 3 条真实差异，均为 `chr4_GL456350_random:227672` 上 C++ `CCGG_seglen()` 末端越界式 ZP/ZL 标签：C++ 输出 `ZP=227672,ZL=139496`，Rust 不输出 ZP/ZL；QNAME/RNAME/POS/FLAG/NM 一致。

负收益尝试：

- `1ef4aa1 perf: shift query in mismatch hot path` 把 mismatch 热路径改成 query shift，10K/100K 速度变快但 mapped 数回退为 2,746/27,523，破坏 SAM 语义；已用 `b4c082d` 回退。
- `f43a289 perf: skip duplicate invalid mismatch candidates` 在本地测试通过，服务器 10K/100K SAM 等价，但 Rust 10K/100K wall 从 1.41/10.85 s 退化到 1.51/11.86 s；已用 `6498018` 回退。
- `039f2c2 perf: unroll scalar mismatch word loop` 在本地 `cargo check/test/build` 通过，服务器 10K/100K/1M SAM 集合与保留版一致；但性能收益不足：10K 从 1.48 s 到 1.37 s，100K 从 11.05 s 到 10.81 s，1M 从 46.09 s 到 46.08 s，属于短样本噪声或无实质收益；为避免增加热路径代码复杂度，已用 `45c6c35` 回退。验证路径：`/workspace/benchmark_results/ssh2/20260627T191911Z-4934/summary.json`、`/workspace/benchmark_results/ssh2/20260627T192523Z-5139/summary.json`。

保留优化：`0781e27 perf: add optional RRBS reverse reference cache`。

- RRBS v10 `.bsi` 默认只保存 forward `refcat`，旧热路径遇到反向链候选时通过 `fill_reverse_window()` 逐候选现场生成 reverse 窗口。
- `0781e27` 新增从 forward `refcat` 一次性 materialize C++ padded reverse `crefcat` 的路径；不改 `.bsi` 格式。
- 自动策略：RRBS 且 `-E` 未指定或 `-E >= 500000` 时默认启用；小样本默认关闭。`BSMAP_RRBS_MATERIALIZE_REVERSE=1/0` 可强制开关。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过；新增单测覆盖 materialized reverse 与 full builder 的 `crefcat` 一致性，以及自动策略阈值。

默认策略 10K/100K 复测路径：`/workspace/benchmark_results/ssh2/20260627T190116Z-4318/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | reverse cache | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 10,000 | 1.48 s | 622% | 893,076 | 68.24 s | 100% | 2,057,224 | 0.022 | off | streaming diff 0；sorted diff 0 |
| 100,000 | 11.05 s | 734% | 911,560 | 76.76 s | 115% | 2,117,744 | 0.144 | off | streaming diff 0；sorted diff 0 |

默认策略 1M 复测路径：`/workspace/benchmark_results/ssh2/20260627T190527Z-4476/summary.json`。

Rust binary SHA256：`1b601d5e234aef043faf3d34c581ddacfe6226d7533e74d1a04a0c7e20721870`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | reverse cache | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---|
| 1,000,000 | 46.09 s | 580% | 1,722,940 | 99.83 s | 293% | 2,486,544 | 0.462 | on，8.51 s | sorted multiset 253,099/253,102 完全一致；仅 3 条 C++ ZP/ZL 边界差异 | 达到 SSH2 1M 速度/RSS 门槛 |

SAM 摘要：

| limit | Rust mapped | C++ mapped | Rust SAM SHA256 | C++ SAM SHA256 |
|---:|---:|---:|---|---|
| 1,000,000 | 253,102 | 253,102 | `44fcf583903fb063245ecf4dec77843aa20317300d681d5207e46729dfe1f92a` | `f8e5ed0d568313828d7a2fed220ca94cd7a12197409be152fa45c2723e910c34` |

## 未解决项

- SSH2 1M 默认策略已达速度/RSS 门槛；full SE 本轮尚未重跑，不能把 1M 结果写成 full 已验证结论。
- 1M sorted multiset 仍有 3 条 `ZP/ZL` 差异；源码证据指向 C++ `CCGG_seglen()` 在最后一个 CCGG site 后的末端 fragment 上先解引用再检查边界。Rust 保持安全行为，不伪造该越界式标签。
- reverse cache 会把 RRBS 大样本 RSS 从约 0.87 GiB 提高到约 1.64 GiB，但仍低于 C++ 1M 的约 2.37 GiB；若未来在内存更小机器上运行，可用 `BSMAP_RRBS_MATERIALIZE_REVERSE=0` 强制关闭。
