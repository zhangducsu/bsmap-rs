# P14 C++ 等价与全路径性能优化合并计划

## 文档定位

本文是前序两份 P14 计划的唯一合并版本，统一以下两条工作主线：

1. **语义与结果等价**：以 C++ BSMAP 2.90 为语义基准，覆盖 WGBS/RRBS、SE/PE，以及 `RNAME`、`POS`、`FLAG`、`NM`、`ZP`、`ZL` 和完整 SAM 记录。
2. **速度与内存优化**：在结果等价的前提下，优化 standalone index、warm-index 加载、read 编码、索引存储和并行比对路径。

执行优先级固定为“正确性等价 -> 可复现基准 -> 性能优化”。性能报告和原始 benchmark 结果不并入本文，继续由 `P14_Performance_Optimization_Report.md` 与 `benchmark/p14/` 保存，避免计划、实现状态和实测证据相互混淆。

## 目标

P14 以 C++ BSMAP 2.90 为语义基准，先实现 WGBS/RRBS 比对结果等价，再优化 RRBS/WGBS、SE/PE 共用的索引加载、索引构建、read 编码和并行路径。正确性优先于性能，任何性能优化都不得改变已经对齐的 SAM 输出。

Rust 索引构建是独立步骤，不计入与 C++ 的单样本比对耗时比较。正式比较只使用已有 `.bsi` 的 Rust warm process 与 C++ 正常 invocation；索引构建时间、CPU、峰值内存和索引大小单独报告。

## 正确性门槛

### WGBS

- example1 SE 使用相同参数和固定随机种子。
- 排除 SAM header 后，Rust 与 C++ 的记录数、记录顺序和每条记录必须 100% 一致。
- 完整记录覆盖 11 个固定字段及全部 optional tags，重点单列 `RNAME`、`POS`、`FLAG`、`NM`、`ZP`、`ZL`。
- 逐行字节比较只允许规范化 CRLF/LF，不允许忽略字段、重排 tags 或排序 SAM。
- WGBS 中 C++ 不输出 `ZP/ZL`，Rust 也必须不输出，统计工具显式验证两者缺失。
- 建立最小确定性 WGBS PE fixture；若原版 C++ 可运行，要求完整记录 100% 一致。若仍退出 134，保存命令、退出码和 stderr，Rust PE 以 `-p 1/-p 8` 字节一致作为可执行门槛。

### RRBS

- mm10 10K SE 保持与 C++ 相同的 2,423 条 mapped QNAME。
- 确定性记录要求完整 SAM 记录 100% 一致，染色体分布、FLAG、`NM/ZP/ZL` 不得回归。
- Rust PE 保持 4,884 条结果及线程间确定性；C++ PE 退出 134 如实记录。

## 实施阶段

### 1. WGBS 语义对齐

逐项对齐 C++ 的 seed 起点调整、`CountSeeds` 排序、hit bucket 起点和环形遍历、`myrand`、`AddHit` 去重、阈值更新、max-hit 早停、primary/secondary FLAG、MAPQ、CIGAR、mate 字段、SEQ/QUAL 方向及 optional tag 输出顺序。example1 达到完整记录 100% 一致后才进入性能结构重构。

### 2. Warm-index 快速加载

- align 启动时先读取 `.bsi` metadata，不再先扫描和编码完整 FASTA。
- 使用文件大小、mtime、reference 名称/长度和索引参数进行快速缓存验证。
- 从索引恢复 `chr_lengths` 和 `ref_anchor`。
- 索引升级到 v7，使用对齐 section、offset table 和明确格式标记。
- 以 owned/mmap 统一只读存储表示 reference chains、hash index 和 positions，避免大数组 bincode 反序列化。
- v6 及更旧格式明确拒绝并提示重建。

### 3. Standalone 索引构建

- 合并 FASTA 读取、reference 编码和 CCGG digestion，避免 RRBS 两次读取 FASTA。
- 使用紧凑 seed-origin 表替代嵌套 `ccgg_index`。
- 使用计数、前缀和、一次填充的两遍构建，并保持 C++ 的 `mode -> block -> origin` hit 顺序。
- 修正 `bsmap index` 的 RRBS 路径，使其与 `align -D` 自动构建语义一致。

### 4. Read 与并行路径

- `EncodedRead` 使用固定 `[u64; FIXELEMENT]` 数组和最小元数据，不再克隆完整 `ReadInf`。
- reverse words/mask 直接写入固定数组，删除逐 read 临时分配。
- SE 使用 indexed Rayon 和 worker-local scratch buffer，输出顺序保持输入顺序。
- PE 在同一并行任务内完成双端编码、比对和 pairing，避免两份完整中间命中集合。
- 暂不引入 allocator 替换、复杂 SIMD、unsafe bounds removal 或低收益 CCGG tuple 压缩。

## Benchmark 口径

在 `benchmark/p14/` 提供统一 runner、SAM 比较器和 metadata 工具。Rust 测量拆分为：

1. standalone index build：wall/user/sys、CPU、RSS、索引大小；
2. warm process：加载已有索引、读取 reads、比对和写 SAM；
3. alignment core：排除索引加载后的纯比对阶段。

每项记录 commit、binary/input SHA256、完整命令、参数、线程数、随机种子、reference/reads/index 路径、退出码、wall/user/sys、CPU%、max RSS、major faults、SAM 统计和原始结果路径。性能测试至少运行三次并使用中位数，首次 OS page-cache 冷启动单独报告。

## 验证循环

每个独立优化提交执行：

```bash
cargo check
cargo test
cargo build --release -p bsmap
git diff --check
```

随后执行 WGBS example1 Rust/C++ 完整记录比较、WGBS example2 Rust `-p 1/-p 8` 回归、最小 WGBS PE fixture、mm10 RRBS 10K SE/PE，以及 standalone/warm-index 性能测试。语义回归、输出不确定或性能无实际收益的步骤不保留。

## 验收标准

- WGBS example1 非 header SAM 与 C++ 逐行 100% 一致。
- WGBS PE fixture 在 C++ 可运行时逐行 100% 一致。
- RRBS mm10 SE 保持 2,423 条并达到确定性记录完全一致。
- warm mm10 SE 中位 wall time不超过 25 秒，峰值 RSS 不超过 1.3 GB。
- standalone RRBS index 不超过 75 秒、RSS 不超过 2.0 GB、v7 索引不超过 1.78 GB。
- alignment core 的 `-p 8` 相对 `-p 1`：SE 至少 2 倍，PE 至少 1.5 倍。
- 单项优化必须在结果完全等价前提下，使速度或峰值内存至少改善 5%。

## 交付物

- 本计划 `P14_Performance_Optimization_Plan.md`。
- 最终报告 `P14_Performance_Optimization_Report.md`。
- `benchmark/p14/` 可复现 runner、统计工具和小型 fixture。
- 每阶段 metadata、summary 和 SAM 比较结果。
- `AGENTS.md` 中新增本轮确认的索引、benchmark 和 C++ PE 陷阱。

## 默认约束

- 只在 `codex/p14-performance` 开发，不修改 P12/P13 worktree。
- 本地测试优先使用 WSL2 和 `D:\BSMAP\benchmark-data\mm10`，默认不再使用服务器。
- C++ BSMAP 2.90 不修改，保持原始语义基准。
- P13 输出不是 WGBS golden；只有达到 C++ 100% 等价后的输出才能成为回归基线。
