# SSH2 RRBS 生产级速度与内存优化计划

## 目标

SSH2 的目标不是继续做小幅微调，而是在 SAM 对齐前提下，用工程优化让 Rust RRBS 比对达到生产可用速度：

- 参数完全相同：`-s 12 -v 0.08 -I 4 -D C-CGG -p 8 -S 1`，按需增加相同的 `-B/-E` 读段范围。
- Rust standalone index 单独计时，不纳入与 C++ 单样本 align 时间比较。
- Rust 与 C++ 的 SAM 至少在 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` 上完全一致；10K 和中等抽样必须完整字段 diff 为 0。
- 内存使用低于或相当于 C++；RSS 不允许为了提速超过 C++，除非报告中明确给出可配置开关且默认关闭。
- 速度目标：同一数据、同一参数、同一服务器环境下，Rust warm align wall time 必须小于等于 C++ normal invocation 的 50%。full SE 目标以 C++ `536.04s` 旧基线为起点，阶段目标为 Rust full SE `<=268s`，最终以 SSH2 重新测得的 C++ full SE 为准。

## 当前证据

- SSH1 基线：Rust 10K SE 与 C++ 2,423 条完全一致，字段 diff 为 0。
- SSH1 full SE：Rust warm align `3,778.00s`，RSS `913,116 KiB`，CPU 约 `753%`；C++ 旧 full SE `536.04s`，RSS 约 `2.87 GiB`。Rust 内存明显更低，但速度慢约 7 倍。
- 10K stage profile：Rust `read_seconds=0.011400`、`prepare_seconds=0.013688`、`align_seconds=1.300677`、`write_seconds=0.023240`，瓶颈集中在 align core。
- 10K count-heavy profile：`raw_bucket_candidates=103,224,911`、`logical_bucket_candidates=59,676,398`、`mode_matched_candidates=20,038,380`、`mismatch_calls=20,038,380`。优化重点应是候选数量、mismatch 热路径、早停和输出后处理，而不是索引重建。
- `390c0c2` 的 iterator 手写循环尝试已撤回：10K 五轮 Rust-only A/B 只有约 `0.7%` 收益，不足以保留。

## 基准策略

SSH2 使用三层验证，避免 10K 噪声和 full 长任务互相误导：

1. **10K correctness gate**
   - 使用 SSH1 runner 或 SSH2 subset runner。
   - Rust/C++ mapped、SAM 字段 diff 必须为 0。
   - 只用于确认语义，没有足够统计力判定小优化收益。

2. **100K/1M screening gate**
   - 使用 `benchmark/ssh2/run_server_rrbs_subset.sh`，对 full R1 执行 `-E 100000`、`-E 1000000`。
   - 每个候选优化至少比较 Rust A/B 三轮；保留门槛为目标 workload wall time 提升 `>=10%`，或为 full 速度目标提供明确候选数量/mismatch 调用下降证据。
   - correctness 使用排序后的 `QNAME/RNAME/POS/FLAG/NM/ZP/ZL` multiset 比较；streaming compare 只用于诊断输出顺序是否与 C++ 一致。
   - sorted multiset diff 必须为 0。若 streaming diff 非 0 但 sorted multiset 为 0，记录为输出顺序差异，不阻塞性能筛选。

3. **full SE acceptance gate**
   - Rust warm index align 与 C++ normal invocation 同机同参数比较。
   - Rust full SE 必须 `wall <= C++ wall / 2`，RSS `<= C++ RSS` 或至少不高于 C++ 的同量级上限。
   - full SAM 采用 streaming compare，不把 800 万级 records 一次性装入内存。

## 优化方向

### Phase 1：可比较 benchmark 与中等抽样

- 新增 SSH2 subset runner，固定 full R1、`-E N`、二进制 SHA、输入 SHA、退出码、time/RSS/CPU、SAM stats 和字段 diff。
- 先跑 `10K/100K`，确认 C++ 与 Rust 都支持同一 read range。
- 若 100K 字段已对齐，扩展到 `1M`，作为后续优化筛选主基准。

### Phase 2：RRBS mode bucket 直接索引

当前 `.bsi` 的 RRBS bucket 仍需要在 runtime 对同一 k-mer 的候选做 mode/read-chain 过滤。SSH2 允许 bump index 版本，重建一次索引，换取 align 热路径大幅减少：

- 将 RRBS hit bucket 从 `kmer -> mixed hits` 改为 `kmer -> mode/read-chain/ref-chain filtered spans` 或等价的 offset/count 表。
- SE 默认只暴露 C++ 等价 normal bucket；PE 或 `-n 1` 再启用 cross-chain bucket。
- 保持 C++ 随机选择语义：随机 modulus 必须作用在 C++ 等价 logical bucket 上，不能先取模再过滤。
- 预估收益：若 10K 的 `mismatch_calls` 从 `20M` 下降到接近 C++ 真实候选数量，full SE 有机会获得 `2x-5x` wall time 改善；RSS 可能增加 offset 表，但应低于 C++ RSS。

### Phase 3：mismatch 热路径重写

- 针对 RRBS read length 和 `seed_size=12` 写专用 mismatch kernel，减少边界检查、slice 分支和函数调用。
- 用 C++ `XM64/XT64/XC64` 语义建立固定向量测试，保证 `NM` 不回归。
- 仅在 100K/1M 端到端收益明确时接入 SIMD；避免只凭 microbench 保留。
- 预估收益：`1.2x-2x`，取决于候选数量是否先被 Phase 2 降下来。

### Phase 4：早停与候选统计等价审计

- 对比 C++ `AddHit()` 中 `snp_thres` 更新、`max_num_hits`、gap 尝试顺序和 RunAlign 的 segment-level 早停。
- 如果 Rust 仍比 C++ 多做候选，优先修等价逻辑，而不是仅靠更快的 mismatch。
- 预估收益：若 full 多候选来自早停差异，可能 `1.5x-3x`；若已等价，则收益有限。

### Phase 5：输出与 I/O

- 只有 stage profile 显示 write/read 占比显著时才优化。
- 默认不启用 RRBS pipeline，除非 full 或 1M 证明 gzip/read 或 SAM write 阻塞。
- 预估收益：当前 10K write/read 占比很低，优先级低；full SAM 3.5G 下可能有 `5%-15%`。

## 保留与撤回规则

- 任何性能优化必须保持 SAM 字段 diff 为 0。
- 10K 单次 wall 不作为保留依据；中等抽样 A/B 才能判断收益。
- 提升低于 `3%` 且增加代码复杂度的改动必须撤回。
- 提升 `3%-10%` 的改动只在不增加复杂度或为后续大优化铺路时保留。
- 提升 `>=10%` 且无语义回归的改动可进入 full gate。

## 当前下一步

1. 提交 SSH2 runner 和计划文档。
2. 同步服务器 Docker 到 SSH2 分支。
3. 跑 `SSH2_LIMITS="10000 100000"`，确认 runner、C++ `-E`、Rust `.bsi` warm 口径和 SAM diff。
4. 若 100K 可用，跑 `SSH2_LIMITS="1000000"`，作为 Phase 2/3 优化筛选基线。
