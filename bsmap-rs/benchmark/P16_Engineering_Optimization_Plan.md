# P16 工程化性能优化计划

## Summary

P16 继续在 P15/P13 已完成的 C++ 语义等价基础上，识别并验证可落地的工程化性能优化点。目标不是改变比对逻辑，而是在保持 Rust/C++ 结果等价的前提下，继续压低 wall time、峰值 RSS、I/O 放大和无效 CPU 开销。

本阶段必须坚持两个边界：

- Rust 的 standalone index 构建是一次性可复用步骤，单样本 align 性能对比不得把 Rust 建索引时间并入 align wall time。
- WGBS 与 RRBS 都是硬验收路径；任何只让一个 workload 变快、同时让另一个 workload 明显回退的候选，都不能作为默认优化保留。

## 当前性能画像

P15 已完成的核心能力：

- RRBS/WGBS 结果已对齐 C++，mm10 RRBS SE 10K 达到 2,423/2,423 完整一致。
- WGBS example1 已要求 Rust/C++ 的非 header SAM 完整逐行一致，并覆盖 `RNAME/POS/FLAG/NM/ZP/ZL`。
- Rust v10/v15 索引已经具备 standalone index、warm `.bsi`、压缩布局和 mmap 读取能力。
- P15 证明 DrvFS 会显著放大 mmap/random access 的 page fault 和 wall time；正式性能结论优先使用 WSL ext4 或服务器 Linux 文件系统。

已踩过且不能直接保留的候选：

- 全局 release profile 调参，例如 ThinLTO、`codegen-units=1`、`panic=abort`，在 WGBS 小样本上可能变快，但 RRBS SE/PE 会回退。
- 只把已生成的 SAM `String` 改成 ASCII 写入，没有减少核心分配，实测无稳定收益。
- 非 TTY progress bar 隐藏属于批处理清理项，不应包装成稳定速度优化。
- PE 临时对象复用、read-chain 预拆分等局部改法，如果没有同环境 WGBS/RRBS 双覆盖 benchmark，容易出现 PE 或 RRBS 回退。

## P16 优化候选

### 1. SAM 输出路径低分配重构

方向：

- 为 SE/PE 的热输出路径提供 direct SAM writer，直接向 `Write` 或复用缓冲区写字段。
- 避免为每条记录构造完整 `AlignmentRecord` 后再 `String` 化。
- 减少 `read.name`、reference name、CIGAR、optional tag 的重复 clone/format。

边界：

- 不改变 SAM 字段顺序、optional tag 顺序和现有 C++ 等价输出。
- 先对 example1 和 mm10 RRBS SE 做逐行 100% 等价，再扩展 PE。
- 如果只减少分配但 wall/RSS 不改善，作为候选撤回或降级到后续 P17。

预期收益：

- WGBS/RRBS 大样本输出量增长时，减少 allocator 压力和 CPU format 开销。
- 对 WGBS 90G 和 RRBS 10G 更有意义，因为输出记录数量远大于 10K smoke 数据。

### 2. PE pairing scratch 与分组结构收敛

方向：

- 梳理 PE pairing 中每批次重复创建的临时 `Vec`、`HashMap`、分组表和 mate 结果容器。
- 在 batch 级复用 scratch buffer，但不跨 read 泄漏状态。
- 优先替换可预测的小范围分组，不做大规模架构改写。

边界：

- 不改变 AddHit、随机多重命中、pair score 和 C++ 坐标语义。
- 每个 scratch 复用点必须有单元测试或 example/mm10 对比保护。
- PE C++ 对照若继续 signal 6/134，报告如实记录，Rust PE 自身必须保持 4,884 条和正常 RNAME 分布。

预期收益：

- 降低 PE 大样本中临时分配和哈希开销。
- 在高线程场景下减少 allocator contention。

### 3. mismatch 热路径 microbench 与 SIMD 接入评估

方向：

- 先建立 scalar/SIMD mismatch microbench，覆盖 WGBS/RRBS seed extension 的真实 read length 和 mismatch threshold。
- 审计现有 AVX2 代码是否真正接入热路径，避免只存在代码但未被调用。
- 只在 microbench 和端到端 benchmark 同时证明收益时接入默认路径。

边界：

- `N` mismatch 默认语义必须保持 C++：未显式 `-N` 时 masked N 不计入 NM。
- scalar、SIMD、fallback 对同一 fixture 的 mismatch、NM、ZP/ZL 必须一致。
- 非 x86_64 或无 AVX2 机器必须稳定回退。

预期收益：

- 对 WGBS 90G 这种 extension 调用量大的 workload 可能最有价值。
- 对 RRBS 10G 的收益取决于候选过滤后进入 mismatch 的真实比例。

### 4. mmap 与索引访问的文件系统适配

方向：

- 保持 RRBS mmap 默认 `MADV_RANDOM` 基线，不因 major faults 数字下降而盲目启用预读。
- 增加 benchmark 记录项：文件系统类型、index/reference 所在路径、major faults、minor faults。
- 对 ext4 与 DrvFS 分别记录结果，避免把 Windows 文件系统限制误判成算法回归。

边界：

- 不为 DrvFS 做破坏 Linux 性能的默认策略。
- `.bsi` 格式兼容性必须通过版本号、marker 和边界校验保护。
- Rust standalone index 继续单独计时；warm align 只使用已存在 `.bsi`。

预期收益：

- 减少错误优化方向，特别是 RRBS 大索引 random access 的 page-cache 判断。
- 为 10G/90G 长测提供可解释的 I/O 和 RSS 数据。

### 5. 线程数、batch size 与 I/O backpressure 矩阵

方向：

- 固定 p1/p2/p4/p8/p16 的线程矩阵，比较 wall、CPU%、RSS 和 SAM SHA。
- 将 batch size 作为可配置实验项，寻找 WGBS 90G/RRBS 10G 下的吞吐与内存平衡点。
- 记录 gzip 输入、SAM 输出路径所在文件系统，区分 CPU-bound 与 I/O-bound。

边界：

- 默认线程策略不因 10K 小样本偶然结果改变。
- 所有矩阵结果必须保持 SAM 等价或已知 C++ PE 限制。
- 跨 PowerShell/WSL 传参不得使用含空格环境变量；优先使用脚本默认矩阵。

预期收益：

- 提高 CPU 利用率解释能力，避免“线程越多越快”的错误假设。
- 为真实 WGBS 90G/RRBS 10G 生产参数提供依据。

## 实施顺序

1. **冻结基线**
   - 使用 P15 commit `93e322d` 作为 baseline。
   - 使用当前 P16 分支作为候选分支。
   - 同一机器、同一脚本、同一输入、同一文件系统路径运行 benchmark。

2. **恢复无收益候选**
   - 对已经证明负收益或不稳定的候选保持撤回状态。
   - 当前未提交候选若 benchmark 显示 PE 或 RRBS 回退，先撤回再进入下一候选。

3. **先做输出路径 micro-optimization**
   - 从 reference name 借用、optional tag 静态字符串、缓冲区复用等小步开始。
   - 每一步只修改一个热点，单独编译测试和 benchmark。

4. **再做 PE scratch**
   - 只在输出路径稳定后进入。
   - 每个 scratch 复用点都要证明没有跨 read 状态污染。

5. **最后做 SIMD/mmap/线程矩阵**
   - SIMD 先 microbench，再接入端到端。
   - mmap advice 不用 faults 单指标决策。
   - 线程矩阵用于生产参数建议，不直接等价为代码优化。

## 验证矩阵

每个保留候选必须通过：

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
python3 -m py_compile benchmark/p15/*.py
python3 -m unittest benchmark/p15/test_tools.py
bash benchmark/p16/run_short_validation.sh . /mnt/d/BSMAP/benchmark-results/p16/<run-id>
```

必须记录：

- commit、branch、dirty 状态。
- Rust/C++ binary SHA256。
- reference 与 reads SHA256。
- 完整命令与参数。
- exit code、wall/user/sys、CPU%、max RSS。
- SAM records、mapped/unmapped、unique/multiple、FLAG/RNAME 分布。
- WGBS example1 的 `RNAME/POS/FLAG/NM/ZP/ZL` 和完整记录 100% 一致。
- mm10 RRBS SE 的 2,423 条完整记录 100% 一致。
- mm10 RRBS PE 的 records、Top RNAME 占比和 chr1 偏斜检查。

## 大样本适用性

当 WGBS 增加到 90G、RRBS 增加到 10G 时，本计划仍适用，但 10K smoke 只能作为功能与趋势筛选，不能作为最终性能结论。

大样本必须补充：

- Rust standalone index 单独计时、单独报告。
- warm align 至少 3 轮，报告中位数和最坏 RSS。
- 输入、输出、index、reference 的文件系统位置。
- gzip 解压、SAM 写出、mmap page fault 的分项判断。
- p1/p2/p4/p8/p16 线程矩阵。
- 至少一个子集做完整 Rust/C++ SAM 等价；全量可做统计等价与抽样 QNAME 精确比对。

## 淘汰条件

任一候选出现以下情况，默认撤回：

- WGBS example1 完整记录或 `RNAME/POS/FLAG/NM/ZP/ZL` 不再 100% 一致。
- mm10 RRBS SE 2,423 条完整记录不再 100% 一致。
- RRBS PE records 或染色体分布回退，特别是 chr1 Top1 异常偏斜。
- WGBS 或 RRBS 任一路径 wall time 明显回退，且没有明确的语义等价收益支撑。
- 峰值 RSS 明显上升，且不是为减少 wall time 做出的可解释权衡。
- 只在 DrvFS 或单轮 page-cache 抖动下看似收益，不能在同环境复测中复现。

## 交付物

- 更新后的 `benchmark/P16_Engineering_Optimization_Report.md`。
- 每个保留优化一个独立 commit。
- `benchmark/p16/` 下保留可复现脚本和 summary。
- 若发现新陷阱，立即追加到根目录 `AGENTS.md`。
- 未达标候选必须写入报告的“被拒绝候选”，不得从历史中消失。
