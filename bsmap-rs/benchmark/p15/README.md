# P15 基准与规模化工具

本目录提供 P15 的机器可复现工具。它们只测已存在索引的 alignment process；Rust standalone index 必须由 `../p14/run_rust_benchmark.sh` 或独立命令单独计时，绝不并入 Rust/C++ 单样本比对时间。

## 工具

- `metrics.py`：解析 GNU `/usr/bin/time -v`，同时识别 signal、外层退出码、RSS、CPU、缺页与上下文切换。
- `index_sections.py`：只读检查 v8 `.bsi` header、marker、section offset/count/bytes 和文件边界。
- `stream_fastq.py`：逐记录解压并重复 FASTQ，直接写入 FIFO；不会生成 90G/10G 中间 FASTQ。
- `slow_sink.py`：常数内存消费 SAM FIFO，可限速以验证输出背压。
- `run_stream_scale.sh`：串起 producer、Rust align、sink、GNU time 和 SHA256，生成单轮 `summary.json`。

## 规模口径

`TARGET_SOURCE_BYTES` 表示原始输入文件的磁盘字节等价量。对于 `.gz`，producer 每轮重新解压，summary 同时记录源文件字节、实际输出的未压缩 FASTQ 字节和 records/pairs。`TARGET_EMITTED_BYTES` 则直接按未压缩 FASTQ 字节停止。两种口径不得混写。

PE 的 R1、R2 由两个独立 producer 流式输出，使用相同 repeat count 并在结束后核对 records 数，避免 reader 按 mate 分批消费时形成 FIFO 互锁。因此 PE 只接受 `TARGET_SOURCE_BYTES` 或 `REPEATS`，不接受需要逐 pair 联动停止的 `TARGET_EMITTED_BYTES`。

90G WGBS SE 示例：

```bash
GIT_COMMIT=<sha> TARGET_SOURCE_BYTES=90G THREADS=8 \
  bash bsmap-rs/benchmark/p15/run_stream_scale.sh \
  /path/to/repo /path/to/reference.fa /path/to/wgbs_R1.fastq.gz /path/to/runs
```

10G RRBS PE 示例：

```bash
GIT_COMMIT=<sha> TARGET_SOURCE_BYTES=10G THREADS=8 \
  SEED_SIZE=12 DIGESTION_SITE=C-CGG READ_2=/path/to/rrbs_R2.fastq.gz \
  bash bsmap-rs/benchmark/p15/run_stream_scale.sh \
  /path/to/repo /path/to/mm10.fa /path/to/rrbs_R1.fastq.gz /path/to/runs
```

输出背压使用相同输入另跑一轮，例如 `SINK_MIB_PER_SEC=25`。`PAGE_CACHE_STATE` 只记录调用方已建立的 `cold`、`warm` 或 `uncontrolled` 状态；runner 不以 root 权限清理 page cache。正式线程扩展曲线对相同 workload 分别设置 `THREADS=1,2,4,8,16`，每个点至少三轮并报告中位 wall、最坏 RSS 和 page-cache 状态。

每个 run 保存 binary/reference/index/read SHA256、完整命令、参数、producer/sink 统计、退出码、GNU time 原文与 JSON。`summary.json` 只有在 align、producer、sink 都成功且 GNU time 未报告 signal 时才标记 `successful=true`。

## 正确性工具

SAM 完整等价继续使用 P14 比较器，不能降级为只比字段：

```bash
python3 bsmap-rs/benchmark/p14/compare_sam.py \
  expected.sam actual.sam \
  --summary comparison.json --field-diff field_diff.tsv
```

WGBS 必须达到非 header 完整记录 100% 一致，且 `RNAME/POS/FLAG/NM/ZP/ZL` 差异均为 0；RRBS SE 必须保持 2,423 条完整记录与 C++ 一致。

## 自测

```bash
cd bsmap-rs/benchmark/p15
python3 -m unittest -v test_tools.py
python3 -m py_compile *.py
bash -n run_stream_scale.sh
```

索引检查：

```bash
python3 index_sections.py /path/to/reference.fa.bsi --output index_sections.json
```
