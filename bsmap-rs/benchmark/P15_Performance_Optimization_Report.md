# P15 速度、内存与 CPU 效率优化报告

> 状态：本轮交付报告。代码优化、单元测试、example/WGBS/RRBS 等价验证、线程矩阵和 RRBS 10G 长测已完成；WGBS 90G 长测已启动并验证常数内存趋势，后按用户要求中止，不再作为本轮完成阻塞项。

## 1. 测试口径

- Rust standalone index 单独计时，不计入 Rust/C++ 单样本比对时间。
- Rust alignment 只统计已有兼容 `.bsi` 的 `bsmap align` 进程，`standalone_index_included=false`。
- C++ BSMAP 2.90 没有等价的可复用 standalone index 接口；C++ 单样本 baseline 保留原始 invocation 行为，不人工扣除其内部 reference 准备成本。
- WGBS example1 必须与 C++ 非 header SAM 完整逐行一致，并且 `RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0。
- WGBS example2 要求 Rust p1/p8 完整 SAM 字节一致；C++ PE 既有 signal 6/buffer overflow 限制如实保留。
- mm10 RRBS SE 必须与 C++ 2,423 条完整逐行一致；PE 必须保持 P14/P13 golden 4,884 条。
- 正式性能值优先使用 WSL2 ext4 路径；DrvFS 结果只作为 Windows 文件系统限制记录。

环境：WSL2 Ubuntu，16 个逻辑 CPU，15 GiB 内存，4 GiB swap。源码 worktree 为 `.claude/worktrees/p15-performance`。大数据位于 `D:/BSMAP/benchmark-data/mm10`，正式性能复核时复制/硬链接到 WSL ext4。

当前可复现提交与二进制：

- benchmark/report/tooling commit：`6e63b42`
- P15 v10 code commit：`e98cc8e`
- release binary：`/home/zhang_i5edc0/p15-binaries/bsmap-6e63b42-a30c6f8d`
- binary SHA256：`a30c6f8de30435c5cba032601d4391c5a86e3e0ab48bab6fde654220d83a6299`

## 2. 固定数据与参数

| 场景 | Reference | Reads | 参数 |
|---|---|---|---|
| WGBS example1 SE | `benchmark/data/chr22_tail_1M.fa` | `data/wgbs/ex1_se75_10x/simulated.fastq.gz` | `-s 16 -v 0.08 -I 4 -S 1` |
| WGBS example2 PE | 同上 | `simulated_1.fastq.gz`、`simulated_2.fastq.gz` | `-s 16 -v 0.08 -I 4 -S 1` |
| mm10 RRBS SE | `/home/zhang_i5edc0/p15-mm10-v10-forward-only/mm10.fa` | `Ctrl_10K_R1.fq` | `-s 12 -v 0.08 -I 4 -D C-CGG -S 1` |
| mm10 RRBS PE | 同上 | `Ctrl_10K_R1.fq`、`Ctrl_10K_R2.fq` | 同上 |
| RRBS 10G PE | 同上 | 同上重复到 `TARGET_SOURCE_BYTES=10G` | `THREADS=16 SINK_MIB_PER_SEC=50` |
| WGBS 90G PE | `/home/zhang_i5edc0/p15-wgbs-v8-ext4/reference.fa` | example2 R1/R2 重复到 `TARGET_SOURCE_BYTES=90G` | `THREADS=16 SINK_MIB_PER_SEC=50` |

关键 SHA256：

- mm10 reference：`db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7`
- mm10 RRBS R1：`13769b68c6f83fe476857ceb2936906ea8e9c0a5737ad00c75245f3e29da40dd`
- mm10 RRBS R2：`839b5c7ca42968a1c5ce65e6ff65beb70c557147922a8674f9672e9c7e5e8a5f`
- RRBS v10 index：`d7afbc84f9111428df3a469071a4eb83239b54e7581465089ca9972fa8ddf1a2`
- WGBS v8 example index：`a9bd5b5a55327263144c8b61d73c2bbe78bad74927249b6f163b256777deff87`

## 3. 保留的优化

### 3.1 WGBS succinct v8 index

v8 用 occupancy bitvector、rank directory、8-byte compact bucket descriptor 和稀疏 overflow table 替代 dense `index2 + start_offsets`。

| 指标 | P14 | P15 v8 |
|---|---:|---:|
| example1 index | 519,037,888 bytes | 13,691,272 bytes |
| example1 p8 RSS | 509,540 KiB | 约 23,000 KiB |
| example1 SAM | 66,120 | 66,120，C++ 完整一致 |

WGBS candidate 顺序、随机起点和 SAM 语义未变。WGBS v7 仍可显式读取；默认 cache compatibility 要求 v8 并强制旧索引重建。

### 3.2 Worker seed scratch

`SeedSegment` 的三个逐段 `Vec` 改为固定容量内联数组；`SingleAlign` 持有并复用 seed chain 和 segment scratch。两个后续实验未达到门槛并已回退：

- identity hasher：example1 收益不足 5%。
- 预计算 N + 复用 selection set：example1 约回退 12%。

### 3.3 RRBS single-reference v9

v9 先消除 RRBS `.bsi` 中的 reverse `crefcat` section，reverse candidate 根据 C++ padded reverse 规则在固定栈缓冲中按需生成窗口。

| 指标 | P14/v8 | P15/v9 | 变化 |
|---|---:|---:|---:|
| mm10 index bytes | 1,773,733,496 | 1,091,007,832 | -38.49% |
| standalone index wall | 49.14 s | 46.28 s | -5.82% |
| standalone index max RSS | 1,911,532 KiB | 1,911,148 KiB | 基本不变 |
| RRBS `crefcat` | 682,725,664 bytes | 0 | 完全移除 |

v9 已满足 index `<=1.15 GB`、SE RSS `<=1.0 GiB`、PE RSS `<=1.1 GiB`，但构建 RSS 仍保留 reverse chain 临时对象，因此进入 v10。

### 3.4 RRBS packed v10 + forward-only build

v10 将 RRBS hit 从 unpacked `Hit { loc: u32, chr: u32 }` 压缩为 7 bytes：

- `loc`: u32
- `block`: u16，等价 C++ 低 16 位 chr/block
- `mode`: 7 bit
- `RRBS_BSC_FLAG`: 1 bit

构建器改为 forward-only：不再 materialize 全局 reverse `crefcat` 和 reverse seed index；reverse seed 由 forward chain 按需推导。v10 `.bsi` 与旧 v9/v8 RRBS index 不兼容，旧缓存会拒绝并重建。

小型 RRBS fixture：

- reference：`chr22_tail_1M.fa`
- reads：`ex3_se75_10x.1.fq.gz`
- v10 index：3,598,688 bytes，v9 为 3,768,440 bytes
- Rust/C++：9,725 条 SAM 完整逐行一致

mm10 standalone index：

```bash
/usr/bin/time -v -o index.time \
  bsmap index -d /home/zhang_i5edc0/p15-mm10-v10-forward-only/mm10.fa \
  -s 12 -I 4 -D C-CGG --verbose 1
```

| 指标 | P15 v9 | P15 v10 forward-only | 结果 |
|---|---:|---:|---|
| index bytes | 1,091,007,832 | 1,041,871,696 | 达到 `<=1.15 GB` |
| wall | 46.28 s | 33.86 s | 达到 `<=35 s` |
| max RSS | 1,911,148 KiB | 1,278,988 KiB | 达到 `<=1.3 GiB` |
| major faults | DrvFS 20 万级 | 45 | ext4 正常 |
| index SHA | v9 不同 | `d7afbc84...` | v10 double-chain 与 forward-only 一致 |

原始结果：`/home/zhang_i5edc0/p15-mm10-v10-forward-only/index.time`。

## 4. 正确性验证

### 4.1 WGBS

WGBS example1 与 C++ 非 header SAM 完整逐行一致，字段差异全为 0：

| 对比 | records | full exact | RNAME | POS | FLAG | NM | ZP | ZL |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Rust example1 p8 vs C++ | 66,120 | 66,120 | 0 | 0 | 0 | 0 | 0 | 0 |

WGBS example2：Rust p1/p8 三轮输出 SHA 均为 `e73edc7e7327524028c61bb4a1eed14b8428eedfe3dc1e8be5a22cc334f313ca`。C++ example2 PE 继续 signal 6/buffer overflow，因此不能作为有效 PE golden。

原始结果：

- `D:/BSMAP/benchmark-results/p15/phase4-v10-wgbs-regression/ex1-comparison.json`
- `D:/BSMAP/benchmark-results/p15/phase4-v10-wgbs-regression/ex2-p1/summary.json`
- `D:/BSMAP/benchmark-results/p15/phase4-v10-wgbs-regression/ex2-runs/*/summary.json`

### 4.2 mm10 RRBS

RRBS SE raw SAM 与 C++ `20260621T164847Z-610/cpp_se/output.sam` 比较：2,423/2,423 完整逐行一致，`RNAME/POS/FLAG/NM/ZP/ZL` 差异全部为 0。

RRBS PE 保持 P14/P13 golden：4,884 条，SHA `7b33a9d894f670e1ec2424430d614d06c2d4d2d48a06fc4880c0568949f39ac6`。p1 与 p8 输出 SHA 一致，证明 PE 线程调度不影响结果。

原始结果：

- `/home/zhang_i5edc0/p15-mm10-v10-forward-only/se-comparison.json`
- `/home/zhang_i5edc0/p15-mm10-v10-forward-only/v10-pe-p1.sam`
- `/home/zhang_i5edc0/p15-mm10-v10-forward-only/pe-single-decode/*/summary.json`

## 5. 性能结果

### 5.1 mm10 RRBS 10K 线程矩阵

所有 run 都使用已有 v10 `.bsi`，不计 standalone index。

RRBS SE：15 runs，SAM 2,423 条，SHA `420e34a3fa39086effbff8341cde5bacf90fde9bf57a32b39e0cb48eeedd9ad0`。

| threads | median wall | median CPU | worst RSS | worst major faults | speedup vs p1 |
|---:|---:|---:|---:|---:|---:|
| 1 | 7.56 s | 98% | 1,010,040 KiB | 52 | 1.00x |
| 2 | 3.72 s | 192% | 1,010,168 KiB | 53 | 2.03x |
| 4 | 1.91 s | 365% | 1,010,356 KiB | 55 | 3.96x |
| 8 | 1.07 s | 668% | 1,010,268 KiB | 58 | 7.07x |
| 16 | 0.72 s | 1198% | 1,010,788 KiB | 56 | 10.50x |

RRBS PE：15 runs，SAM 4,884 条，SHA `7b33a9d894f670e1ec2424430d614d06c2d4d2d48a06fc4880c0568949f39ac6`。

| threads | median wall | median CPU | worst RSS | worst major faults | speedup vs p1 |
|---:|---:|---:|---:|---:|---:|
| 1 | 13.47 s | 99% | 1,006,532 KiB | 52 | 1.00x |
| 2 | 7.11 s | 195% | 1,006,916 KiB | 53 | 1.89x |
| 4 | 3.95 s | 377% | 1,006,940 KiB | 56 | 3.41x |
| 8 | 2.04 s | 727% | 1,007,308 KiB | 57 | 6.60x |
| 16 | 1.36 s | 1355% | 1,008,180 KiB | 57 | 9.90x |

结论：RRBS 对线程扩展敏感，p16 仍有收益；v10 packed hit 与 ext4 mmap 将 major faults 压到约 50 到 60。

### 5.2 WGBS example 线程矩阵

WGBS example1 SE：15 runs，SAM 66,120 条，SHA `dddb945e533dce90c7f7f17a884a4e633006ddf057fbc6f065ca7f83c139d296`。

| threads | median wall | median CPU | worst RSS | speedup vs p1 |
|---:|---:|---:|---:|---:|
| 1 | 1.18 s | 66% | 23,412 KiB | 1.00x |
| 2 | 0.84 s | 83% | 23,028 KiB | 1.40x |
| 4 | 0.71 s | 103% | 23,176 KiB | 1.66x |
| 8 | 0.65 s | 127% | 23,460 KiB | 1.82x |
| 16 | 0.67 s | 167% | 24,352 KiB | 1.76x |

WGBS example2 PE：15 runs，SAM 66,958 条，SHA `e73edc7e7327524028c61bb4a1eed14b8428eedfe3dc1e8be5a22cc334f313ca`。

| threads | median wall | median CPU | worst RSS | speedup vs p1 |
|---:|---:|---:|---:|---:|
| 1 | 1.55 s | 72% | 31,456 KiB | 1.00x |
| 2 | 1.13 s | 94% | 31,428 KiB | 1.37x |
| 4 | 0.96 s | 113% | 31,772 KiB | 1.61x |
| 8 | 0.93 s | 138% | 32,020 KiB | 1.67x |
| 16 | 0.92 s | 164% | 33,180 KiB | 1.68x |

结论：小 reference WGBS 的 p8/p16 收益有限，瓶颈主要在输入解压、SAM 输出和固定调度成本；p16 不回退，但实际部署可按样本规模选择 p8 或 p16。

### 5.3 RRBS PE 10G 常数内存长测

命令：

```bash
GIT_COMMIT=6e63b42 REPO_DIRTY=false \
RUST_BINARY=/home/zhang_i5edc0/p15-binaries/bsmap-6e63b42-a30c6f8d \
TARGET_SOURCE_BYTES=10G THREADS=16 \
READ_2=/home/zhang_i5edc0/p15-mm10-v9-ext4/Ctrl_10K_R2.fq \
SEED_SIZE=12 INDEX_INTERVAL=4 DIGESTION_SITE=C-CGG \
RANDOM_SEED=1 MISMATCH_RATE=0.08 SINK_MIB_PER_SEC=50 \
PAGE_CACHE_STATE=warm-ext4 \
bash benchmark/p15/run_stream_scale.sh \
  <repo> /home/zhang_i5edc0/p15-mm10-v10-forward-only/mm10.fa \
  /home/zhang_i5edc0/p15-mm10-v9-ext4/Ctrl_10K_R1.fq \
  /home/zhang_i5edc0/p15-mm10-v10-forward-only/scale-rrbs-pe-10g
```

结果：

| 指标 | 值 |
|---|---:|
| run id | `20260623T014057Z.haY1f9` |
| target source bytes | 10G |
| cycles | 1,356 |
| read pairs | 13,560,000 |
| emitted FASTQ bytes | 10,002,593,664 |
| SAM records | 6,622,704 |
| SAM bytes through FIFO sink | 2,638,754,361 |
| wall | 1,806.98 s |
| user / sys | 27,051.96 s / 44.46 s |
| CPU | 1499% |
| max RSS | 1,014,524 KiB，0.968 GiB |
| major faults | 0 |
| sink SHA256 | `a6113672089e67a22b01f6dbdfe4a710586f778fcb55c272dbc60d2e89e2fa81` |
| exit codes | align=0，producer=0，sink=0 |

原始结果：`/home/zhang_i5edc0/p15-mm10-v10-forward-only/scale-rrbs-pe-10g/20260623T014057Z.haY1f9/summary.json`。

该长测证明 RRBS PE 在 10G 源数据规模下保持常数内存；RSS 与 10K p16 矩阵基本一致，没有随输入规模线性增长。

### 5.4 WGBS PE 90G 常数内存检查

本轮原计划跑完整 `TARGET_SOURCE_BYTES=90G`，后按用户要求“不跑了，就按照目前的结果完成吧”中止。因此该项不是完整成功长测，不生成 `summary.json`，只作为常数内存趋势证据记录。

- run id：`20260623T021531Z.4Qud3s`
- target：`TARGET_SOURCE_BYTES=90G`
- command：`THREADS=16 SINK_MIB_PER_SEC=50 -s 16 -v 0.08 -I 4 -S 1`
- reference：`/home/zhang_i5edc0/p15-wgbs-v8-ext4/reference.fa`
- reads：`/home/zhang_i5edc0/p15-wgbs-v8-ext4/ex2-r1.fastq.gz`、`ex2-r2.fastq.gz`

中止前 25 分钟检查点：

- `bsmap` PID 1324 已运行 25:22，CPU 约 181%，RSS 33,624 KiB。
- producer PID 1322 已从源 gzip 读取约 17.42 GB，向 FIFO 写入约 109.21 GB 解压 FASTQ。
- `bsmap` 已从 FIFO 读取约 109.21 GB，并向 SAM FIFO 写入约 62.97 GB。
- sink PID 1321 仍正常消费 SAM FIFO，RSS 约 23 MiB。
- `bsmap`、producer、sink 在中止前均无 stderr 错误；中止后确认无残留 `bsmap-6e63b42`、`run_stream_scale.sh`、`stream_fastq.py` 或 `slow_sink.py` 进程。

结论：WGBS PE 在超过 100 GB 解压 FASTQ 流量下仍保持几十 MiB RSS，证明当前 FIFO producer/align/sink 路径没有随输入规模增长的内存泄漏。由于该 run 是人工中止，不能作为完整 90G wall/SHA 结果；完整 90G 长测可作为 P16 或后续 overnight benchmark 复跑。

## 6. 已回退或不保留的实验

| 实验 | 现象 | 结论 |
|---|---|---|
| RRBS `rrbs_hits=MADV_NORMAL` | SE major faults 从 207,813 降到约 149,159，但 wall 从 8.61 s 退化到 9.11 s，RSS 从 829,560 KiB 增至 927,984 KiB | 回退 |
| RRBS `rrbs_hits=MADV_WILLNEED` | wall 退化到 9.93 s，RSS 约 928 MiB | 回退 |
| identity hasher | example1 收益不足 5% | 不保留 |
| 预计算 N + 复用 selection set | example1 回退约 12% | 不保留 |

这些结果已写入 `AGENTS.md`，后续不得只因缺页下降或局部 micro benchmark 改善就保留端到端回退。

## 7. 交付状态与后续项

本轮 P15 保留的代码优化已经通过既有验证：

- `cargo check -p bsmap`、`cargo test -p bsmap`、`cargo build --release -p bsmap` 通过。
- benchmark 工具 `python3 -m py_compile benchmark/p15/*.py`、`python3 -m unittest benchmark/p15/test_tools.py` 和 shell 语法检查通过。
- WGBS example1 与 C++ 完整逐行一致；WGBS example2 Rust p1/p8 完整字节一致。
- mm10 RRBS SE 与 C++ 2,423 条完整逐行一致；RRBS PE 保持 4,884 条 golden，p1/p8 deterministic。
- RRBS PE 10G 完整长测成功，证明大规模 RRBS 常数内存。
- WGBS PE 90G 完整 wall/SHA 未完成，原因是用户明确要求停止长测并按当前结果交付；该限制已在本报告中显式保留。

建议后续 P16 若继续追求极限性能，优先方向不是再改 RRBS 索引布局，而是 WGBS/RRBS 共用的输入解压、低分配 SAM/BAM writer、PE pairing 临时分配和更细的 NUMA/cache profiling。
