# SSH1 RRBS 服务器基准

SSH1 runner 用于重新测量服务器 Docker 内 mm10 RRBS 10K/full 数据。Rust standalone index 必须单独完成，本 runner 只测已有 `.bsi` 的 warm align，并在运行前后校验 index SHA。

默认只跑 10K SE/PE，避免误启动长任务：

```bash
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

跑全量时显式指定：

```bash
SSH1_CASES="full_se full_pe" \
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

固定 RRBS 参数：

```text
-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1
```

输出包含 `metadata.tsv`、每个 case 的 `time.json`/`sam_stats.json`、Rust/C++ 字段 diff 和总 `summary.json`。

RRBS 热路径诊断需要显式开启，开启后只用于定位瓶颈，不作为正式性能基线。
full run 优先使用低开销阶段计时：

```bash
SSH1_PROFILE_RRBS=stage SSH1_CASES="full_se" \
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

10K 或抽样数据可使用详细候选计数：

```bash
SSH1_PROFILE_RRBS=1 SSH1_CASES="10k_se" \
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

开启后 Rust case 的 `stderr.txt` 会包含 `BSMAP_PROFILE_RRBS key=value`，包括
read/prepare/align/write 阶段耗时；`SSH1_PROFILE_RRBS=1` 还会包含 RRBS
candidate/mismatch/hit 计数。`summary.json` 会把这些值汇总到
`cases.<case>.rrbs_profile`。正式 Rust/C++ wall time 对比应使用
`SSH1_PROFILE_RRBS=0` 的 warm run；`stage` 只用于拆分 full run 慢点。
