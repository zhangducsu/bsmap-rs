# S1 激进生产级优化计划

## 1. 状态与目标

S1 从 `main` 的 `6e98dcb` 新建分支 `codex/s1-aggressive-optimization-plan`。本阶段不再继续 P17 式低风险小改，而是允许重构批处理、配对、输入输出和调度边界，目标是在保持 C++ BSMAP 2.90 语义等价的前提下，寻找最大可能速度、内存和 CPU 利用率收益。

本文件是 S1 的评估与执行计划，不包含生产代码实现。S1 的核心判断是：P15/P16 已经基本榨干局部低风险收益，继续单点 formatter、mmap advice 或编译选项不会带来足够收益；下一步最大空间来自架构级重构。

Rust standalone index 仍然单独计时，不并入 Rust/C++ 单样本 align 时间比较。

## 2. 现有证据

### 2.1 P15/P16 已保留收益

- WGBS v8 succinct index 已将旧 example1 index 从 519,037,888 bytes 降到 13,691,272 bytes，旧 example1 RSS 从约 509 MiB 降到约 23 MiB。S1 后续短基准不再只依赖 1 Mb `chr22_tail_1M.fa`，改用完整 mm10 reference 生成的 WGBS 10K fixture。
- RRBS v10 packed hit + forward-only build 已将 mm10 RRBS index 降到 1,041,871,696 bytes，standalone index wall 降到 33.86 s，RSS 降到 1,278,988 KiB。
- P16 direct SE SAM formatting 在四个短基准均有小幅收益：WGBS/RRBS wall 降低约 4.67% 到 8.16%。
- RRBS PE 10G 历史长测成功，13,560,000 read pairs、10.0G source bytes、wall 1,806.98 s、CPU 1499%、RSS 0.968 GiB，证明 RRBS 大样本已常数内存且能吃满大部分 CPU。S1 正式长测统一降为 5G，用于缩短迭代周期。
- WGBS PE 90G 历史检查中止前已处理约 109 GB 解压 FASTQ，`bsmap` CPU 约 181%、RSS 33,624 KiB，说明 WGBS 大样本内存稳定，但吞吐受 pipeline、输出或调度限制，CPU 未充分利用。S1 正式 WGBS 长测统一使用 5G。

### 2.2 P17 已证伪的低收益方向

- PE/SAM direct writer 单独实作后短基准无稳定收益，WGBS PE 曾回退 4% 到 10%，已撤回。
- 单独重排 pair-level order、`group_hits_by_chr` Vec 分组、read-chain split 缓存均造成 PE wall 明显回退，已撤回。
- SIMD mismatch probe 结果混合：只有 150bp offset=0 full scan 明显受益，75bp 和 offset case 变慢，不支持直接接入默认热路径。
- benchmark 噪声已经足以污染结论，后续必须使用同轮 back-to-back baseline 或多轮中位数。

### 2.3 当前代码结构热点

- `run_paired_end_alignment()` 仍按批串行执行：read batch -> `process_batch()` -> encode -> `PairAlign::do_pair_batch()` -> 单线程输出。
- `FastqReader::read_batch()` 每条 read 都分配 `RawRead { name, seq, qual }` 三个 `Vec<u8>`。
- `process_batch()` 将 name 转成 `String`，并继续持有 `ReadInf { name, seq, qual }`。
- PE `do_pair_batch()` 对两个 read 各跑完整 `SingleAlign::run_align()`，再在全部 hit 结束后 `compute_pair_hits()`。
- `PairBatchResult` 仍会 flatten `Vec<Vec<PairHit>>`，unpair 路径也会 clone best hits。
- `pairs/output.rs` 仍大量使用 `format!`、临时 `String`、reference name 克隆和 pair/unpair 双记录字符串返回。
- 当前 `BATCH_SIZE=8192`，pipeline depth 固定为 1，读取/解压、编码、比对、格式化、写出不能充分重叠。

## 3. 最大可能优化点排序

| 排名 | 优化点 | 主要受益场景 | wall time 估算 | RSS/内存估算 | CPU 利用率估算 | 置信度 | 结论 |
|---:|---|---|---:|---:|---:|---|---|
| 1 | bounded streaming pipeline + stage scheduler | WGBS 5G PE、gzip FASTQ、SAM 输出 | 大样本降低 20% 到 45%；短基准 0% 到 10% | 增加 1 到 3 个 batch，可控 | WGBS PE 从约 181% 提升到 400% 到 900% | 中高 | S1 第一主线 |
| 2 | C++ 等价 PE interleaved pairing | WGBS PE、RRBS PE | PE 降低 15% 到 40%；RRBS PE 5G 降低 10% 到 25% | 临时 pair/hit 分配降低 30% 到 70% | 提升 5 到 20 个百分点 | 中 | S1 第二主线 |
| 3 | read arena + fused trim/encode | 所有 FASTQ，尤其大样本 WGBS | WGBS 降低 8% 到 20%；RRBS 降低 3% 到 8% | 每 read heap allocation 降低 60% 到 90% | 小幅提升 | 中高 | 随 pipeline 实施 |
| 4 | direct ordered pair writer + byte buffer | SAM 输出占比较高的 PE | PE 降低 5% 到 15%；大 SAM 输出可能更高 | 输出临时 String 降低 50% 到 90% | sys time 降低 | 中 | 只有配合 pipeline 才做 |
| 5 | native BAM/direct BGZF 输出路径 | 生产直接下游 BAM/管道 | 输出字节降低 40% 到 80%；wall 取决于压缩等级 | SAM 中间文本接近 0 | CPU 可能上升 | 中低 | 作为可选生产模式，不影响 SAM parity |
| 6 | profile-guided mismatch/kernel dispatch | RRBS/PE mismatch 热点 | 3% 到 12%，也可能为 0 | 基本不变 | 小幅提升 | 中低 | 后置，必须 profiler 证明 |
| 7 | RRBS mmap/index 微调 | RRBS 大索引冷启动 | 0% 到 8% | 0% 到 8% | 基本不变 | 低 | 非主线，避免重复 P15/P17 坑 |

综合收益不能线性相加。S1 的现实目标是：WGBS PE 5G 吞吐提高 25% 到 60%；RRBS PE 5G wall 降低 15% 到 35%；短基准不要求巨大收益，但必须无正确性回归。

## 4. S1 架构方案

### 4.1 新 pipeline 边界

当前路径：

```text
read batch -> process_batch -> encode -> align full batch -> format/write
```

S1 目标路径：

```text
reader/decompress
  -> batch arena + trim/filter/fused encode
  -> ordered bounded queue
  -> align workers
  -> ordered writer
```

关键要求：

- pipeline 必须保留输入 read order；writer 按 `batch_id` 严格顺序输出。
- pipeline depth 默认先为 1，验证后允许 `--pipeline-depth <N>` 或内部自适应。
- 每个 batch 有明确内存预算：arena bytes、encoded reads、hit scratch、output buffer。
- PE 的 R1/R2 仍保持锁步 batch，禁止重现 FIFO/PE 打开顺序死锁。
- 统计使用 batch-local plain counters，writer 或主线程归并，避免无意义原子计数。

### 4.2 PE interleaved pairing

当前 PE 是两端分别完整 SE alignment 后再 pairing。S1 允许拆分 `SingleAlign`，但必须先建立中间等价护栏：

- 将 `SingleAlign::run_align()` 拆为可观察阶段：
  - seed extraction；
  - chain/mode reorder；
  - segment/mismatch-level extension；
  - accepted hit append；
  - early stop decision。
- `PairAlign` 改为 C++ `pairs.cpp::RunAlign()` 等价调度：
  - segment/mismatch level 后立即尝试 `GetPairs(i,i)`、`GetPairs(i,j)`、`GetPairs(j,i)`；
  - 只维护当前最低 total mismatch level；
  - 找到可接受 pair 后按 C++ 早停；
  - unpair fallback 仍与当前 golden 一致。
- 用连续 range 或小型 fixed table 表示 read-chain/chromosome 分组，禁止在热路径反复构建 `HashMap`。
- pair hit 输出顺序必须由 fixture 锁定，不允许只追求速度而改变 primary/secondary 顺序。

### 4.3 Read arena 与 fused encode

- `RawRead` 不再为 name/seq/qual 分别分配 Vec；batch arena 保存原始字节，read handle 只记录 range。
- QNAME 截断到第一个 ASCII 空白的规则必须保留。
- trim、adapter、N 计数、quality summary、forward/reverse encode 尽量合并为一次序列扫描。
- `ReadInf` 逐步拆成：
  - `ReadView`：借用 arena，供输出；
  - `EncodedRead`：固定数组，供 alignment；
  - `ReadMeta`：index、read_set、trimmed_len、flags。
- 如果完全借用导致生命周期过度复杂，可先用 per-batch arena + stable offsets，不引入跨 batch borrow。

### 4.4 Direct writer

P17 证明“只改 formatter”收益不稳定，因此 S1 只在 pipeline 和 PE 重构后做 writer：

- worker-local `Vec<u8>` 直接写 SAM 行，不返回 `(String, String)`。
- reference name、ZS、CIGAR short path 尽量用 borrowed/static bytes。
- 数字格式化使用 `itoa` 或等价低分配路径；若新增依赖必须单独说明收益。
- writer 接收完整 batch output bytes，顺序写入 `BufWriter` 或 stdout lock。
- BAM 作为可选生产路径：直接构造 noodles record，不经过 SAM 文本；不影响 SAM parity 验收。

## 5. 分阶段执行计划

### Phase 0：S1 基线与 profiler

目标：先证明最大热点，防止激进重构跑偏。

- 新增 `benchmark/s1/`，复用 P15/P16 runner，但支持同轮 baseline/candidate back-to-back。
- 固定四个短基准：mm10 WGBS SE 10K、mm10 WGBS PE 10K、mm10 RRBS SE 10K、mm10 RRBS PE 10K。
- 固定两个长程样本：RRBS PE 5G、WGBS PE 5G，均用 `TARGET_SOURCE_BYTES=5G`。
- 旧 `chr22_tail_1M.fa` example1/example2 只作为快速 smoke 或历史回归参考，不再作为 S1 主要 WGBS 验收数据。
- 记录阶段耗时：read/decompress、process/encode、align、format/write、wait/order。
- 记录 CPU：`/usr/bin/time -v`、perf stat、RSS、major/minor faults、SAM bytes、records、binary/input/index SHA。
- 增加 allocation counter feature 或 heaptrack/DHAT 脚本，用于证明 arena/direct writer 是否值得。

保留标准：baseline 可三轮复现；C++ control 或 Rust baseline 漂移超过 10% 的 run 只作为噪声，不作为保留依据。

### Phase 1：bounded pipeline 原型

目标：在不改变 alignment 结果的前提下，先重叠读取、编码、比对和写出。

- 新增内部 pipeline executor，默认 `pipeline_depth=1` 保持行为。
- reader stage 生成 `Batch { id, reads }`。
- align stage 使用现有 SE/PE 逻辑，不重构 pairing。
- writer stage 按 batch id 顺序写出。
- 验证后允许 `pipeline_depth=2/3`。

验收：

- mm10 WGBS SE 10K 完整 SAM 与 C++ 100% 一致；若 C++ 失败，记录退出码和 stderr。
- mm10 WGBS PE 10K 与 RRBS PE 10K 的 Rust p1/p8 SAM SHA 稳定；C++ PE 若继续失败，按既有限制记录。
- WGBS 大样本 CPU 利用率明显提升，wall 降低至少 15%，否则不作为默认启用。

### Phase 2：read arena + fused encode

目标：降低大样本每 read 分配与复制。

- arena 保存 QNAME/SEQ/QUAL，read handle 记录 offset/len。
- 将 `process_batch()` 和 `encode_read_with_quality()` 融合成一个 pipeline stage。
- adapter/quality/N 过滤仍逐项测试。
- 保留旧 `ReadInf` 兼容层，先让 output 使用 view，再逐步删除复制。

验收：

- 每 read heap allocation 降低至少 60%。
- mm10 WGBS PE 10K 或 WGBS PE 5G wall 降低至少 8%。
- 所有 SAM parity 不变。

### Phase 3：PE interleaved pairing

目标：重构最大 PE CPU 热点。

- 拆分 `SingleAlign` 内部阶段，增加 test-only probe 导出中间 hit level。
- 用 C++ 源码和小 fixture 锁定 `RunAlign/GetPairs` 调度顺序。
- `PairAlign` 逐 mismatch level 执行 pairing，找到低 mismatch pair 后提前停止。
- 删除热路径中重复 chain split、HashMap 分组和 flatten。
- 保持 unpair fallback 与 P16 golden 一致。

验收：

- mm10 WGBS PE 10K p1/p8 SHA 稳定。
- RRBS PE 10K 仍为 4,884 records，Top chr1 不异常。
- PE 短基准 wall 至少降低 10%；RRBS PE 5G wall 至少降低 15%。

### Phase 4：ordered direct writer

目标：把 pipeline 的输出端从 String 模式改为 batch byte buffer。

- SE 和 PE 共用 `SamLineWriter` trait 或轻量 helper。
- pair/unpair 不再返回 `String`，直接 append 到 worker-local `Vec<u8>`。
- writer stage 只做顺序落盘，不再做字段计算。
- 可选实现 BAM direct writer，但 SAM parity 仍是主验收。

验收：

- P16 direct SE formatter 收益不回退。
- PE writer 端 allocation 降低至少 70%。
- 大样本 SAM 输出 wall 降低至少 5%，或 sys time 降低至少 10%。

### Phase 5：自适应调度与生产参数

目标：把 S1 从实验变成可控生产模式。

- 根据输入是否 gzip、SE/PE、WGBS/RRBS、输出 SAM/BAM 自动选择 pipeline depth。
- CLI 可暴露 `--pipeline-depth`、`--io-threads`，默认仍保守。
- 批大小从常量扩展为内部策略：小样本避免过大固定开销，大样本提高吞吐。
- 记录每个 stage backpressure，防止 writer 慢导致内存膨胀。

验收：

- 默认参数在短基准无回归。
- 大样本有明确吞吐收益。
- 峰值 RSS 有公式上界：`index RSS + pipeline_depth * batch_budget + output_buffer_budget`。

### Phase 6：后置 CPU kernel

只有 profiler 证明 mismatch 或 encode kernel 仍是主要热点时才做。

- 重新评估 POPCNT/AVX2/AVX-512，按真实 read length、offset、early abort 分布选择。
- `target-cpu=native` 只能作为本机优化构建，不替代 portable release。
- 编译选项、LTO、allocator 替换必须经过完整 WGBS/RRBS 矩阵，不能只凭小样本。

## 6. 验收标准

每个保留提交必须通过：

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
git diff --check
```

短基准正确性：

- mm10 WGBS SE 10K：Rust/C++ 非 header SAM 完整逐行 100% 一致；`RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。若 C++ 在完整 mm10 WGBS SE 10K 上失败，必须记录退出码和 stderr，并以 Rust p1/p8 稳定性作为临时回归门槛。
- mm10 WGBS PE 10K：Rust p1/p8 完整 SAM SHA 一致；若 C++ PE 成功，则增加 Rust/C++ 完整记录和字段 diff；若 C++ PE 失败，按失败记录。
- mm10 RRBS SE 10K：Rust/C++ 2,423 条完整逐行 100% 一致；字段 diff 为 0。
- mm10 RRBS PE 10K：Rust 4,884 records；p1/p8 SHA 一致；Top chr1 不异常。

大样本性能：

- RRBS PE 5G：保持常数 RSS，不高于 P15/P16 同口径 1.05 GiB 10%；wall 至少降低 15% 才能宣称 S1 PE 收益。
- WGBS PE 5G：RSS 保持几十 MiB 到可解释 batch 上界；wall 至少降低 20% 或 CPU 利用率至少翻倍，才默认启用 pipeline。

## 7. 风险与回退

- PE interleaved pairing 可能改变 primary/secondary 顺序，是 S1 最大正确性风险。必须先上中间 fixture，再切生产路径。
- Pipeline 可能改变输出顺序。必须按 batch id 顺序写出，不能让 worker 直接写 stdout/file。
- Arena 可能引入生命周期复杂度。若实现开始扩散，应退回 stable offset 模式，而不是到处传播复杂 borrow。
- 大样本 benchmark 可能受 sink 限速、DrvFS、page cache 和 WSL 调度影响。正式结论必须标注文件系统、sink 限速和 cache 状态。
- P17 已撤回的微优化不得原样重试；除非 profiler 证明瓶颈改变，并且使用同轮 back-to-back baseline。

## 8. S1 交付物

- `benchmark/S1_Aggressive_Optimization_Plan.md`：本计划。
- `benchmark/S1_Aggressive_Optimization_Report.md`：实施后记录每阶段结果、保留/撤回结论。
- `benchmark/s1/`：back-to-back runner、stage timing、perf/alloc summary、SAM parity 工具入口。
- 生产代码只在通过对应阶段验收后保留；未达标阶段必须撤回，并在报告中保留失败证据。

## 9. 当前建议

S1 第一批实现不应从 SIMD、LTO 或 mmap advice 开始。建议顺序固定为：

1. Phase 0 profiler 与 back-to-back runner。
2. Phase 1 bounded pipeline 原型。
3. Phase 2 read arena + fused encode。
4. Phase 3 PE interleaved pairing。
5. Phase 4 direct ordered writer。

这一路线优先解决 WGBS 大样本 CPU 利用率不足和 PE 结构性重复工作，才有机会获得超过 P16/P17 小幅收益的实质提升。
