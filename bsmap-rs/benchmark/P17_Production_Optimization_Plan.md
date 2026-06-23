# P17 C++ 等价优先的生产级速度与内存优化计划

## 状态

P17 已于 2026-06-24 停止继续优化并收尾。本文件保留为历史计划、候选清单和收益估算记录，不再作为当前执行计划。

后续若继续追求生产性能，应新开 P18 或新的明确阶段：先建立稳定 A/B benchmark 和 profiler 证据，再选择单个高收益路径重构。不得把 P17 中已撤回或未实作的候选直接视为待合入生产优化。

## Summary

P17 从 `29daa8f` 开始，目标是在保持 C++ BSMAP 2.90 语义等价的前提下，继续压低 Rust 版的 wall time、RSS 和 CPU 浪费。本轮不跑 WGBS 90G / RRBS 10G 规模测试，只跑 WGBS example1、WGBS example2、mm10 RRBS 10K；大样本收益只写为估算。收尾结论是：低风险微优化未产生稳定收益，P17 不再继续执行。

Rust standalone index 是可复用步骤，继续单独计时，不并入 Rust/C++ 单样本 warm align 对比。

## 优化步骤与收益估算

| 阶段 | 内容 | 实施状态 | wall time 估算 | RSS 估算 | CPU 利用率估算 | 置信度 |
|---|---|---|---:|---:|---:|---|
| Phase 1 | PE interleaved pairing，对齐 C++ `RunAlign/GetPairs` 调度 | 已补 test-only 调度顺序护栏；单独重排最终 pair 循环已撤回，完整生产重构仍需中间候选 fixture | PE 降低 10% 到 30% | 降低 5% 到 15% | 提升 5 到 15 个百分点 | 中高 |
| Phase 2 | PE/SAM direct writer，减少 pair/unpair 输出分配 | 已验证并撤回，短基准未达保留标准 | PE 降低 3% 到 8% | 降低 0% 到 3% | sys time 小幅下降 | 高 |
| Phase 3 | bounded read-align-write pipeline 原型 | 仅保留为后续候选，本轮不默认启用 | 短基准 0% 到 5%；大样本估算 5% 到 20% | 增加约 1 个 batch | 提升 5 到 25 个百分点 | 中 |
| Phase 4 | SIMD mismatch runtime dispatch | test-only probe 已完成，结果混合，默认不接入 | SE/RRBS 降低 3% 到 12%；也可能 0% | 基本不变 | 小幅提升或不变 | 中低 |
| Phase 5 | mmap/index 低风险审查 | v10 格式默认不改 | 降低 0% 到 5% | 降低 0% 到 8% | 基本不变 | 中低 |

综合收益不得线性相加。P17 已证明 PE/SAM direct writer 在当前短基准下不能作为默认优化保留；完整 PE interleaved pairing 只有在中间 fixture 与短基准都达标后才进入生产代码。

截至收尾，P17 未保留新的生产比对或输出代码优化。保留下来的内容是 benchmark 入口、summary 稳定性检查、test-only 语义护栏、SIMD probe 和评估文档。

## 验证矩阵

每个保留提交必须通过：

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
python3 -m py_compile benchmark/p15/*.py benchmark/p17/*.py
python3 -m unittest benchmark/p15/test_tools.py
bash benchmark/p17/run_short_validation.sh . /mnt/d/BSMAP/benchmark-results/p17/<run-id>
```

端到端门槛：

- WGBS example1：Rust/C++ 66,120 mapped；完整 SAM 记录 100% 一致；`RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。
- WGBS example2：Rust 输出记录数和 SHA 相对 P16 golden 不变；C++ PE 若继续 signal 6/134，如实记录。
- mm10 RRBS SE 10K：Rust/C++ 2,423 条完整一致；`RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。
- mm10 RRBS PE 10K：Rust 4,884 records，Top chr1 不回退，SAM SHA 相对 P16/P15 golden 稳定。

## 保留标准

- 所有实测结果必须与 P16 `29daa8f` 在同机器、同路径、同参数下比较。
- `benchmark/p17/summarize_short_validation.py` 必须检查 `benchmark_stability`；若 C++ control 的 wall drift 超过 10%，该 run 的性能百分比不能单独作为保留或撤回生产代码的依据。
- 默认保留条件：四个短基准无明确回归，并且至少一个目标 workload wall time 提升不少于 3%，或 RSS/CPU 改善不少于 5%。
- C++ 等价必需改动可以在性能中性时保留，但必须在报告中说明原因。
- 不跑 WGBS 90G / RRBS 10G；所有大样本收益只能写为估算。
