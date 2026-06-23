# P16 工程化优化计划

## 目标

P16 在 P15 已完成 C++ 等价和索引压缩的基础上，继续寻找低风险的速度、内存和 CPU 利用率优化点。优化必须保持 P13/P15 的结果等价，不以牺牲 RRBS 或 WGBS 任一主路径换取局部收益。

## 范围

- 保持 Rust standalone index 单独计时，不计入 Rust/C++ 单样本 align 时间。
- 不做 RRBS 10G 或 WGBS 90G 长测。
- 使用本地 WSL2 和 `D:/BSMAP/benchmark-data/mm10` 的 mm10 10K 数据。
- 与 P15 基线 `93e322d` 使用同一脚本、同一机器、同一输入进行对比。

## 候选优化

1. 非 TTY progress bar 隐藏
   - benchmark 和批处理场景 stderr 通常重定向到文件。
   - stderr 非 TTY 时使用 `ProgressBar::hidden()`，交互终端仍显示进度。

2. release profile 调参
   - 测试 ThinLTO、`codegen-units=1`、`panic=abort`。
   - 只有 WGBS/RRBS 同时不回退才保留。

3. PE pairing 临时分配优化
   - 预拆 read-chain、减少 `HashMap` 分组和重复临时对象。
   - 必须通过同环境 benchmark；负收益立即撤回。

4. 输出路径低分配重构
   - 后续 P17 候选。
   - 需要 direct SAM writer，而不是只把已生成 `String` 改写入方式。

## 验收标准

- `cargo check -p bsmap`
- `cargo test -p bsmap`
- `cargo build --release -p bsmap`
- `python3 -m py_compile benchmark/p15/*.py`
- `python3 -m unittest benchmark/p15/test_tools.py`
- WGBS example1 Rust vs C++ 非 header SAM 完整逐行一致，`RNAME/POS/FLAG/NM/ZP/ZL` 差异为 0。
- WGBS example2 Rust 正常输出；C++ PE 若继续 signal 6/134，如实记录。
- mm10 RRBS SE 10K Rust vs C++ 2,423 条完整逐行一致。
- mm10 RRBS PE 10K Rust 输出 4,884 条，染色体分布不得回退到 chr1 偏斜。
- 任一候选若 WGBS 或 RRBS 明显回退，不保留为默认优化。
