# P12 RRBS 索引修复与稳定化报告

## 结论

P12 当前目标是稳定化与合并准备，不继续扩展 RRBS 新方案。`codex/p12-rrbs-stabilize` 本地已经恢复可构建状态，Docker 端完成 mm10 RRBS 索引强制重建和 10K subset 验证。Rust RRBS 输出不再出现 99% chr1 偏斜。

需要如实保留的限制：

- Docker 端验证使用容器内已有 release binary：`/workspace/03_project/01_bsmap-rs/target/release/bsmap`，sha256 为 `8cd86b0764d89e253787cd12ba5606971f1956f90081a21968874d9fcbcd2bdd`。
- 容器内源码 `/workspace/03_project/01_bsmap-rs` 当前不是本地 Codex 分支的完整同步副本，`cargo check -p bsmap` 仍有 16 个编译错误；因此 Docker 端结果用于真实数据行为验证，不替代本地源码构建验证。
- C++ BSMAP 2.90 单端对照成功；双端对照在 10K PE 上触发 `buffer overflow detected`，输出 SAM 为空，不能作为有效 PE 对照。

## 本地基线

本地环境：WSL2，目录 `/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs`。

| 命令 | 状态 |
| --- | --- |
| `cargo check` | 通过，仅 warning |
| `cargo test` | 通过，仅 warning |
| `cargo build --release -p bsmap` | 通过，仅 warning |
| `ex1_small` 冒烟测试 | 通过，生成 `.bsi` 和 SAM，`sam_records=10`，`index_size=6398344` |

本地修复重点：

- `reference/index_io.rs`：从 `.bsi` 元数据恢复 `KmerIndex.seed_size`，避免加载缓存索引后丢失 seed size。
- `pairs/output.rs`：测试断言改为匹配 C++ BSMAP 行为，QNAME 去除 `/1`、`/2` 后缀。
- `align/mismatch.rs`：测试断言改为匹配当前 C/T 容忍语义。

## Docker 验证环境

服务器通过 MCP 访问，所有写入与删除均发生在 Docker 容器 `1c1402862f8d` 内。

验证目录：`/tmp/p12_codex_validate`

参考与输入：

- 参考：`/workspace/00_data/reference/mm10.fa`，66 条序列，2,730,871,774 bp。
- 验证用参考路径：`/tmp/p12_codex_validate/mm10.fa`，指向真实 FASTA 的符号链接。
- 强制重建索引：删除并重建 `/tmp/p12_codex_validate/mm10.fa.bsi`，未删除原始 `/workspace/00_data/reference/mm10.fa.bsi`。
- reads：`/workspace/00_data/rrbs/Ctrl_10K_R1.fq`、`/workspace/00_data/rrbs/Ctrl_10K_R2.fq`。
- Rust binary：`/workspace/03_project/01_bsmap-rs/target/release/bsmap`，mtime `2026-05-26 10:02`。
- C++ binary：`/workspace/03_project/bsmap-2.90/bsmap`，mtime `2026-05-23 18:03`。

核心参数：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8`。

## RRBS 索引重建

| 项目 | 结果 |
| --- | --- |
| 命令 | `bsmap index -d /tmp/p12_codex_validate/mm10.fa -s 12 -I 4 -D C-CGG` |
| 退出码 | 0 |
| 耗时 | 0:49.37 |
| 最大 RSS | 5,338,020 KB |
| 索引文件 | `/tmp/p12_codex_validate/mm10.fa.bsi` |
| 索引大小 | 1.7 GB |

索引日志显示 v2 索引保存成功：`refcat=85340708 words, crefcat=85340708 words`。

## 10K subset 结果

| 样本 | SAM 记录数 | mapped | unique/mapq255 | multiple/mapq 1-254 | Top chr | Top chr 占比 | 耗时 | 最大 RSS |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: |
| Rust SE | 2,124 | 2,124 | 2,124 | 0 | chr10 | 8.33% | 0:30.58 | 5,338,380 KB |
| Rust PE | 4,199 | 4,199 | 4,199 | 0 | chr1 | 12.29% | 0:26.80 | 5,338,264 KB |
| C++ SE | 2,423 | 2,423 | 2,423 | 0 | chr2 | 7.47% | 1:07.74 | 1,957,992 KB |
| C++ PE | 0 | 0 | 0 | 0 | NA | 0.00% | 1:11.94 | 2,118,832 KB |
| C++ PE, `-p 1` | 0 | 0 | 0 | 0 | NA | 0.00% | 1:13.26 | 2,118,828 KB |

Rust SE/PE 的染色体分布没有回退到 chr1 极端偏斜。Rust PE 的 top chromosome 是 chr1，但占比仅 12.29%，不属于 P12 修复前的 99% chr1 问题。

C++ SE 对照成功，aligned reads 为 2,423。C++ PE 在 `-p 8` 和 `-p 1` 下均触发：

```text
*** buffer overflow detected ***: terminated
```

因此本轮不能得到有效 C++ PE 对照输出。

## 与 P12 目标的关系

P12 已验证的稳定化目标：

- RRBS mm10 索引可在 Docker 容器内强制重建。
- Rust RRBS 10K SE/PE 输出非空。
- Rust RRBS 染色体分布不再出现 99% chr1 偏斜。
- 本地 `cargo check`、`cargo test`、`cargo build --release -p bsmap` 已恢复通过。

仍需保留的未解决项：

- RRBS `seed_size=12` 仍有性能瓶颈，尤其是候选位置遍历压力大。
- Rust/C++ SE 比对率仍有差异：本轮 Rust SE 2,124，C++ SE 2,423。
- Docker 容器源码未同步到本地 Codex 分支，不能把容器源码编译状态视为 P12 源码状态。
- C++ BSMAP 2.90 PE 对照在本轮数据上崩溃，P12 报告只能使用 C++ SE 做有效对照。
- P13 的 BSC flag / chain 坐标方案仍需在 P12 合入后继续验证。

## 提交与合并建议

提交前必须清理 staged 范围，只纳入 P12 代码和本报告。不要纳入：

- `.codex/`
- `.agents/`
- `.claude/worktrees/`
- 私钥
- `wget-log`
- 临时输出
- cargo 乱码输出文件
- Docker 验证临时目录输出

建议拆分：

1. `fix: stabilize P12 RRBS index loading and tests`
2. `docs: update P12 RRBS validation report`

P12 合入 `main` 后，再让 P13 worktree 基于最新 `main` rebase/merge。P13 只比较 BSC flag / chain 坐标方案是否优于 P12，不重新解决 P12 已经稳定的问题。
