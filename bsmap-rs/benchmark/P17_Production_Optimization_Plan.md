# P17 C++ 等价优先的生产级速度与内存优化计划

## Summary

P17 从 `29daa8f` 开始，目标是在保持 C++ BSMAP 2.90 语义等价的前提下，继续压低 Rust 版的 wall time、RSS 和 CPU 浪费。本轮不跑 WGBS 90G / RRBS 10G 规模测试，只跑 WGBS example1、WGBS example2、mm10 RRBS 10K；大样本收益只写为估算。

Rust standalone index 是可复用步骤，继续单独计时，不并入 Rust/C++ 单样本 warm align 对比。

## 优化步骤与收益估算

| 阶段 | 内容 | 实施状态 | wall time 估算 | RSS 估算 | CPU 利用率估算 | 置信度 |
|---|---|---|---:|---:|---:|---|
| Phase 1 | PE interleaved pairing，对齐 C++ `RunAlign/GetPairs` 调度 | 暂缓，需 test-only fixture 先证明中间语义 | PE 降低 10% 到 30% | 降低 5% 到 15% | 提升 5 到 15 个百分点 | 中高 |
| Phase 2 | PE/SAM direct writer，减少 pair/unpair 输出分配 | 已验证并撤回，短基准未达保留标准 | PE 降低 3% 到 8% | 降低 0% 到 3% | sys time 小幅下降 | 高 |
| Phase 3 | bounded read-align-write pipeline 原型 | 仅保留为后续候选，本轮不默认启用 | 短基准 0% 到 5%；大样本估算 5% 到 20% | 增加约 1 个 batch | 提升 5 到 25 个百分点 | 中 |
| Phase 4 | SIMD mismatch runtime dispatch | 先 microbench，未证实前不接入 | SE/RRBS 降低 3% 到 12%；也可能 0% | 基本不变 | 小幅提升或不变 | 中低 |
| Phase 5 | mmap/index 低风险审查 | v10 格式默认不改 | 降低 0% 到 5% | 降低 0% 到 8% | 基本不变 | 中低 |

综合收益不得线性相加。P17 已证明 PE/SAM direct writer 在当前短基准下不能作为默认优化保留；完整 PE interleaved pairing 只有在中间 fixture 与短基准都达标后才进入生产代码。

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
- 默认保留条件：四个短基准无明确回归，并且至少一个目标 workload wall time 提升不少于 3%，或 RSS/CPU 改善不少于 5%。
- C++ 等价必需改动可以在性能中性时保留，但必须在报告中说明原因。
- 不跑 WGBS 90G / RRBS 10G；所有大样本收益只能写为估算。
