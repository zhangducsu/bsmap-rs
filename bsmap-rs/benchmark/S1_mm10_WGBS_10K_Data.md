# S1 mm10 WGBS 10K 测试数据

## 1. 目的

原有 WGBS example1/example2 使用 1 Mb `chr22_tail_1M.fa`，对 S1 的 pipeline、PE pairing、I/O 和完整 mm10 大索引访问优化压力不足。S1 将基于完整 mm10 reference 的 WGBS 10K fixture 作为正式 WGBS 短基准数据：

- mm10 WGBS SE 75 bp，10,000 reads。
- mm10 WGBS PE 150 bp，10,000 pairs。

旧 `chr22_tail_1M.fa` example1/example2 只保留为快速 smoke 或历史回归参考，不再作为 S1 的主要 WGBS 验收数据。生成数据不提交到 Git，固定保存在本机 D 盘 benchmark 数据目录。

## 2. 生成工具

- 工具：Sherman WGBS simulator。
- 使用路径：`/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/tools/sherman/Sherman`。
- Sherman SHA256：`9bd24c0bf680d549abee88f6a51b9282c0fc60875d92d8fd850a1d89ba579bc8`。
- 注意：S1 worktree 中 `bsmap-rs/tools/sherman` 是 gitlink 空壳；本次使用主工作区已展开的 Sherman clone。
- Sherman 未提供显式 random seed 参数，因此 fixture 以生成后的 FASTQ SHA256 固定；后续不得假设重新运行脚本会得到相同 FASTQ 内容。

## 3. Reference

- Reference：`/mnt/d/BSMAP/benchmark-data/mm10/mm10.fa`
- SHA256：`db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7`
- 大小：约 2.6 GB。

## 4. 生成参数

SE：

```bash
Sherman \
  --genome_folder /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/genome \
  -l 75 \
  -n 10000 \
  -cr 99.0 \
  -o /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/se75_10k
```

PE：

```bash
Sherman \
  --genome_folder /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/genome \
  -l 150 \
  -n 10000 \
  -pe \
  -I 70 \
  -X 400 \
  -cr 99.0 \
  -o /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k
```

压缩使用 `gzip -n`，避免 gzip 时间戳污染压缩文件 SHA。

## 5. 输出文件

| 数据 | 路径 | records | SHA256 |
|---|---|---:|---|
| SE 75 bp | `/mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/se75_10k/simulated.fastq.gz` | 10,000 | `4f3866f8c6b41aef28c9b88a0052cccf77e78c8ff1be6b411fa013ea69b60f38` |
| PE 150 bp R1 | `/mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k/simulated_1.fastq.gz` | 10,000 | `79dd975dd889af5ea0423cb853715ffa78c27d59663161cf914c5028b2930a05` |
| PE 150 bp R2 | `/mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k/simulated_2.fastq.gz` | 10,000 | `f923271573a441d2811eddd7150559c588fa1c01bc2e483253c0c3d48c1c882a` |

本次 metadata：`/mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/metadata.tsv`。

## 6. 验证

```bash
gzip -cd /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/se75_10k/simulated.fastq.gz | wc -l
gzip -cd /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k/simulated_1.fastq.gz | wc -l
gzip -cd /mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k/pe150_10k/simulated_2.fastq.gz | wc -l
```

结果均为 `40000` 行，即每个 FASTQ 文件 10,000 条记录。

## 7. S1 benchmark 口径

正式 WGBS 短基准：

- mm10 WGBS SE 10K：`-s 16 -v 0.08 -I 4 -p 8 -S 1`
- mm10 WGBS PE 10K：`-s 16 -v 0.08 -I 4 -p 8 -S 1`

正式 WGBS 长程基准统一改为 5G：

- mm10 WGBS PE 5G：基于 `pe150_10k` fixture 流式 repeat，使用 `TARGET_SOURCE_BYTES=5G`。
- 若后续增加 WGBS SE 长程基准，也必须使用 5G 数据口径，不再使用 90G。

Rust standalone WGBS index 必须单独计时，不并入 Rust/C++ 单样本 align 时间比较。若 C++ 在完整 mm10 WGBS 10K 上可成功输出，则 S1 后续应增加 Rust/C++ 的 `RNAME/POS/FLAG/NM/ZP/ZL` 与完整记录一致性检查；若 C++ 失败，必须如实记录退出码和 stderr。
