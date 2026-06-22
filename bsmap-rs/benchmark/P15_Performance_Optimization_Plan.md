# P15 极致速度、内存与 CPU 效率优化计划

## 1. 目标与原则

P15 在 P14 已达到 C++ BSMAP 2.90 结果等价的基础上，继续压缩索引、峰值内存和单样本 wall time，并提高计算阶段的多核利用率。优化优先级固定为：

1. 完整 SAM 结果等价；
2. 消除结构性内存和 I/O 浪费；
3. 降低每条 read 的分配、分支和重复计算；
4. 提升并行流水线与 CPU 指令效率。

任何优化不得改变 seed、candidate、随机 bucket 起点、候选遍历顺序、AddHit 早停或 SAM 输出顺序。没有达到独立收益门槛的复杂实现不保留。

Rust 的 standalone index 始终单独计时，绝不计入 Rust 与 C++ 的单样本比对时间。单样本比较继续采用“Rust 已有兼容索引的 process time”对“C++ 正常 invocation”。

## 2. P14 基线与热点证据

### 2.1 基线指标

| 场景 | P14 wall | CPU | max RSS | 关键状态 |
|---|---:|---:|---:|---|
| WGBS example1 SE，Rust p8 | 2.00 s | 76% | 509,540 KiB | 66,120 条，与 C++ 完整记录 100% 一致 |
| WGBS example1 SE，C++ p8 | 1.46 s | 117% | 872,184 KiB | 有效 C++ golden |
| WGBS example2 PE，Rust p8 | 2.39 s | 81% | 518,580 KiB | 66,958 条，Rust p1/p8 字节一致 |
| mm10 RRBS 10K SE，Rust p8 | 中位 12.39 s | 118% | 最坏 1,309,692 KiB | 2,423 条，与 C++ 完整记录 100% 一致 |
| mm10 RRBS 10K PE，Rust p8 | 13.84 s | 138% | 1,401,320 KiB | 4,884 条，Rust p1/p8 字节一致 |
| mm10 RRBS standalone index | 49.14 s | 56% | 1,911,532 KiB | 1,773,733,496 bytes |

### 2.2 WGBS 索引结构浪费

P14 example1 v7 索引总大小为 519,037,888 bytes（494.99 MiB）：

| section | 大小 | 占比 |
|---|---:|---:|
| `index2` | 328.42 MiB | 66.35% |
| `start_offsets` | 164.21 MiB | 33.17% |
| `positions` | 1.87 MiB | 0.38% |
| `refcat + crefcat` | 0.49 MiB | 0.10% |

`seed_size=16` 固定创建 43,046,721 个 dense bucket，但 1 Mb reference 最多只有 491,030 个有效 positions，非空 bucket 占比上限仅 1.14%。当前约 99.5% 文件空间用于 dense bucket metadata，而不是参考序列或真实命中。这是 P15 的最高优先级内存问题，也直接造成 WGBS 启动缺页和 500 MiB RSS。

### 2.3 RRBS mmap 与缺页瓶颈

P14 mm10 RRBS v7 索引主要 section：

| section | 大小 | 占比 |
|---|---:|---:|
| `refcat` | 651.10 MiB | 38.49% |
| `crefcat` | 651.10 MiB | 38.49% |
| `rrbs_hits` | 374.88 MiB | 22.16% |
| 其他 section | 14.49 MiB | 0.86% |

最终 SE 中位轮次为 `user=4.25s`、`sys=10.39s`、331,750 major faults，说明主要时间花在随机 mmap 缺页和存储访问，而不是 mismatch 算力。P14 中默认预读轮次曾达到约 5.34 秒和 1.60 GiB RSS；全局 `MADV_RANDOM` 将 RSS 降至约 1.25 GiB，却把 wall 增至约 12 秒。P15 不再对全部 RRBS section 使用同一种 advice。

### 2.4 每 read 热路径分配与重复工作

当前实现仍存在以下可直接定位的开销：

- `extract_seeds()` 为每条 read 创建两层 `Vec` 并提取所有位置；`SeedSegment` 又为 `seeds/reg_masks/seed_positions` 分别分配和克隆。
- `SingleAlign` 使用默认 SipHash 的两个 `HashSet<(u32, u32)>` 做内部整数 key 去重。
- RRBS SE 对同一 bucket 先 `.filter().count()`，再创建第二条过滤迭代器执行 circular walk。
- `compute_pair_hits()` 在每个 mismatch 组合中重复拆分 read-chain、构建 `HashMap`、分组和复制命中。
- `format_sam()` 先构建多个 owned `String`，再构建最终行；reference name、CIGAR、ZS、SEQ 和 QUAL 均有临时分配。
- BAM 输出先生成 SAM 文本，再解析为 noodles `Record`，形成无必要的格式化与反解析往返。
- FASTQ 读取、read 处理、编码、比对、格式化和写出按批串行衔接；编码本身仍是串行 `.map()`。
- SAM 统计在单线程输出循环中使用 `Arc<AtomicU32>`，没有并行共享收益。

### 2.5 CPU kernel 机会

- 热路径实际调用标量 `count_mismatch()`；现有 `count_mismatch_simd()` 仅被测试调用。
- 当前 release profile 未显式启用 LTO、单 codegen unit 或 CPU feature dispatch。
- 本机 Ryzen 7 255 支持 POPCNT、BMI1/2、AVX2、AVX-512 和 AVX-512 VPOPCNTDQ，但默认 portable binary 未针对这些能力专门生成热 kernel。
- `snp_align_segment()` 在每个 seed 内重复判断 WGBS/RRBS 模式，并通过动态 storage 接口反复取得 slice。

## 3. Benchmark 与 profiling 口径

### 3.1 明确定义四种计时

1. **standalone index**：单独记录 index wall/user/sys、CPU、RSS、文件大小和 section 大小。
2. **index-warm process**：`.bsi` 已存在，但不假定 OS page cache 状态；这是 Rust/C++ 单样本主比较口径。
3. **page-cache-cold process**：显式确认或清理 page cache 后运行；无法可靠清理时标记为“未测”，不得伪装成 cold。
4. **page-cache-warm process**：同一 binary/index/input 连续运行至少三次，使用中位数；用于分析纯计算和多核扩展。

alignment core、解析/编码、等待缺页、格式化/输出分别打点。报告不能再用一个含义不明确的“warm”混合这些阶段。

### 3.2 profiling 工具

- 必选：`/usr/bin/time -v`、阶段计时、index section 统计、binary/input/index SHA256。
- CPU：`perf stat` 记录 cycles、instructions、IPC、branches、branch-misses、cache-misses、page-faults、context-switches；若本机未安装，实施前单独确认安装权限。
- 火焰图：`perf record` 或等价采样工具，分别覆盖 WGBS example1、RRBS SE、RRBS PE。
- 分配：优先使用 DHAT、heaptrack 或 allocator 计数 feature；只在有数据证明 allocator 本身仍是热点后评估替换 allocator。
- I/O：记录 major/minor faults、file-system inputs、sys time，并区分 NTFS/DrvFS 与 WSL ext4。同一结论必须在部署目标文件系统上复核。

### 3.3 固定数据与参数

- WGBS reference：`bsmap-rs/benchmark/data/chr22_tail_1M.fa`。
- WGBS example1：`data/wgbs/ex1_se75_10x/simulated.fastq.gz`。
- WGBS example2：`data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz`、`simulated_2.fastq.gz`。
- mm10 reference：`D:/BSMAP/benchmark-data/mm10/mm10.fa`。
- mm10 reads：`Ctrl_10K_R1.fq`、`Ctrl_10K_R2.fq`。
- WGBS：`-s 16 -v 0.08 -I 4 -S 1`。
- RRBS：`-s 12 -v 0.08 -I 4 -D C-CGG -S 1`。
- 线程曲线：`-p 1/2/4/8/16`；正式 Rust/C++ 主表继续使用相同线程数。

新增 mm10 WGBS 10K 场景，用真实多染色体 reference 检查 compact WGBS index 的扩展性、`int2hit` 和全基因组内存；其 standalone index 仍单独计时。

## 4. 分阶段实施计划

每一步必须是独立提交，先通过完整正确性门槛，再运行该步对应 benchmark。任何一步未达到收益门槛则 revert，不把多项未验证优化捆在一起。

### Phase 0：P15 可观测性与基线冻结

- 新增 `benchmark/p15/`，复用 P14 SAM 比较器并增加 index section、page-fault、阶段耗时和线程扩展汇总。
- 固定 P14 binary、P14 `.bsi` 和 golden SAM SHA256。
- 在 D 盘 DrvFS 与 WSL ext4 各跑一组 index-warm/page-cache-warm 基线，确认文件系统造成的差异。
- 为 seed schedule、bucket walk、mismatch、pairing、SAM formatting 建立 Criterion microbench。
- 增加 release-portable 与 release-native 两个明确构建标签；所有报告记录 `rustc -Vv`、RUSTFLAGS 和 CPU flags。

验收：runner 可从空结果目录复现全部 metadata，失败时 fail-fast，不覆盖 P14 原始结果。

### Phase 1：WGBS succinct bucket index（v8）

将 `index2 + start_offsets` 的 12 bytes/dense-bucket 布局替换为自适应 succinct 布局：

- occupancy bitvector：每个 hash 1 bit；43,046,721 个 hash 约 5.13 MiB。
- rank directory：通过分层 prefix/popcount 将 hash 映射到 compact descriptor 下标。
- compact descriptor：优先采用 `offset:u32 + fwd_count:u16 + rev_count:u16` 的 8-byte 布局。
- 对超过 16-bit 但未被 cutoff 过滤的 count 使用显式 overflow side table；不得静默截断。
- `positions` 的 forward/reverse 顺序和 circular bucket 语义保持不变。
- occupancy 较高时允许使用 packed dense descriptor，构建时按估算文件大小自动选择，格式 marker 明确记录布局。

预期：example1 WGBS index 从 494.99 MiB 降至约 12 至 20 MiB，同时减少启动 major faults；full-mm10 WGBS 不因 sparse hash table 过度膨胀。

回退条件：完整 SAM 有任一字节差异；lookup microbench 慢超过 5%；example1 index 大于 32 MiB。

### Phase 2：热路径零/低分配数据结构

- 用 worker-local `SeedScratch` 替代 `Vec<Vec<u32>>`：固定 seed 数组、固定 segment 数组、长度字段和原地排序。
- 只计算 profile 和 offset 实际访问的 seed；若为保持 C++ 调度必须缓存全部位置，则写入固定数组，不分配 heap。
- `SeedSegment` 改为固定容量数组或 slice view，删除 `seeds/reg_masks/seed_positions` 的逐 segment clone。
- 将整数命中去重改为 worker-local open-address table 或 identity hasher；容量从 `max_num_hits` 安全推导并复用。
- 将 `GHit` 压缩为经过测试的 64-bit 表示；字段宽度必须覆盖 mm10 坐标、strand、gap、gap_pos 和 mismatch。若字段范围无法静态证明，保留 16-byte fallback。
- 将 `IndexView`、`ReferenceView` 和 WGBS/RRBS 专用 kernel 在 batch 开始时绑定，移除 seed 内模式分支和 trait dispatch。

验收：每 read heap allocation 数显著下降；alignment core 至少快 8%，或 worker scratch/结果内存至少下降 20%。

### Phase 3：RRBS logical bucket 与页访问优化

- 为 `RRBS_BSC_FLAG` 建立紧凑 eligibility bitmap、rank/select 和每 bucket normal count。
- SE 直接按 logical rank 定位第 N 个 normal hit，删除两次 `.filter()` 全扫描；PE/`-n 1` 保持完整 raw bucket。
- random modulus 仍基于 C++ 对应 logical bucket 长度，normal hit 相对顺序不变，mode 过滤顺序不变。
- mmap 改为 section-scoped 或共享单一 `Arc<Mmap>`，避免三个整文件 mapping；advice 仅作用于对应 section。
- 分别评估 `Normal/Random/WillNeed`、显式 readahead 和候选页软件预取，禁止再次全局一刀切。
- 可选实验：在 batch 内按首个 seed hash/page 对 read 任务稳定分组，比对完成后按原始 read index 输出。只有 random seed、candidate walk 和完整 SAM 均不变时才保留。

验收：mm10 SE major faults 和 sys time至少下降 50%，且 wall/RSS 同时优于 P14；只换取一项、明显牺牲另一项的 advice 变更不保留。

### Phase 4：单份参考链与 RRBS hit 压缩实验（v8）

这是 P15 风险最高、潜在收益最大的结构优化，必须在前面阶段稳定后单独实施。

- `.bsi` 只保存 forward 2-bit reference；reverse candidate 从 forward section 按 C++ padded reverse 规则生成固定长度窗口。
- 为 reverse window 实现标量、POPCNT/SSSE3/AVX2 可选 kernel；运行时 dispatch，portable fallback 必须存在。
- 先在小型跨 word、染色体边界、padding、BSW/BSC fixture 上证明每个 u64 window 与现有 `crefcat` 字节等价。
- 若 reverse-on-demand wall 退化，则保留可选 dual-chain layout；构建器根据 benchmark 或显式参数选择，不强制牺牲速度换内存。
- 在 candidate 顺序不变前提下，实验把 RRBS `Hit.chr` 元数据按原始顺序做 run metadata，loc 保持 32-bit flat array；只有随机起点和顺序访问仍为 O(1)/摊销 O(1) 时才保留。

仅删除 `crefcat` 即可理论减少约 651.10 MiB 索引；RRBS hit metadata 压缩是 stretch goal，不作为单链方案完成的前置条件。

验收：mm10 index 不超过 1.15 GB、index-warm SE max RSS 不超过 1.0 GiB，并且 wall 不慢于 Phase 3。若 reverse-on-demand 使 wall 退化超过 5%，恢复 dual-chain fast layout。

### Phase 5：输入、编码、输出流水线

- 用 batch arena 保存 QNAME/SEQ/QUAL，read 仅持有 range，替代每 read 三个独立 heap allocation。
- 将 trim、N 计数、质量摘要和 fixed read encode 合并为一次序列扫描；保留 adapter 语义。
- 编码并行化，并使用容量为 2 至 3 的 bounded pipeline 重叠“读取/解压 -> 编码/比对 -> 格式化/写出”。
- 每个 stage 保持 batch sequence number，writer 严格按输入顺序提交。
- SAM/BSP 直接写入 worker-local `Vec<u8>`，复用 buffer；reference name 和 ZS 返回 borrowed/static slice，数字使用无临时 `String` 的写入方式。
- stdout 使用持久锁和缓冲，不逐记录 `println!`。
- BAM 直接构造 noodles alignment record，删除“先生成 SAM 文本再 parse”的路径；增加 BAM round-trip 与字段等价测试。
- 统计改为 batch-local plain counters，主线程归并，删除无收益的原子操作。

验收：WGBS example1 process wall 至少下降 10%；SAM、BSP、BAM 各自输出完全等价；pipeline 峰值内存增量有明确上界。

### Phase 6：PE pairing 专项

- 每个 mismatch level 只拆分一次 read-chain，复用 slice/range，不在 `na × nb` 循环中复制命中。
- 用按 chromosome 的连续 range 或固定小表替代临时 `HashMap<u16, Vec<&GHit>>`。
- 坐标转换在每个 hit 上只计算一次并缓存到 worker scratch。
- 直接维护最低 total-mismatch level 和命中数，避免构建全部 `Vec<Vec<PairHit>>` 后再 flatten。
- 保持 C++ 的 level 枚举、pair 插入顺序、nt3 早停和 primary/secondary 顺序。
- 删除或收敛当前未走生产批处理路径的重复 PairAlign 实现，避免两套语义继续漂移。

验收：example2 PE core 至少快 15%，临时分配至少下降 50%，p1/p8 SAM SHA256 完全一致。

### Phase 7：mismatch kernel 与编译配置

- 先把 SWAR mismatch 改写为严格等价的 `(diff | diff >> 1) & 0x5555...` + POPCNT 基线，并保留早停。
- 对 aligned/unaligned offset、1 至 6 words、不同 `snp_thres` 分别 benchmark；短 read 不默认使用处理过量数据的宽 SIMD。
- 运行时 dispatch 候选：portable scalar、POPCNT、AVX2、AVX-512 VPOPCNTDQ。只保留在真实 alignment core 中有收益的版本。
- 将 CPU feature detection 移出 candidate loop，在进程启动时绑定函数或 enum kernel。
- 单独评估 release `lto`、`codegen-units=1`、`panic=abort`；`target-cpu=native` 仅作为明确标记的本机优化构建，不替代 portable release。
- allocator 替换放在最后；只有 allocation profile 仍证明系统 allocator占热点且端到端收益超过 5% 时才引入。

验收：WGBS 与 RRBS alignment core 至少再快 5%；所有 offset、N mask、C/T tolerance、early abort fixture 逐值一致。

### Phase 8：standalone index 构建与落盘

该阶段只优化一次性索引构建，不参与单样本 Rust/C++ wall 比较。

- parallelize forward/reverse encoding 或 single-chain encoding，以 chromosome/chunk 为确定性任务单元。
- 流式生成 v8 section，避免同时持有完整 reference、完整 hit array 和第二份写出缓冲。
- 对 RRBS 两遍 hit 构建使用 thread-local 531,441-entry count table；prefix 后按稳定 partition offset 并行填充，严格保持原始 `mode -> block -> normal/cross` 顺序。
- 大文件写入评估更大的 BufWriter、vectored write、预分配和直接 section mmap；不能以 sparse file 或未落盘缓存伪造完成时间。
- 报告 parse/encode、digestion、hit count/fill、serialize/write 和 fsync 分段时间。

验收：mm10 RRBS standalone index wall 不超过 35 秒、RSS 不超过 1.3 GiB；生成索引与单线程 reference builder 逐 section 等价。

## 5. 正确性门槛

每个保留提交必须通过：

```bash
cargo check
cargo test
cargo build --release -p bsmap
git diff --check
```

随后执行：

- WGBS example1：Rust 与 C++ 非 header SAM 记录顺序、11 个固定字段、全部 optional tags 和完整记录 100% 一致。
- WGBS 字段：`RNAME/POS/FLAG/NM/ZP/ZL` 差异全部为 0；WGBS 双方均不输出 `ZP/ZL`。
- WGBS example2：Rust p1/p8 完整文件 SHA256 一致；C++ 若继续 signal 6，记录退出码、stderr 和空输出，不伪造 golden。
- mm10 RRBS SE：2,423 条与 C++ 完整记录 100% 一致，FLAG 和染色体分布完全一致。
- mm10 RRBS PE：4,884 条，Rust p1/p8 SHA256 一致；C++ PE 失败按既有规则记录。
- 新增 mm10 WGBS 10K：C++ 可成功时要求完整记录 100% 一致；否则保留 Rust 线程确定性与失败证据。
- v8 index：corrupt section、overflow count、marker、边界、v7 拒绝/重建和 mmap/memory round-trip 测试全部覆盖。

P14 golden 继续作为 Rust regression baseline；C++ 是语义真相。两者冲突时以 C++ 源码和有效输出为准。

## 6. P15 目标指标

### 6.1 硬目标

| 指标 | P14 | P15 目标 |
|---|---:|---:|
| example1 WGBS index | 494.99 MiB | <= 32 MiB |
| example1 WGBS p8 max RSS | 509,540 KiB | <= 128 MiB |
| example1 WGBS p8 wall | 2.00 s | <= 1.46 s，至少不慢于 C++ |
| example2 WGBS PE p8 wall | 2.39 s | <= 1.80 s |
| mm10 RRBS index | 1.7737 GB | <= 1.15 GB |
| mm10 RRBS index RSS | 1.823 GiB | <= 1.30 GiB |
| mm10 RRBS index wall | 49.14 s | <= 35 s |
| mm10 RRBS SE index-warm wall | 12.39 s | <= 5.0 s |
| mm10 RRBS SE max RSS | 1.249 GiB | <= 1.0 GiB |
| mm10 RRBS SE major faults | 331,750 | <= 100,000 |
| mm10 RRBS PE index-warm wall | 13.84 s | <= 7.0 s |
| mm10 RRBS PE max RSS | 1.337 GiB | <= 1.1 GiB |

### 6.2 CPU 与扩展目标

- page-cache-warm、计算占主导的 workload 中，p8 alignment core 相对 p1 加速至少 5 倍。
- p1/p2/p4/p8/p16 吞吐曲线必须记录；p16 若受 SMT 或内存带宽限制，不要求强行优于 p8，但不得默认选择更慢线程数。
- 对 I/O-bound 冷启动不以 CPU% 越高越好；主指标是 wall、throughput、sys time、major faults 和能耗/每百万 reads。
- 每个非基础设施优化必须满足：端到端 wall 改善至少 5%，或峰值 RSS/索引大小改善至少 10%，且另一关键指标不得退化超过 5%。

## 7. 提交、回退与报告

- 开发分支：`codex/p15-performance`；不在 P14 历史提交上继续改写。
- 每个 Phase 至少一个独立 commit；v8 格式变更集中管理，最终 marker 和 version 只在布局稳定后确定。
- 每步保存 binary/index/input SHA256、完整命令、环境、原始 `time/perf`、SAM compare 和 section report。
- 报告唯一正式文件为 `P15_Performance_Optimization_Report.md`；中间结果放 `benchmark/p15/results/<step>/`，大型 SAM 保持 ignored。
- 任何失败、revert、仅在 native build 有效的优化和平台限制都必须写入报告。
- 新确认的环境、Git、mmap、benchmark 或工具链陷阱当轮追加到根目录 `AGENTS.md`。

## 8. 明确不做

- 不通过减少候选、改变随机数、改变 hit 顺序或放宽 SAM 比较换取速度。
- 不把 Rust standalone index 时间加回每个样本，也不从 C++ 正常 invocation 人工扣除内部索引时间。
- 不再次对 WGBS/RRBS 所有 mmap section 使用同一种 advice。
- 不在缺少 profile 证据时先引入 jemalloc/mimalloc、宽 SIMD、io_uring 或大面积 unsafe。
- 不只在 1 Mb reference 上证明 index 优化；compact layout 必须覆盖 mm10 WGBS 扩展性。
- 不用单次最快值验收；正式性能表至少三次并报告中位数、最坏 RSS 和 page-cache 状态。

## 9. 推荐执行顺序

1. Phase 0 可观测性与基线冻结。
2. Phase 1 WGBS succinct index，先拿下最大的确定性内存浪费。
3. Phase 2 worker scratch、整数去重和专用 kernel。
4. Phase 3 RRBS logical bucket 与 section 级 mmap 策略。
5. Phase 5 输入/输出流水线和直接 BAM writer。
6. Phase 6 PE pairing 专项。
7. Phase 4 单份参考链；高风险方案在语义与 profile 基础稳定后进入。
8. Phase 7 mismatch/编译级优化。
9. Phase 8 standalone index 构建与最终综合验收。

该顺序优先获得低风险、可测量收益，再处理单链 reference 和压缩 hit 等高风险结构变更，避免同时改变索引布局、候选遍历和 CPU kernel 而无法定位回归。

