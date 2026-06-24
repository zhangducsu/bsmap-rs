# S1 激进优化阶段报告

## 1. 本轮结论

S1 本轮完成了第一批可保留优化和基准框架：

- 保留：`RawRead` batch 外层 `Vec` 复用，避免每批通过 `mem::take()` 丢弃容量。
- 保留：WGBS 可选 bounded producer pipeline，新增 `--pipeline-depth <INT>`。
- 保守处理：默认 `pipeline-depth=1` 仍走原串行路径；只有显式 `--pipeline-depth > 1` 且非 RRBS 时启用 pipeline。
- 保守处理：RRBS 暂不启用 pipeline。早期短基准显示 RRBS pipeline 版本存在轻微回退风险，最终实现强制 RRBS 回到串行路径。
- 新增：`benchmark/s1/run_short_pipeline_benchmark.sh` 和 `benchmark/s1/summarize_short_pipeline.py`，用于同轮 back-to-back 比较 baseline/current。

S1 的长程 5G WGBS/RRBS 测试本轮未执行，因此不能宣称 pipeline 已达到 S1 计划中的大样本默认启用标准。当前交付是安全的 Phase 1 框架与短基准验证，不是完整 S1 架构重构终点。

Rust standalone index 继续单独计时，不并入 Rust/C++ 或 baseline/current 的单样本 align wall time 比较。

## 2. 代码变更

### 2.1 Batch 容器复用

新增 `process_batch_reuse(&mut Vec<RawRead>, ...)`：

- 复用读取批次的外层 `Vec<RawRead>` 容量。
- `process_batch()` 保留原接口，并委托给共享 helper。
- 增加单元测试确认复用路径不会丢失外层容量。

### 2.2 可选 WGBS pipeline

新增 `--pipeline-depth <INT>`：

- `1`：默认值，保持串行行为。
- `>1`：仅 WGBS 启用 producer -> aligner 的 bounded queue。
- RRBS：无论 depth 参数是多少，本轮都保留串行路径，避免引入 RRBS 短基准回退。

当前 pipeline 只重叠 read/process/encode 与 align 阶段，writer 仍由主线程顺序输出。这样能保证输出顺序稳定，也把风险控制在最小范围。

### 2.3 Benchmark 工具

新增短基准 runner：

```bash
bash bsmap-rs/benchmark/s1/run_short_pipeline_benchmark.sh \
  <repo-root> <baseline-bsmap-binary> <run-root>
```

runner 覆盖：

- mm10 WGBS SE 10K
- mm10 WGBS PE 10K
- mm10 RRBS SE 10K
- mm10 RRBS PE 10K

记录内容：

- baseline/current binary SHA256
- reference/read SHA256
- standalone index time/RSS
- align wall/user/sys/CPU/RSS/exit code
- SAM total/mapped/unmapped、FLAG 分布、Top RNAME
- SAM SHA 一致性检查

## 3. 最终短基准

### 3.1 环境与输入

- 运行时间：2026-06-24
- 结果目录：`D:/BSMAP/benchmark-results/s1/s1-pipeline-20260624T021811Z`
- baseline commit：`9a4f7ca`
- baseline binary SHA256：`96ac6f102b77245444a40a802132a46148a69a90c4030ecf8ea769341c088186`
- current binary SHA256：`796976a35d259c18a544bee1799b5badebb73f39c749e1c61d7d480c7f81e43f`
- reference：`D:/BSMAP/benchmark-data/mm10/mm10.fa`
- WGBS SE：`D:/BSMAP/benchmark-data/mm10/wgbs_10k/se75_10k/simulated.fastq.gz`
- WGBS PE：`D:/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k/simulated_1.fastq.gz`、`simulated_2.fastq.gz`
- RRBS PE/SE：`D:/BSMAP/benchmark-data/mm10/Ctrl_10K_R1.fq`、`Ctrl_10K_R2.fq`
- threads：`8`
- random seed：`1`

参数：

```bash
# WGBS SE
bsmap align -a <WGBS_SE> -d <mm10.fa> -o out.sam -s 16 -v 0.08 -I 4 -p 8 -S 1

# WGBS PE
bsmap align -a <WGBS_R1> -b <WGBS_R2> -d <mm10.fa> -o out.sam -s 16 -v 0.08 -I 4 -p 8 -S 1

# RRBS SE
bsmap align -a <RRBS_R1> -d <mm10.fa> -o out.sam -s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1

# RRBS PE
bsmap align -a <RRBS_R1> -b <RRBS_R2> -d <mm10.fa> -o out.sam -s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1
```

current runs additionally tested `--pipeline-depth 1` and `--pipeline-depth 2`。

### 3.2 正确性

| workload | baseline/current SAM SHA | records | mapped | Top RNAME | 结论 |
|---|---:|---:|---:|---|---|
| WGBS SE 10K | 完全一致 | 9,988 | 9,988 | chr1 7.0985% | 通过 |
| WGBS PE 10K | 完全一致 | 19,994 | 19,994 | chr1 7.2122% | 通过 |
| RRBS SE 10K | 完全一致 | 2,423 | 2,423 | chr5 7.0986% | 通过 |
| RRBS PE 10K | 完全一致 | 4,884 | 4,884 | chr1 7.7805% | 通过 |

说明：这里的“一致”是 baseline、current `--pipeline-depth 1`、current `--pipeline-depth 2` 的完整 SAM SHA 一致，不只是统计字段一致。

### 3.3 Align 短基准

| workload | baseline wall(s) | current d1 wall(s) | d1 变化 | current d2 wall(s) | d2 变化 | RSS 变化 |
|---|---:|---:|---:|---:|---:|---:|
| WGBS SE 10K | 8.30 | 8.32 | +0.24% | 8.00 | -3.61% | 约 0% |
| WGBS PE 10K | 11.83 | 11.55 | -2.37% | 11.67 | -1.35% | 约 0% |
| RRBS SE 10K | 7.42 | 7.39 | -0.40% | 7.63 | +2.83% | 约 0.1% |
| RRBS PE 10K | 9.20 | 9.00 | -2.17% | 9.14 | -0.65% | 约 0.1% |

解释：

- `d1` 是最终默认行为，走串行路径并包含 batch 容器复用。
- `d2` 是显式 pipeline 原型。WGBS SE 本轮收益较明显，WGBS PE 轻微收益；RRBS 因代码保护仍走串行路径，差异属于短基准噪声。
- 短基准收益没有达到“默认启用 pipeline”的门槛。当前只保留 opt-in 参数和框架。

### 3.4 Standalone index 记录

| mode | baseline wall(s) | current wall(s) | 说明 |
|---|---:|---:|---|
| WGBS index | 298.38 | 306.25 | 单独记录，不并入 align 比较 |
| RRBS index | 43.26 | 40.68 | 单独记录，不并入 align 比较 |

本轮代码没有修改 index 构建逻辑，这里的差异应视为缓存、文件系统和运行噪声，不能作为 S1 index 优化收益声明。

## 4. 本地验证

已执行：

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
bash -n benchmark/s1/run_short_pipeline_benchmark.sh
python3 -m py_compile benchmark/s1/summarize_short_pipeline.py
```

结果：

- `cargo check -p bsmap` 通过，保留既有 warning。
- `cargo test -p bsmap` 通过。
- `cargo build --release -p bsmap` 通过。
- benchmark 脚本语法检查通过。
- Python summary 脚本编译通过。

## 5. 保留与不保留决策

保留：

- batch 外层容量复用。正确性通过，默认路径轻量、风险低。
- `--pipeline-depth` CLI 和 WGBS opt-in pipeline。正确性通过，为后续大样本验证保留入口。
- S1 back-to-back runner。它解决了前面多轮性能判断中 baseline/current 不同环境的问题。

不默认启用：

- WGBS pipeline。短基准收益存在波动，没有达到 S1 默认启用标准。
- RRBS pipeline。早期测试出现 RRBS 回退风险，最终实现强制 RRBS 串行保护。

未执行：

- WGBS PE 5G 长测。
- RRBS PE 5G 长测。
- read arena + fused encode。
- PE interleaved pairing。
- direct ordered writer。

## 6. 后续建议

下一步不应继续微调当前 pipeline 的小参数。更高收益路径仍是 S1 计划中的 Phase 2/3：

1. read arena + fused encode，目标是减少每 read 分配和复制。
2. PE interleaved pairing，目标是减少 PE 两端完整 SE 后再配对的重复工作。
3. direct ordered writer，等 pipeline 和 PE 调度稳定后再做，避免重复 P17 的低收益 formatter 改动。

只有 WGBS/RRBS 5G 长测证明 `--pipeline-depth > 1` 在真实大样本上稳定收益，才应考虑把 pipeline 从 opt-in 改为默认或自适应。
