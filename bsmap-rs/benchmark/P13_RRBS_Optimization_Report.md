# P13 RRBS C++ 等价重构与性能报告

## 结论

P13 已达到本阶段验收标准。最终代码提交为 `365124d`，分支为
`worktree-p13-rrbs-bsc-fix`。

- mm10 RRBS SE：Rust 与 C++ 均输出 2,423 条 mapped records，QNAME 集合、RNAME、POS、FLAG、strand、NM、ZP、ZL 全部 100% 一致。
- 2,423 条无 header SAM records 逐行完全一致；SAM 文件 SHA256 不同仅因 header 中程序信息不同。
- Rust SE 100.60 秒，C++ SE 121.98 秒；同轮 Rust 快 17.5%。
- Rust SE 峰值 RSS 2,165,560 KB，低于 2.2 GB 门槛；较 C++ 1,959,676 KB 高 10.5%。
- FLAG 和染色体分布完全一致；Top1 均为 chr5 172 条，占 7.10%。
- WGBS example1 保持 66,120 mapped，坐标一致率 98.84%；example2 Rust PE 正常输出 66,958 条。
- C++ example2 PE 和 mm10 PE 均退出 134，保留退出码与 stderr，不作为 Rust 失败。

## 代码变化

| 提交 | 变化 | 验证结果 |
| --- | --- | --- |
| `b6e055c` | RRBS hit index 改为 offset/count + flat `Vec<Hit>` | 峰值 RSS 从约 5.34 GB 降到约 2.15 GB |
| `88fc882` | 流式构建 reference 和 RRBS index | 避免全基因组 reference 的重复驻留 |
| `62ff229` | RRBS reverse padding 对齐 C++ | mapped 恢复为 2,423，FLAG/染色体分布恢复 |
| `a7fe6ad` | 使用全局 read index、C++ hit 分层和确定性选择 | 修复 batch 间随机序列重置 |
| `0084257` | SE 在随机取模前排除 reusable index 中的 BSC hit | 随机选择基于 C++ 等价 logical bucket |
| `d82ce8d` | 修正 `myrand` 第二乘数并加入 C++ 固定向量 | RNAME/POS/FLAG/strand 达到 100% |
| `0edefff` | 默认 N 不计入 mismatch，与 C++ `N_mis=0` 一致 | NM 达到 2,423/2,423 一致 |
| `365124d` | 保存 digestion sites、索引升至 v6、输出 ZP/ZL | 2,423 条 SAM records 逐行一致 |

RRBS v6 index 持久化每条染色体排序后的 `(cut_position, reverse_offset)`，旧 RRBS v5
会被兼容性检查拒绝并重建。WGBS v5 仍可读取。

## 环境与数据

- 本地系统：Windows 11 + WSL2 Ubuntu，16 个逻辑 CPU，WSL 可用内存 15 GiB、swap 4 GiB。
- 正式 checkout：`/home/zhang_i5edc0/p13-benchmark-repo`，WSL ext4，`repo_dirty=false`。
- 输入和结果位于 D 盘，避免 OneDrive、C 盘和 WSL VHDX 容量影响。
- 最终结果目录：`D:/BSMAP/benchmark-results/p13/mm10/20260621T164847Z-610`。

| 数据 | 路径 | SHA256 |
| --- | --- | --- |
| mm10 reference | `D:/BSMAP/benchmark-data/mm10/mm10.fa` | `db16cb4633191754f1d9cc70e73d2a1f60d03fdf62bcf4902a31a4717a3d2de7` |
| RRBS R1 10K | `D:/BSMAP/benchmark-data/mm10/Ctrl_10K_R1.fq` | `13769b68c6f83fe476857ceb2936906ea8e9c0a5737ad00c75245f3e29da40dd` |
| RRBS R2 10K | `D:/BSMAP/benchmark-data/mm10/Ctrl_10K_R2.fq` | `839b5c7ca42968a1c5ce65e6ff65beb70c557147922a8674f9672e9c7e5e8a5f` |
| Rust binary | `/home/zhang_i5edc0/p13-benchmark-repo/bsmap-rs/target/release/bsmap` | `6ab2f7da3a7650809f54c498baa050463ed181bf9dee0b82c50e74bd3b1238f8` |
| C++ binary | `/tmp/p13-bsmap-cpp` | `09417edbab04b5552fdd9d3e6a9230b3d22e0660c607781c91c2d13e48bc4da6` |

本地 WGBS reference 和 reads：

- Reference：`bsmap-rs/benchmark/data/chr22_tail_1M.fa`
- Example1：`bsmap-rs/benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz`
- Example2：`bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz` 与 `simulated_2.fastq.gz`

## 编译与测试

最终代码在 WSL2 执行：

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p13-rrbs-bsc-fix/bsmap-rs
set -euo pipefail
cargo check
cargo test
cargo build --release -p bsmap
```

结果：`cargo check` 通过；库测试 192 passed；主程序测试 3 passed；bsp2sam 14 passed；doc tests 1 passed、3 ignored；release build 通过。warning 均为既有 unused import/variable，不含 error 或 failure。

新增业务测试覆盖：RRBS fragment binary search、v6 index roundtrip、SAM ZP/ZL 顺序、C++ RNG 固定向量、logical hit bucket 和默认 N mismatch。

## Benchmark 命令

Example1/Example2：

```bash
bash benchmark/p13/run_local_ex1_ex2.sh step23_rrbs_fragment_tags
```

固定参数：SE/PE 均为 `-s 16 -v 0.08 -I 4 -p 1`。

mm10 RRBS 10K：

```bash
REFERENCE=/mnt/d/BSMAP/benchmark-data/mm10/mm10.fa \
READ_1=/mnt/d/BSMAP/benchmark-data/mm10/Ctrl_10K_R1.fq \
READ_2=/mnt/d/BSMAP/benchmark-data/mm10/Ctrl_10K_R2.fq \
CPP_BINARY=/tmp/p13-bsmap-cpp \
bash bsmap-rs/benchmark/p13/run_docker_mm10.sh \
  /home/zhang_i5edc0/p13-benchmark-repo \
  /mnt/d/BSMAP/benchmark-results/p13/mm10
```

四组公共参数固定为：

```text
-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1
```

runner 每次使用独立结果目录，强制删除临时 reference symlink 对应的 `.bsi`，重建 Rust RRBS index，并记录 commit、dirty 状态、完整命令、退出码、GNU time、SHA256 和 SAM stats。

## Example 回归结果

原始结果：`bsmap-rs/benchmark/p13/runs/step23_rrbs_fragment_tags/local/summary.json`。

| Case | exit | wall | CPU | max RSS | mapped |
| --- | ---: | ---: | ---: | ---: | ---: |
| Example1 C++ SE | 0 | 2.84s | 74% | 872,108 KB | 66,120 |
| Example1 Rust SE | 0 | 16.74s | 23% | 1,462,920 KB | 66,120 |
| Example2 C++ PE | 134 | 1.42s | 95% | 872,104 KB | 0 |
| Example2 Rust PE | 0 | 18.51s | 26% | 1,462,768 KB | 66,958 |

Example1 common QNAME 为 66,120；same RNAME/POS 为 65,353（98.84%）；exact comparison fields 为 65,351（98.84%）。本轮在 OneDrive/DrvFS worktree 运行，wall time 受文件系统影响，主要用于 WGBS 正确性回归，不与 ext4 mm10 绝对耗时混合比较。

## mm10 RRBS 10K 最终结果

原始 summary：`D:/BSMAP/benchmark-results/p13/mm10/20260621T164847Z-610/summary.json`。

| Case | exit | wall | user | sys | CPU | max RSS | mapped |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Rust SE | 0 | 100.60s | 35.31s | 15.14s | 50% | 2,165,560 KB | 2,423 |
| C++ SE | 0 | 121.98s | 55.33s | 7.92s | 51% | 1,959,676 KB | 2,423 |
| Rust PE | 0 | 92.19s | 35.96s | 14.31s | 54% | 2,165,488 KB | 4,884 |
| C++ PE | 134 | 117.15s | 53.61s | 7.97s | 52% | 2,160,684 KB | 0 |

SE SAM 详情：

| 指标 | Rust | C++ |
| --- | ---: | ---: |
| mapped records | 2,423 | 2,423 |
| unique mismatch level | 1,930 | 1,930 |
| multiple mismatch level | 493 | 493 |
| FLAG 0 / 16 / 256 / 272 | 966 / 964 / 251 / 242 | 966 / 964 / 251 / 242 |
| Top RNAME | chr5 172（7.10%） | chr5 172（7.10%） |

逐 QNAME 验证：common 2,423，cpp-only 0，rust-only 0；RNAME/POS 2,423/2,423；strand 2,423/2,423；NM 2,423/2,423；ZP/ZL 2,423/2,423；无 header SAM records 逐行相等 2,423/2,423。

Rust PE 输出 4,884 条 mapped records，Top1 为 chr1 380 条（7.78%）。C++ PE 在输出 SAM 前因既有 buffer overflow 退出 134，因此没有可用 PE 结果集合，不能伪造 parity 结论。

## 性能与验收

| 验收项 | 门槛 | 最终结果 | 状态 |
| --- | --- | --- | --- |
| mm10 SE mapped QNAME | 与 C++ 2,423 一致 | 2,423/2,423 | 通过 |
| 确定性及 secondary 记录 | 至少 99.5% | 100% | 通过 |
| FLAG/染色体分布 | Top1 误差不超过 1 个百分点 | 完全一致 | 通过 |
| Example1 | 66,120 mapped，坐标至少 98.8% | 66,120，98.84% | 通过 |
| Rust SE wall | 不慢于 C++ | 快 17.5% | 通过 |
| Rust SE RSS | 不超过 2.2 GB | 2,165,560 KB | 通过 |
| Rust PE | 正常完成 | 4,884 records，exit 0 | 通过 |

## 已知限制

1. C++ BSMAP 2.90 PE 在 example2 和 mm10 10K 上退出 134；这是 C++ 基线限制，P13 不修改原版二进制。
2. Rust mm10 SE 的峰值 RSS 比 C++ 高约 10.5%，虽已满足 2.2 GB 门槛，仍是后续可优化空间。
3. CPU 利用率约 50%，主要受 mm10 reference、v6 index 和结果位于 `/mnt/d` DrvFS 的 I/O 限制；跨机器或跨文件系统不直接比较绝对 wall time。
4. P13 已完成 RRBS SE 语义等价和本阶段性能门槛；更广泛参数组合、其他 digestion motifs 与 C++ PE 的修复不属于本阶段完成声明。
