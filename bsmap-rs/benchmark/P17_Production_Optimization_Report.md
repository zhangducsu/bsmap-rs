# P17 生产级速度与内存优化报告

## 当前结论

P17 已完成一轮生产工程化审查和短基准验证。最终没有保留新的比对或输出代码优化；本轮保留的是 P17 可复现短基准入口、summary 对比工具、计划文档和评估报告。

PE/SAM direct writer 候选已实作并验证，但短基准没有稳定达到保留标准：WGBS PE 与 RRBS PE 没有稳定提速，且 WGBS PE 出现过 4% 到 10% 的 wall time 回退。因此该候选已撤回，避免把噪声或局部收益带入生产分支。

Phase 1 的完整 PE interleaved pairing 仍是下一步最大收益点，但暂未进入生产代码。原因是它需要拆分 `SingleAlign` 内部阶段并建立 C++ `RunAlign/GetPairs` 中间 fixture；在缺少中间等价保护前直接切换风险过高。

## 保留改动

- `benchmark/p17/`
  - 新增 P17 短基准入口，复用 P16 的 example1/example2/mm10 RRBS 10K 验证矩阵。
  - 新增 summary 对比工具，明确标记规模收益为估算。

- `benchmark/P17_Production_Optimization_Plan.md`
  - 固定 P17 的候选顺序、收益估算、验证矩阵和保留标准。

- `benchmark/P17_Production_Optimization_Report.md`
  - 记录本轮实测结果、撤回项和后续可继续项。

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

短基准结果目录：

- 第一版 PE/SAM direct writer：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-20260623T080000Z`
- 第一版复测：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-20260623T081000Z`
- 完整 SEQ/QUAL direct writer：`D:/BSMAP/benchmark-results/p17/pe-sam-direct-full-20260623T082000Z`

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

上述百分比均相对 P16 baseline `D:/BSMAP/benchmark-results/p16/sam-direct-warm-20260623T072000Z` 的 summary。Rust standalone index 独立计时，不并入 warm align 对比。

## 未解决项

- PE interleaved pairing 仍是 P17 后续最大收益点，但必须先增加 test-only C++ 中间结果 fixture。
- PE/SAM direct writer 在 10K 短基准未达标，暂不保留。若后续要重试，必须先用 profiler 证明输出分配是 PE 主瓶颈，而不是 pairing、I/O 或 batch 调度。
- 本轮不跑 WGBS 90G / RRBS 10G；报告中的大样本收益均为估算，不作为实测结论。
