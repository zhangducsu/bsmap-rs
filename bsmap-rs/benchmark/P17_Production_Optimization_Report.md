# P17 生产级速度与内存优化报告

## 收尾状态

P17 于 2026-06-24 按决策停止继续优化并收尾。本轮不再追加 PE interleaved pairing、pipeline、SIMD 或 mmap/index 生产代码实验；当前分支只保留已验证的测试护栏、benchmark 工具和评估文档。

后续若继续追求极致性能，应新开 P18，并先用稳定 A/B runner 与 profiler 证明单个热点具备足够收益，再进入重构。P17 不再作为继续挖掘低风险微优化的工作分支。

## 当前结论

P17 已完成一轮生产工程化审查和短基准验证。最终没有保留新的比对或输出代码优化；本轮保留的是 P17 可复现短基准入口、summary 对比工具、计划文档和评估报告。

PE/SAM direct writer 候选已实作并验证，但短基准没有稳定达到保留标准：WGBS PE 与 RRBS PE 没有稳定提速，且 WGBS PE 出现过 4% 到 10% 的 wall time 回退。因此该候选已撤回，避免把噪声或局部收益带入生产分支。

Phase 1 的完整 PE interleaved pairing 仍是未来最大潜在收益点，但不再在 P17 内继续。原因是它需要拆分 `SingleAlign` 内部阶段并建立 C++ `RunAlign/GetPairs` 中间 fixture；在缺少中间等价保护前直接切换风险过高。

## 保留改动

- `benchmark/p17/`
  - 新增 P17 短基准入口，复用 P16 的 example1/example2/mm10 RRBS 10K 验证矩阵。
  - 新增 summary 对比工具，明确标记规模收益为估算。
  - 新增 `benchmark_stability` 字段：用 C++ control 的 wall drift 判断本轮短基准是否受环境噪声污染。

- `benchmark/P17_Production_Optimization_Plan.md`
  - 固定 P17 的候选顺序、收益估算、验证矩阵和保留标准。

- `benchmark/P17_Production_Optimization_Report.md`
  - 记录本轮实测结果、撤回项和后续可继续项。

- `bsmap/src/align/mismatch.rs`
  - 新增 ignored 的 P17 mismatch scalar/SIMD microbench probe。
  - 默认 `cargo test` 不运行该 probe；只用于手动评估是否值得把 SIMD 接入热路径。

- `bsmap/src/pairs/pair.rs`
  - 新增 test-only 的 C++ `pairs.cpp::RunAlign()` 配对 mismatch-level 调度顺序护栏。
  - 当前只锁定 `(i,i)`、`(i,j)`、`(j,i)` 的层级顺序和非对称 read 最大 mismatch 边界；不改变生产 PE 行为。

## 收益估算

| 优化 | 主要场景 | wall time 估算 | RSS 估算 | CPU 利用率估算 | 置信度 |
|---|---|---:|---:|---:|---|
| PE/SAM direct writer | SAM 输出占比高的 PE | PE 降低 3% 到 8% | 降低 0% 到 3% | sys time 小幅下降 | 高 |
| PE interleaved pairing | WGBS PE、RRBS PE | PE 降低 10% 到 30% | 降低 5% 到 15% | 提升 5 到 15 个百分点 | 中高，未实现 |
| bounded pipeline | 大 FASTQ、gzip、慢磁盘、SAM 文件输出 | 短基准 0% 到 5%；大样本估算 5% 到 20% | 增加约 1 个 batch | 提升 5 到 25 个百分点 | 中，未实现 |
| SIMD mismatch | mismatch 热路径明显的 SE/PE | 降低 3% 到 12%；也可能 0% | 基本不变 | 小幅提升或不变 | 中低，未实现 |
| mmap/index 微优化 | RRBS mmap 大索引 | 降低 0% 到 5% | 降低 0% 到 8% | 基本不变 | 中低，未实现 |

## 验证状态

已通过：

- `cargo check -p bsmap`
- `cargo test -p bsmap`
- `cargo build --release -p bsmap`
- `python3 -m py_compile benchmark/p15/*.py benchmark/p17/*.py`
- `python3 -m unittest benchmark/p15/test_tools.py`
- `cargo test -p bsmap --lib bench_count_mismatch_scalar_vs_simd_p17_probe --release -- --ignored --nocapture`
- `cargo test -p bsmap cpp_pair_level_schedule`

短基准结果目录：

- 第一版 PE/SAM direct writer：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-20260623T080000Z`
- 第一版复测：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-20260623T081000Z`
- 完整 SEQ/QUAL direct writer：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-full-20260623T082000Z`
- C++ pair-level order 生产候选：`D:/BSMAP/benchmark-results/p17/cpp-pair-order-20260623T093000Z`
- `group_hits_by_chr` Vec 分组候选：`D:/BSMAP/benchmark-results/p17/group-hits-vec-20260623T101000Z`
- read-chain split 缓存候选：`D:/BSMAP/benchmark-results/p17/cached-read-chain-splits-20260623T113000Z`

正确性结果：

| 场景 | 结果 |
|---|---|
| WGBS example1 | Rust/C++ 66,120 records；完整 SAM 记录 100% 一致；`RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0 |
| WGBS example2 | Rust 正常输出；C++ PE 既有失败按 runner 记录 |
| mm10 RRBS SE 10K | Rust/C++ 2,423 records；完整 SAM 记录 100% 一致；`RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0 |
| mm10 RRBS PE 10K | Rust 4,884 records；Top chr1 未回退到异常偏斜 |

候选性能结果：

| 候选 | 运行 | WGBS example2 Rust wall | mm10 RRBS PE Rust wall | 结论 |
|---|---|---:|---:|---|
| PE/SAM direct writer 第一版 | `20260623T080000Z` | +9.83% | -4.69% | WGBS PE 明显回退，不能保留 |
| PE/SAM direct writer 第一版复测 | `20260623T081000Z` | +1.73% | +1.43% | 无稳定收益 |
| 完整 SEQ/QUAL direct writer | `20260623T082000Z` | +4.62% | +0.82% | PE 仍无收益，已撤回 |
| C++ pair-level order 生产候选 | `20260623T093000Z` | +41.62% | +29.80% | 改变 example2/rrbs_pe 记录 SHA 且明显变慢，已撤回 |
| `group_hits_by_chr` Vec 分组候选 | `20260623T101000Z` | +42.77% | +32.35% | PE 记录 SHA 与 P16 一致，但 wall time 明显回退，已撤回 |
| read-chain split 缓存候选 | `20260623T113000Z` | +41.62% | +31.43% | PE 记录 SHA 与 P16 一致，但 wall time 明显回退，已撤回 |

上述百分比均相对 P16 baseline `D:/BSMAP/benchmark-results/p16/sam-direct-warm-20260623T072000Z` 的 summary。Rust standalone index 独立计时，不并入 warm align 对比。

P17 summary 现在会输出 `benchmark_stability`。若任一 C++ control 的 `cpp_time.wall_pct` 绝对值超过 10%，该 run 标记为 `unstable=true`，性能百分比只能作为噪声提示，不能单独作为保留或撤回生产代码的证据。对 `cached-read-chain-splits-20260623T113000Z` 重新汇总后，C++ control 最大漂移为 35.13%，说明该 run 的性能 delta 受环境漂移显著影响；代码撤回仍按保守策略处理，但后续 P17 候选应优先采用同轮 back-to-back baseline 或多轮中位数。

SIMD mismatch probe 结果：

| case | len | offset | threshold | scalar ns/iter | simd ns/iter | simd/scalar |
|---|---:|---:|---:|---:|---:|---:|
| `se75_full_scan` | 75 | 0 | 100 | 23.301 | 27.334 | 1.1731 |
| `se75_early_abort` | 75 | 0 | 2 | 10.423 | 10.654 | 1.0222 |
| `pe150_full_scan` | 150 | 0 | 100 | 31.714 | 24.536 | 0.7737 |
| `pe150_offset` | 150 | 6 | 100 | 13.009 | 16.226 | 1.2473 |

AVX2 可用，但结果混合：只有 150bp、offset=0 的 full scan 明显受益，75bp 和 offset case 均变慢。因此 P17 不把 `count_mismatch_simd()` 接入默认 `extend.rs` 热路径；后续若要继续，必须先按真实候选分布统计 offset 与 read length 占比。

## 收尾后遗留项

- PE interleaved pairing 仍是后续新阶段的最大潜在收益点；当前已增加 C++ 调度顺序 test-only 护栏。实测证明，仅把现有全量配对结果改成 C++ pair-level 枚举顺序会改变 PE 输出 SHA 并造成短基准回退，因此完整生产重构必须先拆分 `SingleAlign` 阶段、补齐中间候选 fixture，并同步处理 C++ 早停语义，不能只重排最终配对循环。
- PE/SAM direct writer 在 10K 短基准未达标，暂不保留。若后续要重试，必须先用 profiler 证明输出分配是 PE 主瓶颈，而不是 pairing、I/O 或 batch 调度。
- SIMD mismatch probe 结果不支持默认接入。下一步应先统计真实 alignment 热路径中 read length、offset 和 early abort 的分布，而不是直接切换实现。
- 本轮不跑 WGBS 90G / RRBS 10G；报告中的大样本收益均为估算，不作为实测结论。

## 最终决策

- P17 停止继续生产优化，不再追加新的性能实验。
- 不保留任何已撤回候选的生产代码。
- 保留 P17 benchmark、summary 稳定性检查、test-only 语义护栏和本报告。
- 后续优化只有在 profiler 和稳定 A/B benchmark 证明收益足够时，才以 P18 或新的明确阶段继续。
