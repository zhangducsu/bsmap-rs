# P15 速度、内存与 CPU 效率优化报告

> 状态：进行中。本文记录已经验证并保留的优化；Phase 3、Phase 5-8、大规模长测、最终合并与交付尚未完成。

## 1. 测试口径

- Rust standalone index 单独计时，不计入 Rust/C++ 单样本比对时间。
- alignment 使用已经存在且兼容的 `.bsi`，`/usr/bin/time -v` 只包围 `bsmap align` 进程。
- 正式性能值至少三轮，报告中位 wall、最坏 RSS 和 page-cache 状态；不能用单次最快值。
- WGBS example1 必须与 C++ 非 header SAM 完整逐行一致，并且 `RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0。
- WGBS example2 要求 Rust p1/p8 完整 SAM 字节一致；C++ PE 既有 signal 6 限制如实保留。
- mm10 RRBS SE 必须与 C++ 2,423 条完整逐行一致；PE 必须保持 4,884 条 P14 golden。

环境：WSL2 Ubuntu，16 个逻辑 CPU，15 GiB 内存，4 GiB swap。源码 worktree 为 `.claude/worktrees/p15-performance`，数据和大型结果位于 D 盘。

## 2. 固定数据与参数

| 场景 | Reference | Reads | 参数 |
|---|---|---|---|
| WGBS example1 SE | `benchmark/data/chr22_tail_1M.fa` | `data/wgbs/ex1_se75_10x/simulated.fastq.gz` | `-s 16 -v 0.08 -I 4 -S 1` |
| WGBS example2 PE | 同上 | `simulated_1.fastq.gz`、`simulated_2.fastq.gz` | `-s 16 -v 0.08 -I 4 -S 1` |
| mm10 RRBS SE | `D:/BSMAP/benchmark-data/mm10/mm10.fa` | `Ctrl_10K_R1.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -S 1` |
| mm10 RRBS PE | 同上 | `Ctrl_10K_R1.fq`、`Ctrl_10K_R2.fq` | 同上 |

关键 SHA256：

- reference mm10：`db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7`
- R1：`13769b68c6f83fe476857ceb2936906ea8e9c0a5737ad00c75245f3e29da40dd`
- RRBS v9 index：`584c354e0725a6e316056e675b576bfba8c4e849b2f18ea7eb13acb652a4482f`
- 当前 release binary：`3e5c49d2cdeaa04a49442a3e6a0ee34663d7b055810f75131a5955e071b77edd`

## 3. Phase 1：WGBS succinct v8 index

v8 用 occupancy bitvector、rank directory、8-byte compact bucket descriptor 和稀疏 overflow table 替代 dense `index2 + start_offsets`。

| 指标 | P14 | P15 v8 |
|---|---:|---:|
| example1 index | 519,037,888 bytes | 13,691,272 bytes |
| example1 p8 RSS | 509,540 KiB | 约 23,000 KiB |
| example1 SAM | 66,120 | 66,120，C++ 完整一致 |

WGBS v7 仍可显式读取，但 cache compatibility 要求 v8 并强制旧索引重建。WGBS 完整 candidate 顺序、随机起点和 SAM 语义未变。

## 4. Phase 2A：worker seed scratch

`SeedSegment` 的三个逐段 `Vec` 改为固定容量内联数组；`SingleAlign` 持有并复用 seed chain 和 segment scratch。两个后续实验未达到门槛并已回退：

- identity hasher：example1 收益不足 5%。
- 预计算 N + 复用 selection set：example1 约回退 12%。

回退后 release binary SHA256 恢复到 Phase 2A 基线，证明没有残留实验代码。

| 场景 | Phase 0 | Phase 2A | 正确性 |
|---|---:|---:|---|
| example1，两次 repeat | 2.23 s | 1.41 s | SHA `7a0203...` 不变 |
| example2 | 1.17 s | 1.02 s | SHA `e73edc...` 不变 |
| mm10 RRBS SE | 13.03 s | 中位 12.59 s | 2,423，SHA `420e34...` |
| mm10 RRBS PE | 14.38 s | 中位 14.70 s | 4,884，SHA `7b33a9...` |

## 5. Phase 4：RRBS single-reference v9

### 5.1 实现

- `.bsi` 只保存 forward 2-bit reference，RRBS `crefcat` section 为空。
- reverse candidate 根据 C++ padded reverse 规则，在固定栈缓冲中按需生成窗口。
- WGBS 继续使用 v8 dual-chain layout；旧 RRBS v8 索引拒绝复用并强制重建。
- `index_sections.py` 支持 v8/v9，校验 mode/version、section 边界、header/count 和 v9 空 `crefcat`。

201 个 lib tests、3 个 bin tests、doc tests、release build 和 14 个 benchmark 工具测试通过。

小型 fixture 使用 `chr22_tail_1M.fa` 和 `ex3_se75_10x.1.fq.gz`：v9 索引 3,768,440 bytes，`crefcat_words=0`；Rust 与 C++ 9,725 条 SAM 完整逐行一致。

### 5.2 mm10 standalone index

命令：

```bash
/usr/bin/time -v -o index.time \
  target/release/bsmap index \
  -d /mnt/d/BSMAP/benchmark-results/p15/phase34-rrbs-v9-20260623/mm10.fa \
  -s 12 -I 4 -D C-CGG
```

| 指标 | P14/v8 | P15/v9 | 变化 |
|---|---:|---:|---:|
| index bytes | 1,773,733,496 | 1,091,007,832 | -38.49% |
| wall | 49.14 s | 46.28 s | -5.82% |
| max RSS | 1,911,532 KiB | 1,911,148 KiB | 基本不变 |
| v9 `crefcat` | 682,725,664 bytes | 0 | 完全移除 |

索引大小达到 `<=1.15 GB` 目标；构建 wall `<=35 s` 和 RSS `<=1.3 GiB` 尚未达到。原因是当前构建器仍在内存中 materialize reverse chain，再在写盘时省略；Phase 8 必须消除该临时对象。

### 5.3 mm10 alignment

使用 `benchmark/p15/run_stream_scale.sh`，`REPEATS=1 THREADS=8`，三轮均为已有索引的 alignment process，`standalone_index_included=false`。

| 场景 | Phase 2A wall | v9 wall 中位 | v9 最坏 RSS | v9 major faults 中位 | 输出 |
|---|---:|---:|---:|---:|---|
| RRBS SE | 12.59 s | 8.61 s | 829,560 KiB | 207,813 | 2,423，SHA `420e34a3...` |
| RRBS PE | 14.70 s | 10.10 s | 878,316 KiB | 244,322 | 4,884，SHA `7b33a9d8...` |

SE raw SAM 与 C++ `20260621T164847Z-610/cpp_se/output.sam` 比较：2,423/2,423 完整逐行一致，`RNAME/POS/FLAG/NM/ZP/ZL` 差异全部为 0。

v9 同时满足 index `<=1.15 GB`、SE RSS `<=1.0 GiB`、PE RSS `<=1.1 GiB`，并使 SE/PE wall 分别改善约 31.6%/31.3%。但 SE `<=5 s`、PE `<=7 s` 和 major faults `<=100,000` 尚未达到。

原始结果：

- `D:/BSMAP/benchmark-results/p15/phase34-rrbs-v9-20260623/index.time`
- `.../index_sections.json`
- `.../se-runs/*/summary.json`
- `.../pe-runs/*/summary.json`
- `.../se-comparison.json`

### 5.4 WGBS 回归

| 场景 | wall | CPU | 最坏 RSS | 结果 |
|---|---:|---:|---:|---|
| example1 p8，三轮中位 | 0.95 s | 99%-103% | 23,536 KiB | 66,120，C++ 完整逐行一致 |
| example2 p1 | 2.19 s | 56% | 31,348 KiB | SHA `e73edc7e...` |
| example2 p8，三轮中位 | 1.46 s | 89%-98% | 32,064 KiB | SHA `e73edc7e...` |

example1 的 `RNAME/POS/FLAG/NM/ZP/ZL` 差异全部为 0；example2 p1/p8 完整文件一致。C++ example2 继续 signal 6，不能提供有效 PE golden。

原始结果：`D:/BSMAP/benchmark-results/p15/phase34-wgbs-v9-regression`。

### 5.5 已回退的 section advice 实验

在 v9 上分别测试 `rrbs_hits=MADV_NORMAL` 和 `rrbs_hits=MADV_WILLNEED`，`refcat` 保持 random。两种实现都保持 2,423 条和 SHA `420e34a3...`，但没有达到综合性能门槛：

| 策略 | wall 中位 | 最坏 RSS | major faults 中位 | 结论 |
|---|---:|---:|---:|---|
| v9 全 RRBS random 基线 | 8.61 s | 829,560 KiB | 207,813 | 保留 |
| hits normal | 9.11 s | 927,984 KiB | 149,159 | wall/RSS 回退，撤销 |
| hits willneed | 9.93 s | 928,144 KiB | 147,022 | wall/RSS 回退，撤销 |

降低 major faults 并未降低端到端 wall，说明 DrvFS 上内核预读把显式 fault 换成了更大的驻留集和 I/O 压力。原始结果位于 `phase3-section-advice` 和 `phase3-willneed`；生产代码已恢复 v9 random 基线。

## 6. 当前未完成项

1. Phase 3：RRBS logical bucket、共享/section-scoped mmap、major faults 和 sys time 优化。
2. Phase 5：bounded 输入/编码/输出流水线、低分配 SAM/BSP、直接 BAM writer。
3. Phase 6：PE pairing 去重复分组和临时分配。
4. Phase 7：mismatch kernel、runtime dispatch、release profile。
5. Phase 8：standalone index wall/RSS 和稳定流式写盘。
6. mm10 WGBS 10K、p1/p2/p4/p8/p16 线程曲线和部署文件系统复核。
7. 90G WGBS / 10G RRBS 正式常数内存、解压、背压和长时间吞吐测试。
8. 最终 completion audit、合并 `main` 和推送。

在以上项目完成前，P15 状态保持“进行中”。
