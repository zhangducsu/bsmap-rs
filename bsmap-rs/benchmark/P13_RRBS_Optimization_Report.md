# P13 RRBS C++ 等价重构报告

## 结论

本轮 P13 已完成一个可保留的小步：把 RRBS 的索引、seed 调度和扩展过滤改向 C++ BSMAP 2.90 的语义，而不是继续修补旧 Rust 输出坐标。当前本地状态已经通过 `cargo check`、`cargo test`、`cargo build --release -p bsmap`，并完成 example1/example2 的 Rust 与 C++ 基准。

服务器 mm10 RRBS 10K 尚未得到有效 P13 结果。原因不是测试失败，而是容器内 `/workspace/03_project/01_bsmap-rs` 不是当前 P13 worktree 的代码架构；MCP hydrossh 当前只提供命令执行，没有文件上传能力，无法把本地 P13 源码或 release binary 同步到容器。为避免污染结论，本报告不把容器旧 Rust binary 的结果当成本轮 P13 结果。

## 本轮代码变更

本轮只改 RRBS 相关路径，WGBS 只作为回归保护：

- `bsmap/src/reference/rrbs.rs`
  - `find_sites()` 按 C++ `RefSeq::find_CCGG` 从 offset 1 开始搜索。
  - `build_rrbs_index()` 从 flatten 的 `[BSW, BSC]` 改为 `mode -> chain -> positions`。
  - 每个 mode/chain 只保留 C++ 等价的第一个 seed 位置。
- `bsmap/src/reference/index.rs`
  - 新增 RRBS hit 编码常量：低 16 位保存 block id，16 位以上保存 mode，bit 24 保存 BSC/read-chain 交叉标记。
  - RRBS 建索引时按 mode 和 chain 分桶写入 hit，不再把所有 mode 混到同一 chain 列表。
- `bsmap/src/align/seed.rs`
  - RRBS 固定 `start_offset=0`。
  - RRBS seed 调度按 C++ mode segment 生成候选，并显式保留原始 `modeindex`。
  - RRBS 不走 WGBS 的 seed 起点调整。
- `bsmap/src/align/extend.rs`
  - RRBS 扩展前按 C++ 条件过滤候选：
    `((hit.chr ^ (read_chain << 24)) >> 16) == cmodeindex`。
  - `read_chain=1` 时使用 `cmodeindex = read_len / seed_size - 1 - modeindex`。
  - mismatch 使用 `hit.chr & 0xffff` 对应的 reference block，再转换为 SAM 坐标。
- `bsmap/src/reference/index_io.rs`
  - RRBS `.bsi` 编码升级为 v3。
  - 旧 RRBS `.bsi` 会被判定不兼容并强制重建；WGBS v2 缓存保持兼容。

## 本地验证

工作区：

```bash
/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p13-rrbs-bsc-fix/bsmap-rs
```

执行命令：

```bash
cargo check
cargo test
cargo build --release -p bsmap
```

结果：

| 项目 | 结果 |
| --- | --- |
| `cargo check` | 通过，只有 warning |
| `cargo test` | 通过，lib 167 个测试、main 3 个测试、bsp2sam 14 个测试均通过 |
| `cargo build --release -p bsmap` | 通过，只有 warning |

## 本地 example 基准

执行命令：

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p13-rrbs-bsc-fix
bash bsmap-rs/benchmark/p13/run_local_ex1_ex2.sh step8_cpp_equiv_rrbs_mode
```

输入与参数：

| 项目 | 内容 |
| --- | --- |
| Reference | `bsmap-rs/benchmark/data/chr22_tail_1M.fa` |
| Example1 SE reads | `bsmap-rs/benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz` |
| Example2 PE reads | `bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz`, `simulated_2.fastq.gz` |
| Example1 参数 | `-s 16 -v 0.08 -I 4 -p 1` |
| Example2 参数 | `-s 16 -v 0.08 -I 4 -p 1` |
| C++ binary | `bsmap-original/bsmap-2.90/bsmap` |
| C++ sha256 | `09417edbab04b5552fdd9d3e6a9230b3d22e0660c607781c91c2d13e48bc4da6` |
| Rust binary | `bsmap-rs/target/release/bsmap` |
| Rust sha256 | `db0a376438f3eb8433583ef9e387675823e7b427784cc211a2698c26f7a393bb` |
| 原始结果目录 | `bsmap-rs/benchmark/p13/runs/step8_cpp_equiv_rrbs_mode/local` |

### Example1 SE

| 指标 | C++ BSMAP | Rust BSMAP |
| --- | ---: | ---: |
| SAM records | 66,120 | 66,120 |
| mapped | 66,120 | 66,120 |
| unmapped | 0 | 0 |
| unique MAPQ255 | 66,120 | 66,120 |
| multiple MAPQ 1-254 | 0 | 0 |
| Top RNAME | `chr22_tail_1M` | `chr22_tail_1M` |
| Top RNAME 占比 | 100.00% | 100.00% |
| wall time | 0:03.02 | 0:12.82 |
| user time | 1.68s | 1.23s |
| sys time | 0.90s | 1.98s |
| CPU | 85% | 25% |
| max RSS | 872,232 KB | 1,463,708 KB |

一致性：

| 指标 | 结果 |
| --- | ---: |
| common QNAME | 66,120 |
| C++ only QNAME | 0 |
| Rust only QNAME | 0 |
| exact record match | 65,342 |
| same RNAME/POS | 65,346 |
| exact match pct | 98.82% |
| same RNAME/POS pct | 98.83% |

FLAG 分布：

| FLAG | C++ | Rust |
| ---: | ---: | ---: |
| 0 | 32,298 | 32,301 |
| 16 | 32,653 | 32,656 |
| 256 | 605 | 596 |
| 272 | 564 | 567 |

### Example2 PE

C++ BSMAP 在本地 example2 PE 上退出码为 134，stderr 包含：

```text
*** buffer overflow detected ***: terminated
```

因此 example2 只能记录 C++ 失败与 Rust 单侧结果，不伪造 PE 一致率。

| 指标 | C++ BSMAP | Rust BSMAP |
| --- | ---: | ---: |
| 退出码 | 134 | 0 |
| SAM records | 0 | 66,958 |
| mapped | 0 | 66,958 |
| unmapped | 0 | 0 |
| unique MAPQ255 | 0 | 66,958 |
| Top RNAME | NA | `chr22_tail_1M` |
| wall time | 0:01.36 | 0:14.36 |
| user time | 0.88s | 1.99s |
| sys time | 0.56s | 2.36s |
| CPU | 105% | 30% |
| max RSS | 872,176 KB | 1,463,504 KB |

Rust FLAG 分布：

| FLAG | Rust |
| ---: | ---: |
| 147 | 16,404 |
| 99 | 16,403 |
| 83 | 10,160 |
| 163 | 10,159 |
| 81 | 6,331 |
| 129 | 6,327 |
| 355 | 323 |
| 403 | 323 |
| 339 | 256 |
| 419 | 256 |
| 385 | 10 |
| 337 | 6 |

## 服务器 mm10 RRBS 10K 状态

服务器访问路径已按项目规则执行：

```text
MCP hydrossh -> rx2-huqi -> 175.178.251.44:10096 -> docker exec vscode-ssh2
```

只读确认 Docker 容器：

```text
1c1402862f8d vscode-ssh2 Up 2 weeks
```

计划使用的数据与参数：

| 项目 | 内容 |
| --- | --- |
| Reference | `/workspace/00_data/reference/mm10.fa` |
| RRBS R1 | `/workspace/00_data/rrbs/Ctrl_10K_R1.fq` |
| RRBS R2 | `/workspace/00_data/rrbs/Ctrl_10K_R2.fq` |
| Rust SE 参数 | `align -a Ctrl_10K_R1.fq -d mm10.fa -o rust_se.sam -s 12 -v 0.08 -I 4 -D C-CGG -p 8` |
| Rust PE 参数 | `align -a Ctrl_10K_R1.fq -b Ctrl_10K_R2.fq -d mm10.fa -o rust_pe.sam -s 12 -v 0.08 -I 4 -D C-CGG -p 8` |
| C++ 参数 | 同等 `-s 12 -v 0.08 -I 4 -D C-CGG -p 8` |

实际状态：

- 未产生有效的当前 P13 Rust mm10 10K benchmark。
- 容器内 `/workspace/03_project/01_bsmap-rs` 是旧 Rust 架构，和当前本地 P13 的 `snp_align_segment`、`KmerIndex`、`KmerLoc` 等接口不一致。
- MCP 当前没有正式上传/同步文件能力，不能直接把当前 P13 worktree 或 release binary 放进容器。
- 曾评估通过 `ssh_exec + docker exec + base64 tar` 分块传源码；源码包约 121 KB，但该方案需要在对话中传递大量 base64 内容，不适合作为可靠、可复用的验收流程。已清理 Docker 内半成品 `/tmp/p13_codex_current/src.tgz.b64`。
- 容器内 `/workspace/03_project/01_bsmap-rs` 不是 git 仓库，无法用 `git checkout`、`git apply` 或 fresh branch 方式恢复到本地 `981a283` 基线。
- 曾尝试在 Docker 内临时适配旧源码，`cargo check` 失败；未覆盖 `/workspace/03_project/01_bsmap-rs/target/release/bsmap`，因此没有生成新的旧架构 binary。该旧源码目录可能仍包含临时改动，后续不应把它视为干净源码基线。
- 由于这一步无法证明运行的是当前 P13 代码，所有旧容器 Rust 结果都不计入本报告的 P13 验收。

## 验收状态

| 验收项 | 状态 | 说明 |
| --- | --- | --- |
| `cargo check` | 通过 | warning 可接受 |
| `cargo test` | 通过 | 全部现有测试通过 |
| `cargo build --release -p bsmap` | 通过 | release binary 已生成 |
| example1 WGBS 不回归 | 通过 | Rust/C++ mapped 均为 66,120，QNAME 完全重合 |
| example2 WGBS 对照 | 部分通过 | Rust 正常；C++ BSMAP 本身 buffer overflow |
| mm10 RRBS 10K SE | 未完成 | 当前缺少把 P13 代码同步进 Docker 的安全路径 |
| mm10 RRBS 10K PE | 未完成 | 同上 |
| RRBS 不回退到 chr1 99% 偏斜 | 未验证 | 需当前 P13 binary 在 Docker 内运行后判断 |
| Rust RRBS SE 不低于 P12 2,124 | 未验证 | 需当前 P13 binary 在 Docker 内运行后判断 |

## 下一步

下一步不建议继续手工改容器内旧 Rust 源码。正确路径是先解决“当前 P13 代码如何进入 Docker”：

1. 通过 MCP 增加文件上传能力，或在 Docker 内 fresh clone/checkout 到与本地 P13 相同提交。
2. 在容器内用当前 P13 源码重新执行 `cargo build --release -p bsmap`。
3. 强制删除旧 RRBS `.bsi`，用 v3 RRBS 编码重建 mm10 index。
4. 再跑 Rust/C++ SE 与 PE，并记录速度、峰值内存、CPU、SAM 详情和染色体分布。

在这一步完成前，P13 只能标记为“本地编译与 WGBS 回归通过，服务器 RRBS 主验收未完成”。
