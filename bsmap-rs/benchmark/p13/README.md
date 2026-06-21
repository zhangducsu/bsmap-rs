# P13 benchmark 工具

本目录保留两类互不影响的 runner：

- `run_local_ex1_ex2.sh`：现有本地 WGBS example1/example2 回归入口，调用方式和结果目录保持不变。
- `run_docker_mm10.sh`：mm10 RRBS 10K 可复现 benchmark 入口；保留 Docker 默认路径，也支持本地 WSL2 路径覆盖。

## mm10 runner

runner 默认把 Git checkout 固定为 `/tmp/p13_codex_github`：

```bash
cd /tmp/p13_codex_github
bash bsmap-rs/benchmark/p13/run_docker_mm10.sh
```

如 checkout 不在默认位置，可把仓库根目录作为第一个参数；第二个参数可指定结果根目录：

```bash
bash bsmap-rs/benchmark/p13/run_docker_mm10.sh /path/to/checkout /path/to/runs
```

若二进制不在仓库默认位置，可通过环境变量覆盖：

```bash
CPP_BINARY=/workspace/03_project/bsmap-2.90/bsmap \
  bash bsmap-rs/benchmark/p13/run_docker_mm10.sh /path/to/checkout /path/to/runs
```

runner 不负责 clone、build 或服务器连接，只在当前 Docker 环境验证已构建 binary。固定输入与 binary 为：

| 项目 | 路径 |
| --- | --- |
| Reference | `/workspace/00_data/reference/mm10.fa` |
| R1 | `/workspace/00_data/rrbs/Ctrl_10K_R1.fq` |
| R2 | `/workspace/00_data/rrbs/Ctrl_10K_R2.fq` |
| Rust | `<repo>/bsmap-rs/target/release/bsmap` |
| C++ | `<repo>/bsmap-original/bsmap-2.90/bsmap` |

`REFERENCE`、`READ_1`、`READ_2`、`RUST_BINARY` 和 `CPP_BINARY` 均可通过环境变量覆盖。P13 的后续 mm10 10K 基准默认在本地 WSL2 执行；只有输入文件 SHA256 与正式报告一致时，结果才能与既有服务器基线直接比较。

本地调用示例：

```bash
REFERENCE=/mnt/d/BSMAP/benchmark-data/mm10/mm10.fa \
READ_1=/mnt/d/BSMAP/benchmark-data/mm10/Ctrl_10K_R1.fq \
READ_2=/mnt/d/BSMAP/benchmark-data/mm10/Ctrl_10K_R2.fq \
bash bsmap-rs/benchmark/p13/run_docker_mm10.sh \
  <repo> /mnt/d/BSMAP/benchmark-results/p13/mm10
```

四组命令固定为 Rust/C++ 的 SE/PE，公共参数固定为：

```text
-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1
```

每组命令使用结果目录内的 `work/mm10.fa` 软链接。runner 仅删除该临时路径对应的 `work/mm10.fa.bsi`，不会删除或覆盖 `/workspace/00_data/reference/mm10.fa.bsi`。每次执行创建带 UTC 时间和 PID 的独立目录，不复用旧输出。

## 结果

每个 case（`rust_se`、`cpp_se`、`rust_pe`、`cpp_pe`）保存：

- `command.txt`：实际执行命令。
- `exit_code.txt`：原始退出码。
- `time.txt`：GNU time 的 wall、user、system、CPU 和最大 RSS。
- `output.sam`、`stdout.txt`、`stderr.txt`：原始产物。

根目录中的 `sha256.tsv` 保存 reference、reads、两个 binary 和四个 SAM 的 SHA256；`comparisons/{se,pe}.json` 来自 `sam_stats.py`；`summary.json` 汇总 commit、dirty 状态、命令、退出码、time、SHA256 和 SAM stats。结构示例见 `result_schema.example.json`。

所有 benchmark 子进程都独立记录状态。C++ PE 返回 134 时 runner 仍会执行 SAM stats 和最终汇总；runner 只在输入缺失或汇总基础设施失败时返回非零。
