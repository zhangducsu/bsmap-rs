# SSH2 RRBS 生产级优化报告

## 目标判定

SSH2 完成标准：

- Rust 与 C++ 使用完全相同参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1`，抽样时双方增加相同 `-E N`。
- Rust standalone index 不计入与 C++ 单样本 align 时间比较。
- Rust/C++ SAM 字段 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` diff 为 0。
- Rust RSS 低于或相当于 C++。
- Rust wall time 小于等于 C++ wall time 的 50%。

## SSH2 起点

基线分支：`codex/ssh2-rrbs-production-optimization`，从 SSH1 `d7373f8` 创建。

SSH1 已知结果：

| 场景 | Rust | C++ | 判定 |
|---|---:|---:|---|
| 10K SE mapped | 2,423 | 2,423 | 字段 diff 为 0 |
| 10K SE Rust stage | 1.41 s | 66.77 s | 10K 受 C++ normal invocation 固定成本影响，不代表 full |
| full SE wall | 3,778.00 s | 536.04 s 旧基线 | Rust 慢约 7 倍 |
| full SE RSS | 913,116 KiB | 约 2.87 GiB | Rust 内存明显更低 |

SSH2 不再接受 10K 单次噪声作为性能结论；新增 100K/1M 中等抽样用于筛选优化。

## 新增工具

- `benchmark/ssh2/run_server_rrbs_subset.sh`
  - 输入：full RRBS R1 `/workspace/00_data/rrbs/Ctrl_R1.fq.gz`
  - read range：通过 `SSH2_LIMITS` 设置，例如 `10000 100000 1000000`
  - 参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1 -E <limit>`
  - Rust：使用已有 `.bsi` warm index
  - C++：normal invocation
  - 输出：metadata、binary/input SHA、time/RSS/CPU、SAM stats、streaming field diff、summary JSON

## Baseline 结果

运行命令：

```bash
SSH2_LIMITS="10000 100000" \
bash bsmap-rs/benchmark/ssh2/run_server_rrbs_subset.sh \
  /tmp/ssh1_sparse_20260627T153127Z_68025/repo \
  /workspace/benchmark_results/ssh2
```

运行路径：`/workspace/benchmark_results/ssh2/20260627T164000Z-73428/summary.json`。

| limit | Rust wall | Rust RSS KiB | C++ wall | C++ RSS KiB | Rust/C++ wall | SAM diff | 判定 |
|---:|---:|---:|---:|---:|---:|---|---|
| 10,000 | 1.41 s | 893,488 | 65.93 s | 2,057,220 | 0.021 | streaming diff 0 | 通过 |
| 100,000 | 10.44 s | 911,620 | 76.20 s | 2,117,748 | 0.137 | streaming diff 受输出顺序影响；sorted multiset 仅 2 条真实差异 | 未通过 correctness gate |

100K 进一步用排序后的 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` multiset 比较：

- C++ records：24,236
- Rust records：24,236
- exact multiset records：24,234
- C++ only records：2
- Rust only records：2
- C++ only QNAME：0
- Rust only QNAME：0

这说明 100K 的大面积 streaming diff 主要来自输出顺序不同，但仍有 2 条真实 C++ 语义差异。SSH2 下一步必须先定位这 2 条差异，再继续速度优化。

## 优化日志

### 2026-06-28：SSH2 基线准备

- 从 SSH1 `d7373f8` 新建 SSH2 分支。
- 新增 SSH2 计划文档和 subset runner。
- 目标从“改善”提升为明确生产门槛：Rust full SE wall `<= C++ / 2`，且 RSS 不高于或相当于 C++。
- 服务器 10K/100K subset baseline 已完成。10K 完全一致；100K mapped 数一致但存在 2 条 sorted multiset 差异，correctness gate 尚未通过。

## 未解决项

- 100K 中存在 2 条真实记录差异，需定位是随机多重命中、早停、候选 bucket 还是输出选择差异。
- 尚未重跑 SSH2 1M baseline；应等 100K correctness 清零后再作为性能筛选主基准。
- full SE 的 C++ 最新 wall/RSS 仍需 SSH2 runner 复测，不能只沿用旧 `536.04s`。
- Rust full SE 与 C++ full SE 旧结果存在 `+124` mapped 差异，SSH2 full acceptance 前必须用 streaming diff 复核并解释。
