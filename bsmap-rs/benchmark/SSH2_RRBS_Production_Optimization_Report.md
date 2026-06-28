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

full SE 复测支持：`fe3f84a bench: allow full SSH2 RRBS SE runs`。

- `SSH2_LIMITS=full` 时 runner 不再传 `-E`，用于验证真实 full RRBS SE。
- 运行路径：`/workspace/benchmark_results/ssh2/20260627T194342Z-5737/summary.json`。
- Docker commit：`6aa44ef193cbe36d0e0f4eb8a9875278ad1eeca4`，对应本地提交 `fe3f84a`。
- Rust binary SHA256：`1b601d5e234aef043faf3d34c581ddacfe6226d7533e74d1a04a0c7e20721870`。
- C++ binary SHA256：`d74f45b109c3229a11b453bb65d57659dbfbdfc0fc40a92937fa9c54d24191a6`。

| case | wall | user | sys | CPU | RSS KiB | mapped | SAM diff | 判定 |
|---|---:|---:|---:|---:|---:|---:|---|---|
| Rust full SE | 1286.32 s | 8819.69 s | 163.49 s | 698% | 1,723,652 | 8,873,078 | sorted multiset 8,873,043/8,873,078；35 条仅 C++ terminal ZP/ZL 边界标签差异 | RSS 低于 C++，wall 慢于 C++，未达 SSH2 |
| C++ full SE | 1041.84 s | 7674.27 s | 164.52 s | 752% | 2,538,788 | 8,873,078 | baseline | 当前同 runner 同参数 C++ full 基线 |

full 结论：

- Rust mapped、Top RNAME 分布与 C++ 对齐，不再有 SSH1 full mapped 多 124 条的问题。
- Rust RSS 约 1.64 GiB，低于 C++ 约 2.42 GiB。
- Rust wall 仍为 C++ 的 1.235 倍，距离 `<= C++ / 2` 差距很大；SSH2 不能用 1M 抽样替代 full 结论。
- Rust stderr 中核心比对耗时为 1117.572323 s，总耗时为 1286.16 s，瓶颈主要在 align core，不是 standalone index 或报告口径。

保留优化：`d406835 perf: add RRBS mode range lookup`。

- 新增运行时 RRBS mode side table，SE 默认关闭 cross-chain 时按 `(seed_hash, cmodeindex)` 直接定位 mode bucket。
- 保留 C++ 随机起点对 non-BSC logical bucket 的取模语义；PE 或 `-n 1` 仍走原完整 bucket 路径。
- 目标是减少扫描非目标 mode 的 bucket 成本；不改变进入 mismatch 的语义条件。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。

mode range 10K/100K 复测路径：`/workspace/benchmark_results/ssh2/20260627T204859Z-6991/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 10,000 | 1.94 s | 494% | 1,002,120 | 66.10 s | 100% | 2,057,220 | 0.029 | streaming diff 0；sorted diff 0 | side table 首次构建成本压过小样本收益 |
| 100,000 | 10.80 s | 709% | 1,019,656 | 78.00 s | 115% | 2,117,744 | 0.138 | streaming diff 0；sorted diff 0 | 相对 reverse-cache 默认 11.05 s 小幅改善 |

mode range 1M 复测路径：`/workspace/benchmark_results/ssh2/20260627T205310Z-7153/summary.json`。

Rust binary SHA256：`1148a530288ae4e9453e503a0092a0cbc8e44205db9f27c4f92dbca75f7920e3`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 1,000,000 | 42.35 s | 551% | 1,830,968 | 100.01 s | 296% | 2,487,628 | 0.423 | sorted multiset 253,099/253,102；仍仅 3 条 C++ terminal ZP/ZL 边界标签差异 | 相对 reverse-cache 默认 46.09 s 改善约 8.1%，RSS 增加约 108 MiB |

mode range full 复测路径：`/workspace/benchmark_results/ssh2/20260627T205842Z-7339/summary.json`。

- Docker commit：`1cda4fb6fd28b6db5038e86fa8b955ceffd93ba4`，对应本地提交 `d406835`。
- Rust binary SHA256：`1148a530288ae4e9453e503a0092a0cbc8e44205db9f27c4f92dbca75f7920e3`。
- C++ binary SHA256：`d74f45b109c3229a11b453bb65d57659dbfbdfc0fc40a92937fa9c54d24191a6`。
- Reference SHA256：`db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7`。
- Rust warm align 不包含 standalone index；run 前后 index SHA256 均为 `1329966ddda5aedd9fc7e13cb84a4e755cd632df3d14a0de32a239a29561e634`。

| case | wall | user | sys | CPU | RSS KiB | mapped | FLAG 分布 | Top RNAME | SAM diff | 判定 |
|---|---:|---:|---:|---:|---:|---:|---|---|---|---|
| Rust full SE | 1134.39 s | 7536.25 s | 154.93 s | 678% | 1,831,660 | 8,873,078 | 0:3,540,475；16:3,572,901；256:880,617；272:879,085 | chr1 6.6778% | sorted multiset 8,873,043/8,873,078；35 条仅 C++ terminal ZP/ZL 边界标签差异 | 相对旧 Rust full 1286.32 s 改善约 11.8%；RSS 仍低于 C++ |
| C++ full SE | 1050.84 s | 7682.63 s | 218.65 s | 751% | 2,539,832 | 8,873,078 | 0:3,540,475；16:3,572,901；256:880,617；272:879,085 | chr1 6.6778% | baseline | 当前同 runner 同参数 C++ full 基线 |

mode range full 结论：

- 该优化值得保留：full wall 从 1286.32 s 降到 1134.39 s，user time 从 8819.69 s 降到 7536.25 s；SAM mapped、FLAG 分布、Top RNAME 和 sorted multiset 维持原有等价水平。
- 该优化没有达成 SSH2 目标：Rust full 仍为 C++ full 的 1.080 倍，而目标是 `<= C++ / 2`，即本轮 C++ 1050.84 s 下应 `<= 525.42 s`。
- Rust full RSS 为 1,831,660 KiB，低于 C++ 2,539,832 KiB，内存目标方向正确。
- Rust stderr 中核心比对耗时为 956.821642 s，总耗时为 1134.21 s；继续优化必须优先压缩 align core 或提高与读写阶段的重叠，单纯减少 SAM 格式化无法达到 SSH2 目标。

## 未解决项

- SSH2 1M 默认策略与 mode-range 策略均已达速度/RSS 门槛；full SE 证明 mode-range 有收益但未达 SSH2 速度目标。
- 1M sorted multiset 仍有 3 条 `ZP/ZL` 差异，full sorted multiset 仍有 35 条 `ZP/ZL` 差异；源码证据指向 C++ `CCGG_seglen()` 在最后一个 CCGG site 后的末端 fragment 上先解引用再检查边界。Rust 保持安全行为，不伪造该越界式标签。
- reverse cache 会把 RRBS 大样本 RSS 从约 0.87 GiB 提高到约 1.64 GiB，但仍低于 C++ 1M 的约 2.37 GiB；若未来在内存更小机器上运行，可用 `BSMAP_RRBS_MATERIALIZE_REVERSE=0` 强制关闭。
- full 主要瓶颈在 align core。下一步优先评估非 64-bit 对齐 offset 的 mismatch kernel、RRBS SE normal-only index/view、读写与 align pipeline，以及 C++ AddHit 早停触发频率是否完全等价；SAM 语义必须先保持 QNAME/RNAME/POS/FLAG/NM 对齐。
- 现有 `count_mismatch_simd` 未接入 `extend.rs` 热路径，且仅优化 `bit_offset == 0`；直接接入可能收益有限，真正值得评估的是非对齐 offset 的 SIMD 或专用 kernel。

## 2026-06-28：RRBS SE pipeline 默认化与 normal-prefix 回退

保留优化：`8a354d4 perf: enable RRBS SE pipeline by default`。

- 行为：RRBS SE 在未显式增加额外参数时内部使用 depth=2 的 producer/align pipeline；WGBS 与 PE 默认路径不变。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。
- Docker binary SHA256：`48199b5d47ba278e9fa9885798bd083e70e1235c4e6b9ab6578a2c0f6a331afb`。

默认参数 10K/100K/1M 验证路径：`/workspace/benchmark_results/ssh2/20260627T223149Z-9798/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust/C++ wall | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.96 s | 473% | 1,002,924 | 65.94 s | 100% | 2,057,236 | 0.030 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.64 s | 718% | 1,036,792 | 77.03 s | 115% | 2,117,744 | 0.138 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 35.77 s | 579% | 1,856,048 | 97.63 s | 298% | 2,486,212 | 0.366 | sorted multiset 253,099/253,102；仍为 3 条 C++ terminal ZP/ZL 边界差异 |

full opt-in 验证路径：`/workspace/benchmark_results/ssh2/pipeline-full-20260627T220442Z/summary.json`。

| case | wall | user | sys | CPU | RSS KiB | mapped | SAM diff |
|---|---:|---:|---:|---:|---:|---:|---|
| Rust full SE pipeline depth 2 | 926.23 s | 6768.29 s | 157.71 s | 747% | 1,858,392 | 8,873,078 | 与 Rust serial sorted multiset 8,873,078/8,873,078；与 C++ sorted multiset 8,873,043/8,873,078，35 条仍为已知 terminal ZP/ZL 边界差异 |
| Rust full SE serial/mode-range baseline | 1134.39 s | 7536.25 s | 154.93 s | 678% | 1,831,660 | 8,873,078 | baseline |
| C++ full SE | 1050.84 s | 7682.63 s | 218.65 s | 751% | 2,539,832 | 8,873,078 | baseline |

pipeline 结论：

- full wall 从 1134.39 s 降到 926.23 s，提升约 18.3%；RSS 增加约 26,732 KiB，仍明显低于 C++。
- SAM 与 Rust serial 完全一致；与 C++ 的差异未扩大。
- 该优化值得保留，但仍未达到 SSH2 full 目标：当前 C++ full 为 1050.84 s，目标应为 Rust `<= 525.42 s`。

诊断 profiling：

- 运行路径：`/workspace/benchmark_results/ssh2/20260627T223922Z-10085/summary.json`。
- `BSMAP_PROFILE_RRBS=1` 会为每个候选做 atomic 计数，1M wall 从生产路径的 35.77 s 膨胀到 197.48 s；该 run 只用于候选规模诊断，不用于性能对比。
- 1M 计数：`segment_calls=9,713,520`，`raw_bucket_candidates=11,391,198,972`，`logical_bucket_candidates=6,432,817,120`，`mode_matched_candidates=1,989,892,588`，`mismatch_calls=1,967,547,050`，说明 full 主要瓶颈仍是 mismatch 前后候选规模和 mismatch kernel。

撤回候选：`ee524cf perf: keep RRBS normal hits contiguous`，已由 `4804347 Revert "perf: keep RRBS normal hits contiguous"` 撤回。

- v11 index 构建路径：`/workspace/benchmark_results/ssh2/v11-normal-prefix-20260627T234627Z-12557`；standalone index 用时 37.21 s，SHA256 `b1b8a20c90ddb04b95fbd6367698f0e8e04876f840780e0eab694b2f4a495c91`。
- v11 10K/100K/1M 验证路径：`/workspace/benchmark_results/ssh2/20260627T234727Z-12600/summary.json`。
- 1M Rust wall 35.21 s，相对默认 pipeline 35.77 s 仅提升约 1.6%；100K sorted multiset diff 0，但 streaming diff 从 record 1 起发生大面积顺序差异。
- 判定：收益不足且改变 streaming SAM 顺序，不符合 SSH2 对 C++ SAM 对齐的优先级，已撤回。

撤回候选：`aec3d51 perf: reduce best-hit selection allocation`，已由 `7f98490 Revert "perf: reduce best-hit selection allocation"` 撤回。

- 验证路径：`/workspace/benchmark_results/ssh2/20260628T000122Z-13125/summary.json`。
- 10K/100K streaming diff 0、sorted diff 0；1M 仍仅有 3 条已知 C++ terminal ZP/ZL 边界差异。
- 性能：1M Rust wall 35.97 s、RSS 1,855,404 KiB；保留基线为 35.77 s、RSS 1,856,048 KiB。
- 判定：线性去重替换 `HashSet` 没有带来端到端收益，甚至略慢；该路径不是当前 full SE 主要瓶颈，已撤回。

撤回候选：`dcb61c2 perf: use packed hit dedup keys`，已由 `e64aaca Revert "perf: use packed hit dedup keys"` 撤回。

- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过。
- Docker 同步：GitHub fetch 一次出现 GnuTLS 断开，改用本地 Git bundle 通过 SSH stdin 写入 Docker `/tmp` 后同步；Docker checkout 为 `dcb61c2`，repo dirty=false。
- Docker binary SHA256：`eff80c041c76dee2303474321c16551f12b06052fbbdddb35013c518fcd28784`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T002013Z-13830/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.88 s | 482% | 1,002,636 | 66.26 s | 100% | 2,057,220 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.70 s | 709% | 1,036,772 | 77.53 s | 115% | 2,117,760 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 36.44 s | 583% | 1,856,036 | 98.51 s | 295% | 2,487,740 | sorted multiset 253,099/253,102；仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：packed `u64` key + identity hasher 保持 SAM 语义，但 1M wall 从保留基线 35.77 s 退到 36.44 s，RSS 基本不变；说明当前 full SE 不是被默认 tuple `HashSet` hash 成本主导。该候选负收益，已撤回。

撤回候选：`f143c44 perf: add opt-in RRBS normal hit cache`，已由 `a82f138 Revert "perf: add opt-in RRBS normal hit cache"` 撤回。

- 优化内容：在运行时按 `(seed_hash, mode)` 解码并缓存 RRBS normal hits，通过环境变量 `BSMAP_RRBS_NORMAL_HIT_CACHE=1` 启用；不修改 `.bsi` 格式，默认生产路径原本不受影响。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过；新增单测确认 cached normal mode 保持 normal hit 顺序。
- Docker 同步：checkout 为 `f143c44`，repo dirty=false，Rust binary SHA256 为 `5794128601e6125a29dd1285bfbd816cb10fdb6ff5102ff500c21e9012e877f4`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T004435Z-14681/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 2.22 s | 444% | 1,169,964 | 65.87 s | 100% | 2,057,224 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.63 s | 703% | 1,203,824 | 76.06 s | 116% | 2,119,712 | streaming exact 0/24,236；sorted diff 0 |
| 1,000,000 | 34.99 s | 568% | 2,076,696 | 98.30 s | 299% | 2,487,720 | streaming exact 99,825/253,102；sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：相对保留基线 1M `35.77 s / 1,856,048 KiB`，该候选仅提升约 2.2%，低于 SSH2 单项保留门槛；RSS 增加约 220,648 KiB，且 100K/1M streaming SAM 顺序仍不满足严格对齐。该方向不保留。

撤回候选：`5b1e4aa perf: cache per-read N counts`，已由 `95798d8 Revert "perf: cache per-read N counts"` 撤回。

- 优化内容：将 `count_n_in_mask(mask, read_len)` 从每个 segment 重复计算改为每条 read/read-chain 预计算一次。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过。
- Docker 同步：checkout 为 `5b1e4aa`，repo dirty=false，Rust binary SHA256 为 `09b9b024cb20945a139c13eb326146eb525c7c650f2833f551dfa92fc2bb47ea`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T010232Z-15433/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.96 s | 470% | 1,002,816 | 67.20 s | 100% | 2,057,236 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.43 s | 716% | 1,036,744 | 77.62 s | 116% | 2,117,760 | streaming exact 0/24,236；sorted diff 0 |
| 1,000,000 | 35.59 s | 576% | 1,857,736 | 99.51 s | 300% | 2,487,892 | streaming exact 51,032/253,102；sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：相对保留基线 1M `35.77 s / 1,856,048 KiB`，该候选仅提升约 0.5%，低于 SSH2 单项保留门槛；RSS 基本持平。该方向不能解决 full core 859 s 的主瓶颈，已撤回。

撤回候选：`01a5564 perf: split fast mismatch reference window path`，已由 `622c7d9 Revert "perf: split fast mismatch reference window path"` 撤回。

- 优化内容：保持现有 reference-shift 语义不变，只把 `count_mismatch()` 的常规 `bit_offset != 0` 路径拆成无末端越界分支的 slice 访问；极端末端候选仍走旧 fallback。新增非零 offset 单测。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过。
- Docker 同步：checkout 为 `01a5564`，repo dirty=false，Rust binary SHA256 为 `acb2d8759f8381693305766efd3880baf431b8c21a7f38178e275c22052640b1`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T011859Z-16179/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.86 s | 489% | 1,002,936 | 66.36 s | 100% | 2,057,236 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.28 s | 719% | 1,036,772 | 76.31 s | 115% | 2,117,744 | streaming exact 0/24,236；sorted diff 0 |
| 1,000,000 | 35.90 s | 570% | 1,857,704 | 97.54 s | 295% | 2,487,604 | streaming exact 99,907/253,102；sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：该候选未改变 mapped、FLAG/RNAME/NM 分布，1M sorted diff 仍只有既有 3 条 C++ terminal ZP/ZL 差异；但 1M wall 从保留基线 `35.77 s` 退到 `35.90 s`，RSS 也略增。说明当前编译器/CPU 对该分支拆分无端到端收益，已撤回。

撤回候选：`44a7e55 perf: extract seeds only for enabled read chains`，已由 `3272fc5 Revert "perf: extract seeds only for enabled read chains"` 撤回。

- 优化内容：SE 默认 `-n 0` 只会扩展 read_chain 0，因此尝试在 `SingleAlign::run_align()` 中只为启用的 read-chain 提取 seed/mask；保留旧 API 的双链行为，并新增 scratch 清空单测。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过。
- Docker 同步：checkout 为 `44a7e55`，repo dirty=false，Rust binary SHA256 为 `bb0beb55b907943744c793c4b7842781366b1533e5c09a8420aa79ae2c8850c3`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T013109Z-16808/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.86 s | 488% | 1,002,916 | 66.30 s | 100% | 2,057,240 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.62 s | 711% | 1,036,732 | 75.93 s | 115% | 2,117,764 | streaming exact 0/24,236；sorted diff 0 |
| 1,000,000 | 36.21 s | 579% | 1,857,664 | 98.18 s | 296% | 2,487,708 | streaming exact 75,678/253,102；sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：该候选保持 mapped 数和 sorted SAM 语义，但 1M wall 从保留基线 `35.77 s` 退到 `36.21 s`，RSS 略增。说明 full core 的主成本不在禁用链 seed 提取的固定开销上，已撤回。

撤回候选：`9a33a7b perf: use popcount for xm64 mismatch counting`，已由 `cce4f2b Revert "perf: use popcount for xm64 mismatch counting"` 撤回。

- 优化内容：将 `xm64()` 从 C++ 风格 SWAR byte-sum 改为 `((tt | tt >> 1) & 0x5555...).count_ones()`，尝试让编译器/CPU 对 mismatch 计数使用 popcount 路径。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 均通过。
- Docker 同步：checkout 为 `9a33a7b`，repo dirty=false，Rust binary SHA256 为 `fb091416a59cfe3decec918ed6f8222d627754fa197e81ad3164456c0bb5dc91`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T014304Z-17440/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.97 s | 480% | 1,002,904 | 66.82 s | 100% | 2,057,220 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.68 s | 715% | 1,036,768 | 76.18 s | 115% | 2,117,748 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 36.79 s | 583% | 1,857,684 | 97.79 s | 297% | 2,487,100 | streaming exact 100,591/253,102；sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：SAM 语义保持，但 1M wall 从保留基线 `35.77 s` 退到 `36.79 s`。当前默认 release 构建下，`count_ones()` 形态比手写 SWAR 更慢，已撤回。

构建候选：`RUSTFLAGS="-C target-cpu=native"`，不改源码，作为显式本机生产构建方式记录。

- Docker checkout：`3a18390ff04e0bd4e36410f7e484b76094d40159`，repo dirty=false。
- native Rust binary SHA256：`c80f886d5703b93eadf50e1229cd66b926d1660a4572f7034e58a9f66545e0e6`。
- 验证后已恢复 portable release binary SHA256：`48199b5d47ba278e9fa9885798bd083e70e1235c4e6b9ab6578a2c0f6a331afb`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T015428Z-20368/summary.json`。

| limit | Rust native wall | Rust native CPU | Rust native RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.77 s | 459% | 1,002,740 | 65.61 s | 100% | 2,057,220 | streaming diff 0；sorted diff 0 |
| 100,000 | 8.80 s | 708% | 1,036,800 | 76.45 s | 115% | 2,117,748 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 34.18 s | 585% | 1,856,024 | 97.99 s | 296% | 2,486,804 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：相对 portable 保留基线 1M `35.77 s / 1,856,048 KiB`，native build 降到 `34.18 s / 1,856,024 KiB`，1M wall 提升约 4.4%，RSS 基本持平；100K 也有明显收益。
- 限制：该方式绑定当前服务器 CPU，不应静默替代 portable release，也不能单独解决 full 目标。按 1M 比例估算，full Rust 仍远高于 `C++ full / 2`，后续仍需从每候选 mismatch kernel、批处理或 pipeline 并行模型继续寻找大收益。

诊断：C++ RRBS 候选规模插桩。

- 目的：确认 Rust full 慢点是否来自比 C++ 多扫候选。
- 方法：仅在 Docker `/tmp` 中复制 C++ BSMAP 2.90 源码，临时加入 RRBS raw/mode/mismatch/AddHit 计数器；为适配当前 g++ 11，临时把 C++ `main.cpp` 全局变量 `ref` 改名，未改仓库源码和正式 C++ binary。
- 运行路径：`/workspace/benchmark_results/ssh2/20260628T020344Z-21229/summary.json`。
- 临时 C++ profile binary SHA256：`75910344186240f7a6fc9275f9cf04ac3d9eaf9d6e1741eec310dd16cc653491`。
- 参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1 -E 100000`。

| 指标 | Rust profile 100K | C++ profile 100K | 判定 |
|---|---:|---:|---|
| raw/logical candidates | Rust raw `1,033,543,412`；Rust logical `590,799,207` | C++ raw `590,799,207` | Rust raw 包含 BSC/cross-chain 物理项；实际 SE logical bucket 与 C++ 一致 |
| mode candidates | `193,812,236` | `191,939,381` | Rust profile 计数点略早；进入 mismatch 的数量见下一行 |
| mismatch calls | `191,939,381` | `191,939,381` | 完全一致 |
| accepted hits | `1,357,106` | `1,357,106` | 完全一致 |
| SAM | 24,236 mapped；streaming diff 0；sorted diff 0 | 24,236 mapped | 完全一致 |

- 结论：当前 Rust RRBS SE 与 C++ 在 100K 上进入 mismatch 的候选数和 accepted hit 数已经等价。继续通过“少扫候选”拿收益的空间很小，除非有新的 C++ 源码证据；后续主攻方向应转为每候选成本、mismatch kernel 批处理、内存访问和并行流水线。

撤回候选：query-shift mismatch fast path，测试阶段即撤回，未提交。

- 优化内容：尝试把 RRBS `count_mismatch()` 从当前 Rust reference-shift 形式改成 C++ `CountMismatch()` 风格的 query/mask shift，避免每个候选重组 reference window。
- 本地定向测试：新增的新旧算法等价测试在 `read_len=33`、`reference_start=2`、`threshold=100` 下失败，新函数返回 25，现有 reference-shift oracle 返回 24。
- 判定：当前 Rust 的 `xc64` 容忍掩码、word 边界和 padding 语义不能被简单改写为 query-shift。该方向没有通过本地 correctness gate，已立即撤回，未进入 Docker benchmark。

pipeline depth probe：不改源码，只测试显式 `--pipeline-depth 4/8`。

- 目的：判断 RRBS SE 默认 depth=2 之后继续加深 pipeline 是否还有低风险收益。
- Rust binary SHA256：`48199b5d47ba278e9fa9885798bd083e70e1235c4e6b9ab6578a2c0f6a331afb`。
- 运行路径：`/workspace/benchmark_results/ssh2/pipeline-depth-probe-20260628T021507Z-21614`。
- 对照：复用同参数 C++ 1M SAM `/workspace/benchmark_results/ssh2/20260628T015428Z-20368/case_cpp_se_1000000/output.sam` 做 streaming/sorted diff。

| depth | Rust wall | Rust user | Rust sys | Rust CPU | Rust RSS KiB | mapped | sorted diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 4 | 35.92 s | 201.31 s | 5.99 s | 577% | 1,866,508 | 253,102 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL |
| 8 | 35.30 s | 197.20 s | 6.22 s | 576% | 1,887,492 | 253,102 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL |

- 判定：相对默认 depth=2 的 1M `35.77 s / 1,856,048 KiB`，depth=4 退化，depth=8 仅约 1.3% 小幅提升且 RSS 增加约 31 MiB；低于 SSH2 保留门槛。继续加深 pipeline 不是当前主收益方向。

诊断提交：`b93cfe3 profile: add RRBS pipeline stage timing`。

- 目的：补齐 RRBS SE pipeline 路径的低开销 stage profile。此前串行路径已有 `read/prepare/align/write` 阶段耗时，但默认保留的 depth=2 pipeline 只记录 alignment core，无法判断 full 缩放慢点是否来自读写重叠不足。
- 代码范围：只在 `SinglePreparedBatch` 中携带 producer 侧 `read_time` 和 `prepare_time`，consumer 侧汇总 `write_time`；仅在 `BSMAP_PROFILE_RRBS=stage` 且 RRBS 时输出，不改变默认生产路径和 SAM 语义。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。
- Docker 同步：checkout 为 `b93cfe3`，repo dirty=false，Rust binary SHA256 为 `2c0afe7204b5ca423935cdcbef03cf5e2830e4fdc517f257a3bd2a204b7f5488`。
- 运行路径：`/workspace/benchmark_results/ssh2/20260628T022458Z-21945/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | Rust stage seconds | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|---|
| 1,000,000 | 36.07 s | 580% | 1,855,968 | 97.87 s | 295% | 2,486,468 | read 2.55；prepare 1.18；align 25.46；write 1.51 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL |

- 判定：stage profile 本身保持 mapped、FLAG/RNAME/NM 分布和既有 sorted SAM 差异水平；性能与默认 depth=2 基线接近，诊断开销可接受。
- 关键结论：1M 中可见的 producer/read、prepare 和 SAM write 合计约 5.25 s，远小于 align core 25.46 s；继续优化 pipeline depth、FASTQ 读取或 SAM 写出，即使全部清零也不足以达到 full `<= C++ / 2`。SSH2 下一步应集中在每候选 mismatch kernel、批量化计算、reference/index 访问局部性或更大粒度的并行调度，而不是继续做浅层 pipeline 调参。

full stage profile 补充运行：

- 运行路径：`/workspace/benchmark_results/ssh2/rust-full-stage-20260628T024823Z-22811/summary.json`。
- Docker checkout：`64c9c668b9940fcc0d0eff8efeb082ca6af138f6`，repo dirty=false。
- Rust binary SHA256：`2c0afe7204b5ca423935cdcbef03cf5e2830e4fdc517f257a3bd2a204b7f5488`。
- Reference SHA256：`db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7`。
- Reads SHA256：`a00aacbc7841f3243c2cf273d627944c2ee607b5310d90906169bc8392573172`。
- Rust warm align 不包含 standalone index；run 前后 index SHA256 均为 `1329966ddda5aedd9fc7e13cb84a4e755cd632df3d14a0de32a239a29561e634`。

| case | wall | user | sys | CPU | RSS KiB | mapped | stage seconds | 判定 |
|---|---:|---:|---:|---:|---:|---:|---|---|
| Rust full SE stage | 933.61 s | 6803.96 s | 161.59 s | 746% | 1,858,428 | 8,873,078 | read 86.18；prepare 41.19；align 863.54；write 51.50 | 与保留 pipeline full 926.23 s 接近；诊断开销可接受 |

- full stage 结论：full 数据上 align core 仍占绝对主导，`read + prepare + write` 合计约 178.87 s，align core 为 863.54 s。即使理想化清零读、prepare 和 SAM write，Rust 仍会高于 SSH2 目标 `<= 525.42 s`。
- 因此，SSH2 后续不再把浅层 pipeline、FASTQ 解压或 SAM writer 作为主线；除非有新 profile 反证，主攻方向应转为降低每候选 mismatch 成本、批量化候选处理、改善 reference/index 随机访问局部性，或改变并行调度粒度。

撤回候选：`87f0c49 perf: force inline mismatch counter`，已由 `2cc6c37 Revert "perf: force inline mismatch counter"` 撤回。

- 优化内容：把 `count_mismatch()` 从普通 `#[inline]` 改为 `#[inline(always)]`，尝试降低约 19.7 亿次 1M mismatch 调用的函数边界成本；不改算法和输出逻辑。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。第一次补丁误叠加 `#[inline]` 与 `#[inline(always)]` 产生新增 warning，已在测试前修正为单一属性。
- Docker 同步：checkout 为 `87f0c49`，repo dirty=false，Rust binary SHA256 为 `9cf3079eb16e8b1ff0c7c3da8dc729293506065c3baac2613944dfab7c76f950`。
- 验证路径：`/workspace/benchmark_results/ssh2/20260628T023417Z-22324/summary.json`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.93 s | 481% | 1,002,884 | 66.62 s | 100% | 2,057,240 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.42 s | 717% | 1,036,788 | 75.84 s | 115% | 2,117,760 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 35.18 s | 576% | 1,856,020 | 96.65 s | 295% | 2,486,916 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL |

- 判定：SAM 语义保持，RSS 基本持平；但相对保留基线 1M `35.77 s / 1,856,048 KiB` 只提升约 1.6%，低于 SSH2 单项保留门槛且可能属于 run-to-run 波动。该候选不保留。

撤回候选：`292aaac perf: align packed RRBS hit storage`，已由 `2eb736b Revert "perf: align packed RRBS hit storage"` 撤回。

- 优化内容：将新建 RRBS index 从 v10 七字节 packed hit 改为 v11 八字节对齐 packed hit，尝试用约 49 MB 的 `.bsi` 增量换取更低的 mmap 解码和 stride 成本；不改变 hit 顺序、mode 编码或 SAM 语义。
- 本地验证：`cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过；新增单测覆盖 v11 aligned hit roundtrip 和 index mmap/memory roundtrip。
- v11 standalone index：`/workspace/benchmark_results/ssh2/v11-aligned-index-20260628T031610Z-23495`，构建 wall 37.96 s，RSS 1,276,372 KiB，索引 SHA256 `99d2861c480730e5a3970c4f4efc5fce429266581405f7b78fafd5225c34c4b7`，大小 1,091,007,832 bytes。
- Docker 验证路径：`/workspace/benchmark_results/ssh2/20260628T031711Z-23535/summary.json`；checkout 为 `292aaac`，repo dirty=false，Rust binary SHA256 `fce20e69087acadc269e7c3e6a96f609646d18bd8814c58babe8769f5c0f0795`。

| limit | Rust wall | Rust CPU | Rust RSS KiB | C++ wall | C++ CPU | C++ RSS KiB | SAM diff |
|---:|---:|---:|---:|---:|---:|---:|---|
| 10,000 | 1.93 s | 480% | 1,050,820 | 66.16 s | 100% | 2,057,224 | streaming diff 0；sorted diff 0 |
| 100,000 | 10.62 s | 709% | 1,084,452 | 75.33 s | 115% | 2,117,748 | streaming diff 0；sorted diff 0 |
| 1,000,000 | 36.06 s | 578% | 1,903,960 | 97.72 s | 295% | 2,487,008 | sorted exact 253,099/253,102，仍为 3 条 C++ terminal ZP/ZL 边界差异 |

- 判定：该候选保持 10K/100K 完全一致，1M sorted SAM 仍只剩既有 3 条 C++ terminal ZP/ZL 差异；但相对保留基线 1M `35.77 s / 1,856,048 KiB`，wall 退化到 `36.06 s`，RSS 增加约 47,912 KiB。说明当前瓶颈不在 v10 七字节 hit 解码或非 2 的幂 stride；该方向不保留。
