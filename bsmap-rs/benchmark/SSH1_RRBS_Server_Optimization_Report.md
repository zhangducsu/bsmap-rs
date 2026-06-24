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

## SSH1 10K 复测结果

运行路径：`/workspace/benchmark_results/ssh1/20260624T071833Z-6712`。

固定参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1`。

index SHA 前后均为 `1329966ddda5aedd9fc7e13cb84a4e755cd632df3d14a0de32a239a29561e634`，说明 Rust align 未重建或改写 `.bsi`。

| 场景 | exit | wall | CPU | RSS KiB | SAM records | mapped | Top RNAME | 字段 diff |
|---|---:|---:|---:|---:|---:|---:|---|---|
| Rust 10K SE | 0 | 1.38 s | 641% | 893,176 | 2,423 | 2,423 | chr5 7.0986% | 与 C++ 完全一致 |
| C++ 10K SE | 0 | 69.68 s | 100% | 2,056,740 | 2,423 | 2,423 | chr5 7.0986% | 基准 |
| Rust 10K PE | 0 | 2.76 s | 756% | 845,128 | 4,884 | 4,884 | chr1 7.7805% | 与 C++ 不一致 |
| C++ 10K PE | 0 | 77.03 s | 100% | 2,192,168 | 4,884 | 4,884 | chr5 7.2686% | 基准 |

10K SE 的 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` diff 全部为 0，满足 SSH1 正确性门槛。

10K PE 的 record 数相同，但字段差异较大：`RNAME=572`、`POS=2774`、`FLAG=987`、`NM=87`、`ZP=4884`、`ZL=4884`。PE 后续必须单独处理，不能用 SE 等价结论外推。

normal-count 缓存在 10K SE 上没有体现明显短测收益；当前 10K wall 主要仍受一次性 index mmap/cache 初始化和 RRBS 扩展热路径影响。该优化是否对 full SE 有摊薄收益，需要以 full SE runner 结果判定。

## SSH1 full SE 进展

运行路径：`/workspace/benchmark_results/ssh1/20260624T073756Z-7884`。

Rust full SE 已完成：

| 场景 | exit | wall | user | sys | CPU | RSS KiB | SAM size | 结论 |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Rust full SE | 0 | 3,778.00 s | 28,436.85 s | 17.63 s | 753% | 913,116 | 3.5G | 比旧 3,975.52 s 约快 5.0%，未达到 30% 目标 |

这个结果说明 runtime normal-count 缓存没有解决 full SE 主瓶颈。RSS 仍稳定在约 0.87 GiB，没有内存回退；CPU 利用率约 7.5 核，说明问题不是线程空转，而是每条 read 的 RRBS candidate / extend / mismatch 热路径工作量过高。

C++ full SE 和 full SE streaming diff 已由同一 runner 继续执行；最终 C++ wall/RSS 和字段 diff 仍待回收后补入。

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

- full SE 已在 Docker 后台启动，完成后需要补入最终 wall/RSS/字段 diff；旧 full SE 只能作为诊断基线。
- 若 normal-count 缓存收益不足，下一步继续定位 mismatch/extend 候选数量、SAM 输出和 gzip/read 阶段耗时。
