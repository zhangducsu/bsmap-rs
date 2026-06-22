# P14 C++ 等价与全路径性能优化报告

## 1. 结论

P14 在代码提交 `b82ec2319f8f2be61127af368117943f6b87f763` 上完成验收：

- WGBS example1 SE 的 66,120 条非 header SAM 与 C++ BSMAP 2.90 逐行完全一致；`RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0。
- mm10 RRBS 10K SE 的 2,423 条 SAM 与 C++ 逐行完全一致；FLAG、染色体分布和全部 optional tags 一致。
- WGBS example2 PE 与 mm10 RRBS PE 的 Rust `-p 1/-p 8` 输出分别字节一致；原版 C++ 两项均因 buffer overflow 收到 signal 6，不能提供有效 PE golden。
- mm10 RRBS standalone index 为 49.14 秒、1.823 GiB RSS、1,773,733,496 bytes，达到 75 秒、2.0 GiB、1.78 GB 门槛。
- mm10 RRBS warm SE 三次 wall time 为 13.36/12.39/12.09 秒，中位数 12.39 秒；最坏 RSS 1,309,692 KiB（1.249 GiB），达到 25 秒和 1.3 GiB 门槛。
- alignment core 的 8 线程加速比为：WGBS SE 3.25 倍、WGBS PE 3.15 倍，达到 2.0/1.5 倍门槛。

Rust 的 standalone index 始终单独计时，未计入 Rust 与 C++ 的单样本比对耗时比较。

## 2. 实现摘要

1. 对齐 C++ WGBS seed 调度、combined hit bucket 环形遍历和 padded reverse reference，消除 example1 最后 769 条差异。
2. v7 索引改为固定 section directory 和 raw aligned sections；reference chains、WGBS hash/positions、RRBS offsets/hits/sites 使用 mmap，旧 v7 bincode 布局及 v1-v6 缓存强制重建。
3. align 启动先校验 `.bsi` metadata，不再为了验证缓存而扫描完整 FASTA；metadata 覆盖 source size/mtime、mode、seed、interval、k-mer ratio、RRBS insert 范围和 digestion sites。
4. RRBS standalone index 将 FASTA 编码与 digestion 合并为单遍；seed origin 删除重复 block id，使用紧凑 position 表。
5. `EncodedRead` 改为四组 `[u64; FIXELEMENT]` 和最小元数据，不再为每条 read 分配四个 `Vec` 或克隆完整 `ReadInf`。
6. SE 使用 indexed Rayon 和 worker-local `SingleAlign`；PE 在同一 Rayon task 内完成双端比对和 pairing，删除两份完整中间 hit 集合。
7. RRBS mmap 使用 `MADV_RANDOM` 抑制大基因组随机访问的顺序预读；WGBS 保持默认 advice，避免小参考性能回退。

## 3. 环境与资产

- 环境：Windows 11 + WSL2 Ubuntu，16 logical CPUs，15 GiB RAM，4 GiB swap。
- Rust binary：`bsmap-rs/target/release/bsmap`
  - SHA256：`E55405180AE48D0C03ED5BDCF3F733DFAD9F22DC778135AD26F1425F4BDD5AF5`
- C++ binary：`bsmap-original/bsmap-2.90/bsmap`
  - SHA256：`09417EDBAB04B5552FDD9D3E6A9230B3D22E0660C607781C91C2D13E48BC4DA6`

| 资产 | 路径 | SHA256 |
|---|---|---|
| WGBS reference | `bsmap-rs/benchmark/data/chr22_tail_1M.fa` | `E5BDD01F47504F51F3EF3E8CA132F741389D383A17D06D85EA04AB568618F267` |
| example1 SE | `bsmap-rs/benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz` | `3D129582FD7AA5EDC1EFA11B9025E71D010B35CCBE4D90327A57CCF15B6E66B4` |
| example2 R1 | `bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz` | `3B31FA20EF03EC10ACD4140013A0C9E9897119F111439DE88A172A118D73A9FB` |
| example2 R2 | `bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz` | `F05A8A4C353DACDD89C378A1866BB7CDC959C2600893A8174F8E7B521A66D735` |
| mm10 reference | `D:/BSMAP/benchmark-data/mm10/mm10.fa` | `DB16CB4633191754F1D9CC70E73D2A1F60D03FDF62BCF4902A31A4717A3D2DE7` |
| mm10 R1 | `D:/BSMAP/benchmark-data/mm10/Ctrl_10K_R1.fq` | `13769B68C6F83FE476857CEB2936906EA8E9C0A5737AD00C75245F3E29DA40DD` |
| mm10 R2 | `D:/BSMAP/benchmark-data/mm10/Ctrl_10K_R2.fq` | `839B5C7CA42968A1C5CE65E6FF65BEB70C557147922A8674F9672E9C7E5E8A5F` |
| mm10 v7 index | `D:/BSMAP/benchmark-results/p14/mm10-final/reference.fa.bsi` | `0F30DB9DADFE8775240A10603D132B744AB144D58EFEC5C197F36C46CA268E1D` |

## 4. 性能结果

### 4.1 standalone RRBS index

| 指标 | P14 | 门槛 | 结果 |
|---|---:|---:|---|
| wall | 49.14 s | <= 75 s | 通过 |
| user/sys | 20.81/6.84 s | 记录项 | 已记录 |
| CPU | 56% | 记录项 | 已记录 |
| max RSS | 1,911,532 KiB（1.823 GiB） | <= 2.0 GiB | 通过 |
| index size | 1,773,733,496 bytes | <= 1.78 GB | 通过 |

原始结果：`D:/BSMAP/benchmark-results/p14/mm10-final/index.time`。

### 4.2 WGBS example1 SE

| 实现 | threads | wall | CPU | max RSS | records |
|---|---:|---:|---:|---:|---:|
| Rust warm | 1 | 5.20 s | 34% | 509,688 KiB | 66,120 |
| Rust warm | 8 | 2.00 s | 76% | 509,540 KiB | 66,120 |
| C++ normal invocation | 8 | 1.46 s | 117% | 872,184 KiB | 66,120 |

- Rust alignment core：4.597862 秒降至 1.412760 秒，加速 3.25 倍。
- Rust 比 C++ wall 慢 0.54 秒，但峰值 RSS 低 41.6%。该速度差异如实保留，不作为“更快”宣称。

### 4.3 WGBS example2 PE

| 实现 | threads | wall | CPU | max RSS | records | 状态 |
|---|---:|---:|---:|---:|---:|---|
| Rust warm | 1 | 5.46 s | 35% | 518,044 KiB | 66,958 | 成功 |
| Rust warm | 8 | 2.39 s | 81% | 518,580 KiB | 66,958 | 成功 |
| C++ normal invocation | 8 | 1.49 s | 101% | 872,136 KiB | 0 | signal 6 |

- Rust alignment core：4.521849 秒降至 1.437912 秒，加速 3.15 倍。
- Rust p1/p8 SAM SHA256 相同。
- C++ stderr：`*** buffer overflow detected ***`；`/usr/bin/time` 记录 `Command terminated by signal 6`，输出 SAM 为 0 bytes。

### 4.4 mm10 RRBS 10K

| 实现/模式 | wall | CPU | max RSS | SAM records | 状态 |
|---|---:|---:|---:|---:|---|
| Rust SE warm #1 | 13.36 s | 111% | 1,309,000 KiB | 2,423 | 成功 |
| Rust SE warm #2 | 12.39 s | 118% | 1,309,692 KiB | 2,423 | 成功 |
| Rust SE warm #3 | 12.09 s | 120% | 1,309,544 KiB | 2,423 | 成功 |
| Rust PE warm | 13.84 s | 138% | 1,401,320 KiB | 4,884 | 成功 |
| C++ SE normal invocation | 121.98 s | 51% | 1,959,676 KiB | 2,423 | 成功 |
| C++ PE normal invocation | 117.15 s | 52% | 2,160,684 KiB | 0 | signal 6 / 134 |

Rust SE warm 中位数为 12.39 秒，相对 C++ 单样本 invocation 快 9.85 倍，峰值 RSS 低 33.2%。C++ 2.90 没有可复用 standalone index 接口，因此表中 C++ 时间包含其正常 invocation 内部建索引；Rust 的 49.14 秒 standalone index 不计入单样本 wall time，并已单独列出。

## 5. SAM 详情与等价性

### WGBS example1

- 完整记录：66,120/66,120 一致。
- mapped/unmapped：66,120/0。
- Rust 统计：64,951 unique，1,169 multiple。
- FLAG：`0=32298, 16=32653, 256=612, 272=557`，Rust/C++ 完全一致。
- RNAME：`chr22_tail_1M=66120`。
- `RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0；WGBS 两侧均不输出 `ZP/ZL`。

### mm10 RRBS SE

- 完整记录：2,423/2,423 一致。
- Rust 统计：1,930 unique，493 multiple。
- FLAG：`0=966, 16=964, 256=251, 272=242`，Rust/C++ 完全一致。
- Top RNAME：`chr5=172 (7.10%)`；chr1 为 154，不存在 chr1 偏斜。
- `RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0。

### RRBS PE

- Rust：4,884 records；1,968 aligned pairs，其中 1,474 unique、494 multiple。
- Top RNAME：`chr1=380 (7.78%)`，分布正常。
- Rust p1/p8 SHA256 相同；C++ PE 无有效 SAM 可比较。

比较结果：

- `D:/BSMAP/benchmark-results/p14/wgbs-final/final-ex1-comparison.json`
- `D:/BSMAP/benchmark-results/p14/mm10-final/final-se-comparison.json`
- `D:/BSMAP/benchmark-results/p14/wgbs-final/final-ex1-sam-stats.json`
- `D:/BSMAP/benchmark-results/p14/mm10-final/final-se-sam-stats.json`

## 6. 关键命令

```bash
# 编译测试
cd bsmap-rs
cargo check
cargo test
cargo build --release -p bsmap

# standalone RRBS index，单独计时
/usr/bin/time -v bsmap index \
  -d reference.fa -s 12 -I 4 -D C-CGG

# Rust mm10 warm SE，已有 reference.fa.bsi
/usr/bin/time -v bsmap align \
  -a Ctrl_10K_R1.fq -d reference.fa -o rust_se.sam \
  -s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1

# C++ mm10 SE 正常单样本 invocation
/usr/bin/time -v bsmap-cpp \
  -a Ctrl_10K_R1.fq -d mm10.fa -o cpp_se.sam \
  -s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1

# WGBS example1
bsmap align -a simulated.fastq.gz -d chr22_tail_1M.fa \
  -o rust.sam -s 16 -v 0.08 -I 4 -p 8 -S 1

# 完整记录比较
python3 benchmark/p14/compare_sam.py cpp.sam rust.sam \
  --summary comparison.json --field-diff field-diff.tsv
```

可复现 runner：`bsmap-rs/benchmark/p14/run_rust_benchmark.sh`。默认将 standalone index 与三次 warm align 分开计时，并校验 warm 阶段没有重建索引。

## 7. 验收矩阵

| 验收项 | 证据 | 状态 |
|---|---|---|
| `cargo check/test/release` | 199 tests，全部通过 | 通过 |
| WGBS example1 完整记录 100% | 66,120/66,120 | 通过 |
| WGBS PE fixture C++ 等价 | C++ signal 6，无有效 golden | 外部基线阻塞，已记录 |
| WGBS example2 Rust 线程确定性 | p1/p8 SHA256 相同 | 通过 |
| RRBS SE 完整记录 100% | 2,423/2,423 | 通过 |
| RRBS PE 线程确定性 | 4,884，p1/p8 SHA256 相同 | 通过 |
| warm SE <= 25 s | 中位 12.39 s | 通过 |
| warm SE RSS <= 1.3 GiB | 最坏 1.249 GiB | 通过 |
| standalone index <= 75 s | 49.14 s | 通过 |
| standalone RSS <= 2.0 GiB | 1.823 GiB | 通过 |
| index <= 1.78 GB | 1.7737 GB | 通过 |
| SE core p8 >= 2x | 3.25x | 通过 |
| PE core p8 >= 1.5x | 3.15x | 通过 |

## 8. 未解决项

- 原版 C++ BSMAP 2.90 在 WGBS example2 PE、最小一对 PE fixture 和 mm10 RRBS PE 上均触发 buffer overflow/signal 6；P14 不修改 C++ 基线，因此无法证明 PE 的 Rust/C++ 完整记录等价。
- WGBS example1 上 Rust warm wall time 仍比 C++ normal invocation 慢 0.54 秒，但内存显著更低。后续优化应以 profiling 证据为前提，不能牺牲已达到的逐行等价。
- `/usr/bin/time` 的 `Maximum resident set size (kbytes)` 实际按 KiB 解释；报告保留原始数值并同时给出 GiB，避免 GB/GiB 混用。
