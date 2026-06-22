# P15 开发交接记录

## 1. 恢复入口

- 日期：2026-06-22
- 分支：`codex/p15-performance`
- worktree：`C:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/.claude/worktrees/p15-performance`
- 基线：`main@9ed7ebca704035d43c2de853883d65c53e9f86b0`
- 当前代码检查点：`3cec7fc`
- P15 仍在进行中，不得合并到 `main`，不得标记完成。

已提交：

1. `c8387dc docs: add P15 performance optimization plan`
2. `dc4e2ea feat: add v8 succinct WGBS index`
3. `3cec7fc fix: preserve legacy WGBS index serialization`

## 2. 已完成

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

1. `benchmark/p15/` profiling、time 汇总和规模化 runner。
2. 90G WGBS / 10G RRBS 的流式倍增吞吐和常数内存验证。
3. Phase 2：`SeedScratch`、低分配 segment、整数去重和模式专用 kernel。
4. Phase 3：RRBS logical rank/select、section 级 mmap advice、缺页和 sys time 优化。
5. Phase 4：single-chain reference / reverse-on-demand 实验。
6. Phase 5：输入编码输出 bounded pipeline、直接 SAM/BAM writer。
7. Phase 6：PE pairing 去复制和去 HashMap。
8. Phase 7：POPCNT/SIMD/runtime dispatch、release profile。
9. Phase 8：standalone index 构建峰值内存和落盘优化。
10. P15 最终报告、完整验收、合并 main 和远端 main 推送。

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

1. 先实现 `benchmark/p15/` 和 FIFO/repeated FASTQ 规模 runner，明确 index-warm、page-cache-cold、page-cache-warm。
2. 给 v8 compact bucket 增加显式 overflow fixture 和 section size 工具。
3. 进入 Phase 2，优先消除每 read seed/segment Vec 与 SipHash；每个提交运行 example1、example2、mm10。
4. 随后进入 RRBS Phase 3；目标是同时降低 major faults、sys time 和 RSS，不再全局使用 `MADV_RANDOM`。
5. 90G/10G 不要求每个小步都生成完整物理文件；使用 FIFO 流式倍增验证常数内存，最终再做代表性长跑和吞吐外推，报告必须标明实测规模与外推规模。

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

先核对远端和本地 HEAD，当前应至少包含：
c8387dc
dc4e2ea
3cec7fc

当前已完成 v8 succinct WGBS index。不要重做 Phase 1，不要修改 main、P12、P13、P14 worktree。

从 handoff 的“未完成项”和“推荐下一步”继续：
先完成 benchmark/p15 的 profiling 与 90G WGBS/10G RRBS 流式规模验证工具，然后实施 Phase 2。每个独立优化后必须：
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

