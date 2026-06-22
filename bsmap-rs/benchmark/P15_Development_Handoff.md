# P15 开发交接记录

## 0. 2026-06-22 退出账号检查点（最新状态，优先于下文旧记录）

### Git 与保存状态

- worktree：`C:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p15-performance`
- 分支：`codex/p15-performance`
- 已提交并推送的最后检查点：`696c9efc25d3172af7acdddbd427976ce9e7a4d6`
- 当前工作树包含未提交的 Phase 0 工具、Phase 2A 和 RRBS v9 原型。退出账号不会删除这些文件，但它们尚未进入 Git commit。
- Windows Git 当前读取 loose object `71d6ecb15ed461915f25833e77f6bc3830053d34` 时返回 `Permission denied`；linked worktree 的 `.git/worktrees/.../index.lock` 也曾因 ACL 无法创建。因此不得声称当前改动已提交或推送。
- 恢复后应先保全当前脏工作树，不要执行 `reset --hard`、`checkout --`、`clean` 或从远端重建 worktree。远端只有 `696c9ef`，会丢失本检查点之后的实现。

当前修改范围：

```text
M  .gitignore
M  AGENTS.md
M  bsmap-rs/benchmark/P15_Development_Handoff.md
M  bsmap-rs/bsmap/src/align/engine.rs
M  bsmap-rs/bsmap/src/align/extend.rs
M  bsmap-rs/bsmap/src/align/seed.rs
M  bsmap-rs/bsmap/src/reference/binseq.rs
M  bsmap-rs/bsmap/src/reference/index_io.rs
?? bsmap-rs/benchmark/p15/
```

### 已完成并验证

1. Phase 0 benchmark 工具：`benchmark/p15/` 已包含 GNU time 解析、v8 section 检查、常数内存 FASTQ repeat producer、限速 SAM sink、FIFO scale runner、汇总脚本和 11 个工具单测。
2. 修复 PE FIFO 死锁：R1/R2 使用独立 producer；工具测试覆盖 FIFO 顺序消费。该陷阱已写入 `AGENTS.md`。
3. Phase 2A seed scratch：`SeedSegment` 改为内联数组，Rayon worker 复用 seed chain/segment scratch；完整 `cargo check/test/release` 已通过。
4. Phase 2A release binary SHA256：`0afc9a2eebea43ef609501146b78cfba9213f7024d47980b25e7c16f07926417`。
5. Phase 2A 结果保持输出完全一致：
   - example1：132,240 条（两次 repeat），SHA256 `7a0203...`，三轮 wall 中位 1.41 秒，RSS 23,028 KiB。
   - example2：66,958 条，SHA256 `e73edc...`，三轮 wall 中位 1.02 秒，RSS 31,336 KiB。
   - mm10 RRBS SE：2,423 条，SHA256 `420e34a3fa39086effbff8341cde5bacf90fde9bf57a32b39e0cb48eeedd9ad0`，三轮 wall 中位 12.59 秒，最坏 RSS 1,309,732 KiB。
   - mm10 RRBS PE：4,884 条，SHA256 `7b33a9d894f670e1ec2424430d614d06c2d4d2d48a06fc4880c0568949f39ac6`，三轮 wall 中位 14.70 秒，最坏 RSS 1,401,944 KiB。
6. 两个不达标实验已完整回退：identity hasher（收益不足 5%）和预计算 N/复用 selection set（example1 回退约 12%）。回退后 binary SHA256 恢复为 Phase 2A 的 `0afc...`。

### 当前 RRBS v9 原型

目标是移除 RRBS 索引中重复保存的约 683 MB reverse reference，改为 single-chain reference + reverse window on demand。

- `reference/binseq.rs`：新增 `fill_reverse_window()`，单测逐 code 对比 materialized `crefcat`，覆盖 padding、正文和染色体边界。
- `align/extend.rs`：RRBS reverse hit 在 `crefcat` 为空时，用固定栈缓冲生成所需 reverse window；旧索引和 WGBS 路径不变。
- `reference/index_io.rs`：新增 RRBS v9；RRBS v9 不写 `crefcat` section，旧 RRBS v8 强制重建；WGBS 继续使用 v8。
- 当前原型已通过：201 个 lib tests、3 个 bin tests、doc tests、`cargo build --release -p bsmap`。
- 尚未完成：`benchmark/p15/index_sections.py` 仍只接受 v8；v9 小 fixture、mm10 v9 索引构建、RRBS SE/PE SAM parity、性能/RSS/major faults、WGBS 回归均未执行。因此 v9 只能称为“编译与单测通过的原型”，不能称为保留优化或交付结果。

### 恢复后的精确执行顺序

1. 先读取本节并检查当前文件仍在；不要先 fetch/reset。
2. 修复或绕开 Windows Git object/index ACL，创建本地检查点提交；在 commit 成功前不做破坏性 Git 操作。
3. 更新 `benchmark/p15/index_sections.py` 及测试，使其接受 RRBS v9、拒绝 WGBS v9，并校验 v9 `crefcat_words == 0`。
4. 运行 v9 小型 RRBS fixture，确认真正走空 `crefcat` 的 align 路径。
5. 在新的结果目录构建 mm10 v9 索引；不得覆盖 `D:/BSMAP/benchmark-data/mm10/mm10.fa.bsi`。standalone index 单独计时。
6. 运行 mm10 RRBS SE/PE 各三轮，分别与上面的完整 SHA256 golden 比较；同时记录 index size、wall/user/sys、CPU、RSS、major faults。
7. 重跑 WGBS example1/example2，确认完整 SAM 不回归；example1 还必须与 C++ 的 RNAME、POS、FLAG、NM、ZP、ZL 和完整记录 100% 一致。
8. 若 v9 输出不一致或关键性能无合理收益，则精确回退 v9 文件，不回退 Phase 0/2A。若保留，再创建正式报告并继续后续 Phase 5-8。

结果目录位于 `D:/BSMAP/benchmark-results/p15/`。90G WGBS / 10G RRBS 正式长测、Phase 5-8、最终报告、合并 `main` 和推送远端 `main` 均未完成。

## 1. 恢复入口

- 日期：2026-06-22
- 分支：`codex/p15-performance`
- worktree：`C:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p15-performance`
- 基线：`main@9ed7ebca704035d43c2de853883d65c53e9f86b0`
- 当前代码检查点：以 `codex/p15-performance` 的 HEAD 为准；最近远端检查点为 `696c9ef`
- P15 仍在进行中，不得合并到 `main`，不得标记完成。

已提交：

1. `c8387dc docs: add P15 performance optimization plan`
2. `dc4e2ea feat: add v8 succinct WGBS index`
3. `3cec7fc fix: preserve legacy WGBS index serialization`

## 2. 已完成

### Phase 0：可复现基准与规模化 runner

- 新增 `benchmark/p15/`：GNU time 解析、v8 section 检查、常数内存 FASTQ 重复 producer、可限速 SAM sink、FIFO scale runner 和 11 个工具单测。
- runner 强制复用现有 `.bsi`，metadata 明确记录 `standalone_index_included=false`；binary/reference/index/reads 均记录 SHA256。
- WGBS example2 PE、mm10 RRBS SE/PE 的 FIFO 输出 SHA256 与 Phase 1 golden 完全相同。
- 限速 5 MiB/s 的 WGBS SE smoke 正常完成，66,120 条 SAM、wall 2.89 秒、RSS 23,024 KiB，验证输出背压不会扩张进程内存。
- v8 section inspector 已验证 WGBS 13,691,272 bytes 和 RRBS 1,773,733,496 bytes 索引边界；RRBS 中两个 reference chain section 各占 682,725,664 bytes，是 Phase 3/4 的主要内存目标。
- 90G WGBS / 10G RRBS 正式长跑尚未执行；当前仅证明 runner 路径和小型 backpressure smoke，不能作为大规模验收结论。

### Phase 1：v8 succinct WGBS index

- 新增 occupancy bitvector、word-level rank、8-byte compact bucket descriptor 和稀疏 overflow count table。
- WGBS 只为非空 hash 保存 descriptor，删除运行时 dense `index2 + start_offsets`。
- 保持 C++ raw CountSeeds、overrepresented bucket、forward/reverse positions 和 circular candidate 顺序。
- v8 使用 `RAWSECT2` marker；WGBS 与 RRBS 在同一 section directory 中使用 mode-aware layout。
- v7 仍可显式读取，但 cache compatibility 要求 v8 并强制旧索引重建。
- legacy `save_index()` 会把 compact index 安全展开为旧 dense layout，语义 round-trip 已测试。
- RRBS v8 继续使用 P14 flat layout，本阶段未声称 RRBS 性能收益。

## 3. 编译与测试状态

在 WSL2 Ubuntu 登录 shell 中执行：

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p15-performance/bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
```

结果：

- `cargo check -p bsmap`：通过。
- `cargo test -p bsmap`：199 个 lib tests、3 个 bin tests和 doc tests 通过。
- `cargo build --release -p bsmap`：通过。
- Windows `git diff --check`：通过。
- 既有 warning 未清理，不属于本轮失败。
- WSL 命令末尾直接运行 `git diff --check` 会因 Windows linked-worktree gitdir 无法解析而失败；Git 命令必须使用 Windows Git。

注意：最后一次 release binary 构建早于 legacy serializer 的测试增强；该增强不影响正常 v8 align，但恢复后仍应先重新执行 release build。

## 4. Phase 1 benchmark

### WGBS example1 SE

结果目录：`D:/BSMAP/benchmark-results/p15/phase1-wgbs-v8`

| 指标 | P14 | P15 Phase 1 |
|---|---:|---:|
| index size | 519,037,888 bytes | 13,691,272 bytes |
| Rust p8 wall | 2.00 s | 0.92 s |
| Rust p8 CPU | 76% | 123% |
| Rust p8 max RSS | 509,540 KiB | 23,368 KiB |
| major faults | 17,447 | 597 |
| SAM records | 66,120 | 66,120 |

与 C++ golden：

- 完整记录：66,120/66,120 一致。
- `RNAME/POS/FLAG/NM/ZP/ZL` 差异全部为 0。
- comparison：`phase1-wgbs-v8/comparison.json`。

### WGBS example2 PE

| 模式 | wall | CPU | max RSS |
|---|---:|---:|---:|
| Rust p1 | 2.26 s | 62% | 31,564 KiB |
| Rust p8 | 1.34 s | 113% | 31,968 KiB |

- p1/p8 SAM SHA256 均为 `e73edc7e7327524028c61bb4a1eed14b8428eedfe3dc1e8be5a22cc334f313ca`。
- C++ PE 既有 signal 6 限制不变。

### mm10 RRBS 10K

结果目录：`D:/BSMAP/benchmark-results/p15/phase1-mm10-v8`

- v8 index：1,773,733,496 bytes，与 P14 RRBS layout 相同。
- SE：2,423 条，与 C++ 完整记录 100% 一致。
- PE：4,884 条，与 P14 PE SAM 字节一致。
- 本轮 SE 17.98 s、PE 19.68 s，major faults 分别为 335,251 和 401,519。
- 该 wall 比 P14 最终轮次慢，属于尚未处理的 DrvFS/mmap 冷缺页问题；Phase 1 不保留或宣称 RRBS 性能收益。
- standalone index 本轮 72.86 s，受输入/落盘状态波动影响；Phase 8 尚未实施。

## 5. 未完成项

以下均未完成，不得在新会话中误判为已交付：

1. 90G WGBS / 10G RRBS 的正式流式吞吐和常数内存长测。
2. Phase 2：`SeedScratch`、低分配 segment、整数去重和模式专用 kernel。
3. Phase 3：RRBS logical rank/select、section 级 mmap advice、缺页和 sys time 优化。
4. Phase 4：single-chain reference / reverse-on-demand 实验。
5. Phase 5：输入编码输出 bounded pipeline、直接 SAM/BAM writer。
6. Phase 6：PE pairing 去复制和去 HashMap。
7. Phase 7：POPCNT/SIMD/runtime dispatch、release profile。
8. Phase 8：standalone index 构建峰值内存和落盘优化。
9. P15 最终报告、完整验收、合并 main 和远端 main 推送。

## 6. 恢复后立即执行

```powershell
git fetch origin codex/p15-performance
git worktree list
git -C .claude/worktrees/p15-performance status --short --branch
git -C .claude/worktrees/p15-performance log --oneline -5
```

然后：

```bash
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p15-performance/bsmap-rs
cargo check -p bsmap
cargo test -p bsmap
cargo build --release -p bsmap
```

若本地 worktree 不存在，应从远端分支新建，不要在脏 `main` 工作区直接开发。

## 7. 推荐下一步

1. 进入 Phase 2，优先消除每 read seed/segment Vec 与 SipHash；每个提交运行 example1、example2、mm10。
2. 随后进入 RRBS Phase 3；目标是同时降低 major faults、sys time 和 RSS，不再全局使用 `MADV_RANDOM`。
3. 使用现有 FIFO runner 做代表性长跑；最终报告必须区分实际流式字节、源文件字节等价量和外推规模。

## 8. 继续开发提示词

```text
继续完成 BSMAP Rust 重构项目的 P15 优化。

仓库：
C:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP

只在以下 worktree/分支开发：
.claude/worktrees/p15-performance
codex/p15-performance

先完整阅读：
1. 根目录 AGENTS.md
2. bsmap-rs/benchmark/P15_Performance_Optimization_Plan.md
3. bsmap-rs/benchmark/P15_Development_Handoff.md
4. bsmap-rs/benchmark/P14_Performance_Optimization_Report.md

本地分支 HEAD/远端最后已提交检查点是 696c9efc25d3172af7acdddbd427976ce9e7a4d6，但 worktree 中还有未提交的重要改动。绝对不要先 reset、clean、checkout 或重建 worktree。先阅读 handoff 第 0 节，核对脏工作树文件，并优先解决 Git loose object/index ACL 后创建检查点提交。

当前真实进度：
- Phase 0 benchmark/p15 工具已完成并通过 11 个工具单测。
- Phase 1 v8 succinct WGBS index 已完成。
- Phase 2A seed scratch 已完成完整 cargo 验证和 example1/example2/mm10 SE/PE 基准，输出 SHA 保持一致；identity hasher 与 selection-set 实验因收益不足或回退已撤销。
- RRBS v9 single-chain reference + reverse-on-demand 目前只完成代码、201+3 单测和 release build；尚未跑小 fixture、mm10 v9 index/SE/PE 和 WGBS 回归，不能宣称已保留。

不要重做 Phase 0/1/2A，不要修改 main、P12、P13、P14 worktree。恢复后的第一项代码任务是更新 benchmark/p15/index_sections.py 支持 RRBS v9，然后严格按 handoff 第 0 节的执行顺序验证 v9。

每个独立优化后必须：
- WSL2 cargo check/test/release build
- WGBS example1 与 C++ 完整 SAM 100% 一致
- WGBS example2 Rust p1/p8 字节一致
- mm10 RRBS 10K SE 与 C++ 完整 SAM 100% 一致
- mm10 RRBS PE 与 P14 基线一致
- 记录 wall/user/sys、CPU、RSS、major faults、SAM 详情和 SHA256

Rust standalone index 必须单独计时，不得计入 Rust/C++ 单样本比对时间。
90G/10G 场景必须验证常数内存、输入解压、输出背压和长时间吞吐。
如实记录所有失败、回退和未完成项。完成全部 P15 验收后再合并 main 并推送。
```
