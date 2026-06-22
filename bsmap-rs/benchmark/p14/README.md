# P14 基准工具

本目录提供最小的 SAM 等价性比较器，以及 Rust standalone index / warm align 分段计时骨架。工具不包含服务器地址或固定数据路径。

## SAM 比较口径

`compare_sam.py` 分别跳过两个 SAM 的 header，然后按原始记录顺序逐条比较。完整行只移除行尾的 LF、CRLF 或 CR；不会排序记录、按 QNAME 聚合、重排 optional tags、忽略字段或修剪其他空白。

完整等价必须同时满足：

- 两侧非 header 记录数相同；
- 每个相同序号的完整记录完全一致。

对完整行不一致的成对记录，`field_diff.tsv` 额外报告 `RNAME`、`POS`、`FLAG`、`NM`、`ZP`、`ZL` 差异。optional tag 以 `TYPE:VALUE` 比较，缺失值写作 `<MISSING>`。MAPQ、CIGAR、SEQ、QUAL、其他 tag 或 tag 顺序的差异仍会导致完整行不一致，但不会伪装成上述六个字段的差异。

```bash
python3 bsmap-rs/benchmark/p14/compare_sam.py \
  expected.sam actual.sam \
  --summary comparison.json \
  --field-diff field_diff.tsv
```

完全一致时退出码为 `0`，存在任何完整行或记录数差异时为 `1`，参数或运行错误由 Python 返回非零退出码。JSON summary 保存记录数、完整行一致数、未配对记录数，以及六个字段各自的差异计数。

## Rust 分段计时

runner 接收本地路径，不负责 clone、build、下载输入或连接服务器：

```bash
bash bsmap-rs/benchmark/p14/run_rust_benchmark.sh \
  /path/to/repo /path/to/reference.fa /path/to/read_1.fq /path/to/results
```

默认按 WGBS SE 运行：`SEED_SIZE=16`、`INDEX_INTERVAL=4`、`THREADS=1`、`RANDOM_SEED=1`、`MISMATCH_RATE=0.08`，并连续执行三次 warm process。RRBS 示例：

```bash
SEED_SIZE=12 DIGESTION_SITE=C-CGG THREADS=8 RANDOM_SEED=1 \
READ_2=/path/to/read_2.fq \
bash bsmap-rs/benchmark/p14/run_rust_benchmark.sh \
  /path/to/repo /path/to/reference.fa /path/to/read_1.fq /path/to/results
```

可通过 `RUST_BINARY` 覆盖默认的 `<repo>/bsmap-rs/target/release/bsmap`，通过 `WARM_RUNS` 调整重复次数。WSL 无法解析 Windows linked-worktree 的 `.git` 指针时，必须显式提供 `GIT_COMMIT=<commit>`；不得把未知提交写成可复现结果。每次执行创建独立 run 目录，并严格拆分：

1. `standalone_index`：只计时 `bsmap index`，索引写入本次 run 的 `work/reference.fa.bsi`。
2. `warm_align_1..N`：确认上述索引已存在后，分别计时完整 warm process，包括索引加载、读取 reads、比对和写 SAM；每轮结束后校验索引 SHA256 未变化，防止自动重建混入计时。

每个阶段保存 `command.txt`、`exit_code.txt`、GNU `/usr/bin/time -v` 的 `time.txt`、`stdout.txt` 和 `stderr.txt`。根目录的 `sha256.tsv` 记录 binary、reference、reads、index 和 SAM 的 SHA256；`metadata.tsv` 记录 commit、dirty 状态、参数和索引大小。

runner 不清理 OS page cache。正式性能结论使用 warm 三轮 wall time 的中位数，并将首次 OS 冷启动单独标注；Rust standalone index 数据不得并入与 C++ 单样本 invocation 的比对耗时。

`fixtures/wgbs_pe_one_pair_R1.fastq` 和 `fixtures/wgbs_pe_one_pair_R2.fastq` 是从 example2 固化的最小 PE 输入。原版 C++ BSMAP 2.90 对这一对 reads 仍触发 buffer overflow；运行前必须设置 `ulimit -c 0`。Rust 应正常退出，且 `-p 1/-p 8` 输出必须逐字节一致。

## 自测

```bash
cd bsmap-rs/benchmark/p14
python3 -m unittest -v test_compare_sam.py
python3 -m py_compile compare_sam.py test_compare_sam.py
bash -n run_rust_benchmark.sh
```
