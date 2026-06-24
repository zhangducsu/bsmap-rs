# SSH1 RRBS 服务器优化报告

## 结论

2026-06-24 服务器复测显示，Rust RRBS 10K SE 在 warm index 下已达到 C++ 的 2,423 条 mapped，但全量 SE 仍有两个问题：一是 Rust warm align 约 3,975.52 秒，明显慢于 C++ normal invocation 的 536.04 秒；二是 Rust full SE 比 C++ 多 124 条 mapped，FLAG/RNAME 分布也有偏移。因此 SSH1 先修基准口径和字段级一致性，再做速度优化。

Rust standalone index 不计入与 C++ 单样本 align 时间比较。后续所有 SSH1 数字必须使用已有 v10 `.bsi` 的 warm Rust run，并记录 index SHA 前后一致。

## 已复盘的服务器结果

| 场景 | Rust | C++ | 判定 |
|---|---:|---:|---|
| RRBS 10K SE mapped | 2,423 | 2,423 | 数量一致，但旧 runner 未完全固定参数，需 SSH1 runner 复测字段 diff |
| RRBS 10K SE wall | 1.29 s | 71.46 s | Rust 为 warm index；C++ normal invocation 含内部参考/索引成本，不直接等价 |
| RRBS full SE mapped | 8,873,078 | 8,872,954 | Rust 多 124 条，需优先定位 |
| RRBS full SE wall | 3,975.52 s | 536.04 s | Rust 约慢 7.4 倍，是 SSH1 主要性能问题 |
| RRBS 10K PE | Rust 4,884 records | C++ signal 6 | C++ PE buffer overflow，只记录失败 |
| RRBS full PE | Rust perf 缺失 | C++ signal 6 | 当前结果不能作为性能基准 |

旧 runner 的主要问题：

- Rust 10K PE 首次运行包含 v10 `.bsi` 重建，不能作为 align 时间。
- full PE 缺少完整 `.perf` 和结束日志，不能作为有效性能结果。
- C++ PE 未设置 `ulimit -c 0`，产生多个 core 文件。
- 参数未全部显式固定为 `-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1`。

## SSH1 已实现改动

- 新增 `benchmark/ssh1/run_server_rrbs.sh`，固定服务器 RRBS benchmark 入口。
- 新增 streaming SAM 工具，避免 full SAM 对比时把 800 万级记录全部装入内存。
- runner 运行前后校验 Rust `.bsi` SHA，确保 standalone index 不混入 align 计时。
- runner 开头执行 `ulimit -c 0`，防止 C++ PE crash 继续生成 core。
- 默认只跑 10K；full run 必须显式设置 `SSH1_CASES="full_se full_pe"` 或 `SSH1_CASES=all`。
- RRBS runtime normal-count 缓存作为第一轮速度优化：不改 `.bsi` 格式，只在内存中缓存每个 RRBS bucket 排除 `RRBS_BSC_FLAG` 后的 normal hit 数，减少 SE seed 计数和 logical bucket 长度计算的重复扫描。

## 后续验证命令

本地：

```bash
cd bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
python3 -m py_compile benchmark/ssh1/*.py
```

服务器 Docker：

```bash
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

全量复测：

```bash
SSH1_CASES="full_se full_pe" \
bash bsmap-rs/benchmark/ssh1/run_server_rrbs.sh \
  /workspace/02_software/bsmap-rs \
  /workspace/benchmark_results/ssh1
```

## 验收标准

- 10K RRBS SE：Rust/C++ mapped 均为 2,423，`QNAME/RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。
- full RRBS SE：Rust mapped、unique/multiple、FLAG/RNAME 分布与 C++ 对齐；若仍有差异，报告必须包含字段 diff 样例。
- Rust full SE 第一阶段目标：wall time 相对 3,975.52 秒降低至少 30%，RSS 不高于 1.2 GiB。
- PE 只有在 `.perf`、退出码、结束日志完整时才进入性能表。

## 未完成项

- SSH1 runner 需要在 Docker 内重新跑 10K，确认参数和字段 diff 已固定。
- full SE 需要用 SSH1 runner 重跑，旧 full SE 只能作为诊断基线。
- 若 normal-count 缓存收益不足，下一步继续定位 mismatch/extend 候选数量、SAM 输出和 gzip/read 阶段耗时。
