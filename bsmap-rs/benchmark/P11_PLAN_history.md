# P11 性能优化计划

**日期:** 2026-05-22  
**基线:** P10 (889 MB RSS, SE 1.43s p=4, 与 C++ BSMAP 2.90 比对结果一致)  
**目标:** 在保持比对逻辑不变的前提下，进一步降低内存、提升速度、提高 CPU 利用率

---

## 总览

| 优先级 | 编号 | 类别 | 预估影响 |
|--------|------|------|----------|
| 严重 | P11-1 ~ P11-3 | 内存 | 释放 ~7.5GB 死数据，预计峰值内存从 889MB → ~500MB |
| 严重 | P11-4 ~ P11-6 | 速度 | 消除热路径冗余分配，预计速度 +15~25% |
| 重要 | P11-7 ~ P11-14 | 速度/内存 | 边际增益累积，预计速度 +10~15% |
| 建议 | P11-15 ~ P11-20 | 架构/代码质量 | 长期收益，非阻塞 |

---

## 严重优先级 — 内存（释放死数据）

### P11-1: 释放 BinSeqCollection.seqs 死数据 (~3 GB)

**文件:** `bsmap/src/reference/binseq.rs:60, 68-147`

**问题:**
- `BinSeqCollection.seqs: Vec<BinarySeq>` 仅在 `from_references()` 构造期间使用：
  1. 编码 refcat/crefcat（从 seqs 逐条 copy words 到拼接数组）
  2. 查找 unmasked regions（`find_blocks` 用原始序列，不用 BinarySeq）
- 构造完成后，所有比对操作仅通过 `refcat` / `crefcat` 的 `as_slice()` 访问参考序列
- 人类基因组 25 条染色体 × 2 链，每个 `BinarySeq` 含 `words: Vec<u64>` + `mask: Vec<u64>`
- 估算：~3 GB（含 words 和 mask）完全不被使用

**修复:**
```rust
// binseq.rs:138 — 在 Self { ... } 返回前
// seqs 仅在构建期有用，完成后释放以节省内存
// 如果后续需要（如 get_chromosome_length），可从 ref_anchor 推算
Self {
    total_num,
    sum_length,
    refcat: Box::new(VecStorage::new(refcat)),
    crefcat: Box::new(VecStorage::new(crefcat)),
    ref_anchor,
    blocks,
    seqs: Vec::new(),  // ← 释放死数据，不保留 seqs
    chr_names,
}
```

**风险:** 需确认 `seqs` 在构造后无任何读写。通过 grep 验证：`seqs` 仅由 `from_references()` 写入，无外部读取路径。`get_chromosome_length()` 使用 `ref_anchor` 而非 `seqs`。

**预估收益:** -3 GB 常驻内存。

---

### P11-2: 消除 BinarySeq.mask 构建与存储 (~1.5 GB)

**文件:** `bsmap/src/reference/binseq.rs:20-27, 77-97`

**问题:**
- `BinarySeq.mask` 是 REG_ALPHABET 掩码（标记 N/有效碱基），在编码阶段构建
- 拼接 refcat/crefcat 时**只复制 `words`**，不复制 `mask`（见 `from_references` 第 124-129 行）
- `mask` 的唯一消费者是 `find_blocks()`，而 `find_blocks()` 使用原始序列（`&r.seq`），不读 BinarySeq.mask
- 因此 mask 在整个生命周期中**从未被读取**

**修复:**
```rust
// 方案 A: 从 BinarySeq 结构体中移除 mask 字段
#[derive(Debug, Clone)]
pub struct BinarySeq {
    pub n: u32,
    pub words: Vec<u64>,
    // pub mask: Vec<u64>,  ← 删除
}

// 方案 B（更保守）: 在 from_references 中构建后立即丢弃
// encode_forward / encode_revcomp 不再构建 mask
```

**注意:** `encode_forward` 和 `encode_revcomp` 仍被单元测试引用 mask 字段。需更新测试或保留仅测试用的构造路径。

**预估收益:** -1.5 GB 常驻内存。

---

### P11-3: 索引构建后立即 drop FASTA refs (~3 GB)

**文件:** `bsmap/src/main.rs` (run_single_align / run_paired_align 入口)

**问题:**
- `main.rs` 的比对流程：
  1. 加载 FASTA → `refs: Vec<Reference>` (~3 GB 原始序列文本)
  2. 构建 `BinSeqCollection::from_references(&refs)` — 将序列编码为 2-bit
  3. 构建 `KmerIndex::build_wgbs(&coll, ...)` — 完全依赖 coll，不依赖 refs
  4. 进入比对循环 — refs 仍然存活
- `Reference.seq: Vec<u8>` 保存原始序列文本（每碱基 1 字节），人类基因组 ~3 GB
- 比对全程仅使用 refcat/crefcat（2-bit 编码，内存 ~250 MB），refs 是冗余的

**修复:**
```rust
// main.rs: 在索引构建后立即 drop refs
let coll = BinSeqCollection::from_references(&refs);
let index = KmerIndex::build_wgbs(&coll, seed_size, ...);
drop(refs);  // ← 显式释放，尽早回收 ~3 GB
// 继续比对循环...
```

对于 RRBS 模式，`build_rrbs` 需要 `refs` 参数，需要在 drop 前调用。

**预估收益:** -3 GB 峰值内存，800+ MB → ~500 MB 稳定态。

---

## 严重优先级 — 速度（消除热路径冗余分配）

### P11-4: batch_raw.clone() → mem::take 消除深拷贝

**文件:** `bsmap/src/main.rs:528`

**问题:**
- `process_batch(batch_raw.clone(), 0, config)` 克隆整个 batch 的 Vec<RawRead>
- 每条 RawRead 包含 name: Vec<u8>, seq: Vec<u8>, qual: Vec<u8>
- BATCH_SIZE=50,000，每条读段 ~100bp，每批约 15 MB 的深拷贝
- `batch_raw` 紧接着被 `clear()`，数据本可以被"取走"而非克隆

**修复:**
```rust
// main.rs:528 — 当前
let reads = process_batch(batch_raw.clone(), 0, config);

// 改为
let reads = process_batch(std::mem::take(&mut batch_raw), 0, config);
// batch_raw 现在是空的（Vec::new()），下一轮循环可以复用
```

**预估收益:** 每批消除 ~15 MB 深拷贝，端到端速度 +3~5%。

---

### P11-5: mismatch_pattern_0/1 只取 .len() — 返回计数而非 Vec

**文件:** `bsmap/src/align/mismatch.rs:163-253`, `bsmap/src/align/gap.rs:152-160`

**问题:**
- `try_all_gaps()` 在热路径中调用 `mismatch_pattern_0()` 和 `mismatch_pattern_1()`
- 每次调用分配新的 `Vec<u32>` 存储 mismatch 位置
- 但调用者**只使用 `.len()`**，完全不关心具体位置（gap.rs:160: `left_positions.len() as u32`）
- `try_all_gaps` 是三重嵌套循环（gap_len × gap_pos × 读段），每批 50,000 reads 可调用数十万次

**修复:**
```rust
// mismatch.rs — 新增只计数的版本
#[inline]
pub fn count_mismatch_positions_0(
    query: &[u64],
    ref_seq: &[u64],
    offset: u32,
    map_readlen: u32,
    nt3: bool,
) -> u32 {
    // 与 mismatch_pattern_0 相同逻辑，但只累加计数，不 push 到 Vec
    let mut count = 0u32;
    // ... (相同的 diff 计算逻辑，找到 mismatch 时 count += 1)
    count
}

// gap.rs:152 — 调用处改为
let left_mm = count_mismatch_positions_0(query, ref_seq, ref_offset, left_len, nt3);
let right_mm = count_mismatch_positions_1(&query[...], ref_seq, right_ref_offset, right_len, nt3);
```

**风险:** `mismatch_pattern_0` 和 `mismatch_pattern_1` 仍有其他调用者需要完整位置列表。保留原函数，新增计数版本。

**预估收益:** gap 比对路径消除大量 Vec 分配，端到端速度 +5~10%。

---

### P11-6: ExtHit 中间层消除 — 直接构造 GHit

**文件:** `bsmap/src/align/extend.rs:24-52, 70-87, 212-226`

**问题:**
- `snp_align_segment` 收集 `Vec<ExtHit>`（u8/u8/i8 紧凑字段）
- `dedup_hits` 对 ExtHit 排序去重
- `snp_align_for_chain` 将 ExtHit 逐条 `to_ghit()` 转换为 GHit（u16/i16 宽字段）
- ExtHit 是**纯中间结构**：字段尺寸缩小版的 GHit
- 转换步骤 (`all_hits.into_iter().map(|h| h.to_ghit()).collect()`) 分配新的 `Vec<GHit>`

**分析:**
- ExtHit 存在的理由：字段更小（snps: u8 vs u8, gap_size: i8 vs i16, gap_pos: u8 vs u16）
- 但 dedup 仅按 (chr, loc, strand) 排序，不涉及 snps/gap
- GHit 已实现 `PartialEq + Eq + Copy`，可直接用于 dedup 和排序

**修复:**
```rust
// extend.rs — 在 snp_align_segment 中直接 push GHit
// 删除 ExtHit 结构体（或保留仅用于内部排序的轻量键）
all_hits.push(GHit {
    chr: chr / 2,
    loc,
    snps: mm_count as u8,
    strand,
    gap_size: 0,
    gap_pos: 0,
});

// dedup_hits 改为接受 &mut Vec<GHit>，按 (chr, loc, strand, snps) 排序去重
pub fn dedup_hits(hits: &mut Vec<GHit>) {
    hits.sort_unstable_by_key(|h| (h.chr, h.loc, h.strand, h.snps));
    hits.dedup_by(|a, b| a.chr == b.chr && a.loc == b.loc && a.strand == b.strand);
}
```

**预估收益:** 消除每条 read 的 ExtHit→GHit 转换分配，端到端速度 +3~5%。

---

## 重要优先级 — 热路径分配与间接开销

### P11-7: quick_gap_check 消除每调用 Vec 分配

**文件:** `bsmap/src/align/gap.rs:276-333`

**问题:**
- `quick_gap_check` 每次调用分配 `vec![u64::MAX; query.len()]`（全 1 掩码，表示无 N 过滤）
- 该掩码是常量模式，功能等价于跳过 N 过滤
- `count_mismatch` 被传入此全 1 掩码，执行 `diff &= m_word`（m_word = u64::MAX，即 no-op）
- 每批 50,000 reads 可调用数十万次

**修复:**
```rust
// 方案 A: 直接在 quick_gap_check 中内联无掩码的 mismatch 计数
// 方案 B: 使用 static 常量数组
static ALL_ONES_MASK: [u64; 32] = [u64::MAX; 32]; // 覆盖最长读段 (FIXELEMENT*SEGLEN/32)

// 调用时传引用
count_mismatch(query, ref_offset, ref_seq, &ALL_ONES_MASK[..query.len()], ...)
```

**预估收益:** 消除 gap 检测路径的 Vec 分配，边际增益。

---

### P11-8: get_reference_name 预计算 chr_accessions

**文件:** `bsmap/src/align/output.rs:296-309`

**问题:**
- `get_reference_name(chr, coll)` 每条比对记录调用一次
- 每次执行 `chr_names[chr_idx].split_whitespace().next()...to_string()` — 创建新 String
- 66,120 条记录 × 1 条染色体 = 66,120 次 split + to_string()
- 对于多染色体参考（人类 25 条），每次 split 返回相同结果

**修复:**
```rust
// BinSeqCollection 新增字段
pub chr_accessions: Vec<String>,  // 预处理的 accession 名

// from_references 中预计算
let chr_accessions: Vec<String> = refs.iter()
    .map(|r| r.name.split_whitespace().next().unwrap_or(&r.name).to_string())
    .collect();

// get_reference_name 改为直接索引
pub fn get_reference_name(chr: u32, coll: &BinSeqCollection) -> &str {
    let chr_idx = chr as usize;
    if chr_idx < coll.chr_accessions.len() {
        &coll.chr_accessions[chr_idx]
    } else {
        "unknown"
    }
}
```

**注意:** 返回类型从 `String` 改为 `&str`，调用者（format_sam/format_bsp 等）需同步调整。`write!` 宏接受 `&str`。

**预估收益:** 消除每条记录的 String 分配，端到端速度 +2~3%。

---

### P11-9: start_offsets 消除 fwd_write_offsets.clone() (~172 MB)

**文件:** `bsmap/src/reference/index.rs:175`

**问题:**
- `start_offsets = fwd_write_offsets.clone()` — 完整克隆 172 MB 的 Vec<u32>（seed_size=16, 43M 元素）
- `start_offsets` 用于 `lookup_separated` 的 O(1) 查找
- `fwd_write_offsets` 在 Pass 3 填充后被修改（写指针递增），因此不能直接重用
- 但可以在 Pass 3 开始**之前**保存一份快照，而非在 Pass 3 填充之后再克隆

**修复:**
```rust
// index.rs:175 — 在调用 fill_positions_chain 之前
// fwd_write_offsets 当前的值为每个 hash 的起始偏移（还未被 fill 修改）
// 直接保存为 start_offsets，无需 clone
let start_offsets = fwd_write_offsets.clone(); // ← 必须在 fill 之前保存
// 然后传入 &mut fwd_write_offsets 给 fill_positions_chain 修改
```

实际上当前代码**已经是**在 fill 之前 clone 的。但可以优化为：用一个 Vec 同时服务两个目的，在 fill 之前取出所有权。

```rust
// 更优方案：直接取出所有权
let start_offsets = std::mem::take(&mut fwd_write_offsets);
// 重建用于填充的写偏移（从 start_offsets 克隆回来用于填充）
let mut fwd_write_offsets = start_offsets.clone();
```

这反而增加了一次克隆。最简单的优化是使用 `Arc` 或共享引用。

**实际优化:**
fwd_write_offsets 需要被 fill_positions_chain 修改，而 start_offsets 需要原始值。最佳方案是使用 `split_at_mut` 风格的共享：在 fill 中不修改原数组，而是使用独立的写指针计数。

但考虑改动范围，当前 clone 开销 172 MB 是一次性的（仅在索引构建时），不影响比对阶段的内存。此项可降级或跳过。

**预估收益:** 节省 172 MB 临时分配于索引构建阶段，不影响比对常驻内存。

---

### P11-10: batch.rs 名字 from_utf8_lossy().to_string() 双分配

**文件:** `bsmap/src/reads/batch.rs:38`

**问题:**
- `String::from_utf8_lossy(&raw.name).to_string()` 创建中间 `Cow<str>`，然后 `.to_string()` 又创建新 String
- 当 name 是有效 UTF-8 时（绝大多数情况），`from_utf8_lossy` 返回 `Cow::Borrowed(&str)`，`.to_string()` 再分配
- 总共 2 次分配（一次 for Cow 的验证，一次 for 最终 String）
- 更高效的做法：直接 `String::from_utf8(raw.name)` 处理无效 UTF-8 时回退

**修复:**
```rust
// batch.rs:38 — 当前
let name = String::from_utf8_lossy(&raw.name).to_string();

// 改为（减少一次分配）
let name = String::from_utf8(raw.name).unwrap_or_else(|e| {
    String::from_utf8_lossy(e.as_bytes()).into_owned()
});
```

**预估收益:** 每批 50,000 reads 减少 ~50,000 次额外 String 分配，边际增益。

---

### P11-11: seed.rs segment Vec 分配复用

**文件:** `bsmap/src/align/seed.rs:200-234, 279-366`

**问题:**
- `reorder_seeds_for_chain` 为每个 segment 分配 3 个 Vec（seeds, reg_masks, seed_positions）
- `adjust_seed_starts_for_chain` 调整 start 后重新提取种子时，使用 `clear()` + 重新 `push()`
- Vec clear 保留容量（好），但多个 segment 间无法共享缓冲区
- 典型读段有 2-4 个 segments，每 segment 约 4 个种子

**分析:**
- 种子数很少（每 segment ≤ 4），Vec 分配开销相对小
- 主要开销在 index.lookup_separated() 和候选数统计（网络/内存访问）
- 此项边际增益可能 < 1%，可降级

**预估收益:** 边际（< 1%），建议降级到建议优先级。

---

### P11-12: extend.rs all_hits 预分配容量

**文件:** `bsmap/src/align/extend.rs:70, 111`

**问题:**
- `snp_align_for_chain` 和 `snp_align_segment` 的 `all_hits: Vec<ExtHit>` 使用 `Vec::new()` 无容量提示
- 候选命中数可从 index lookup 预估（positions.len()）
- 默认容量 0 导致多次 realloc（从 0→4→8→16→...）

**修复:**
```rust
// extend.rs:70
let estimated_hits = segments.iter()
    .map(|seg| seg.seeds.iter()
        .map(|&s| index.lookup_separated(s))
        .map(|(f, r)| f.len() + r.len())
        .sum::<usize>())
    .sum::<usize>()
    .min(max_hits);
let mut all_hits: Vec<ExtHit> = Vec::with_capacity(estimated_hits);
```

**风险:** 预估可能不准，但 `with_capacity` 的低估只是导致一次 realloc，不影响正确性。

**预估收益:** 减少 Vec 扩容次数，边际增益。

---

### P11-13: encode_read 中 info.clone() 避免克隆 ReadInf

**文件:** `bsmap/src/reads/encode.rs:84`

**问题:**
- `EncodedRead` 存储 `info: read.clone()` — 克隆整个 ReadInf（含 name: String, seq: Vec<u8>, qual: Vec<u8>）
- encode_read 之后，原始的 `ReadInf`（来自 `reads` Vec）仅用于输出格式化（取 name、seq、qual）
- 两个副本同时存在，每读段浪费 ~200 字节

**分析:**
- 当前架构中，`reads` 和 `encoded` 是独立的两个 Vec，都需要存活到输出阶段
- 重构为 EncodedRead 持有对 ReadInf 的引用（`&'a ReadInf`）会引入生命周期复杂性
- 更实际的方案：在输出阶段直接从 `reads` 中索引访问，EncodedRead 仅存储索引

**修复方案（保守）:**
```rust
// EncodedRead 不再存储 info: ReadInf，改为存储 read_idx
pub struct EncodedRead {
    pub fwd_words: Vec<u64>,
    pub rev_words: Vec<u64>,
    pub fwd_mask: Vec<u64>,
    pub rev_mask: Vec<u64>,
    pub read_idx: u32,      // ← 替代 info: ReadInf
    pub read_len: u32,      // ← 直接存储长度
    pub n_count: u32,
}
```

**风险:** 需要大量的调用点修改。短期可以不做，列为建议优先级。

**预估收益:** 减少 ~10 MB 每批的内存占用，边际增益。

---

### P11-14: PairAlign::do_pair_batch 并行化单端比对阶段

**文件:** `bsmap/src/pairs/pair.rs:511-575`

**问题:**
- `do_pair_batch` 采用串行 for 循环处理每对 read
- 而 `SingleAlign::do_batch` 已使用 `par_iter` 并行
- 配对批处理中，每对 read 的单端比对（align_a.run_align + align_b.run_align）是独立的
- 配对阶段（get_pairs）需要在两个单端比对完成后才能进行

**修复:**
```rust
// 阶段 1: 并行单端比对
let (results_a, results_b): (Vec<_>, Vec<_>) = rayon::join(
    || SingleAlign::do_batch(reads_a, index, coll, config),
    || SingleAlign::do_batch(reads_b, index, coll, config),
);

// 阶段 2: 串行配对（配对逻辑有状态，不适合直接并行）
for i in 0..batch_size {
    // 用 results_a[i] 和 results_b[i] 配对
}
```

目前的 do_pair_batch 已使用 `SingleAlign` 实例（通过 `self.align_a` / `self.align_b`），但由于配对逻辑需要访问 `self.pair_hits` 等状态，不能直接并行。

更好的方案是分离单端比对和配对：
1. 使用 `SingleAlign::do_batch` 并行比对所有 read_a 和 read_b
2. 然后串行配对

**预估收益:** 双端模式速度 +20~30%。

---

## 建议优先级 — 架构优化与代码质量

### P11-15: SAM 行 buffer 初始容量优化

**文件:** `bsmap/src/align/output.rs:57-90`

**问题:**
- `format_sam` 每次 `buf.clear()` 后写入 SAM 行
- 初始 buffer 容量从 0 开始增长，前几批会多次 realloc
- SAM 单行通常 100-300 字节

**修复:**
```rust
// 调用方预热 buffer 容量
let mut buf = String::with_capacity(512);
```

**预估收益:** 边际，减少前几批的 realloc。

---

### P11-16: 批处理管道化 — I/O/对齐/输出重叠

**文件:** `bsmap/src/main.rs:489-568`

**问题:**
- 当前批处理是串行的：读 I/O → 编码 → 比对 → 写 I/O → 下一批
- 三个阶段的硬件资源利用不同：
  - 读 I/O：磁盘 + needletail 解析（CPU 轻）
  - 比对：纯 CPU 密集
  - 写 I/O：磁盘 + noodles 编码（CPU 轻）
- 管道化可以让三个阶段在不同线程上重叠执行

**修复:**
```rust
use crossbeam_channel;

// 生产者线程：I/O 读取 + 编码
// 中间 channel：EncodedRead batch
// 消费者线程 1..N：比对
// 输出线程：写 SAM/BAM
```

**风险:** 管道化增加代码复杂度，当前性能已与 C++ 持平，如非必要可暂不实施。

**预估收益:** 理论 I/O 隐藏 +10~20%，但实现复杂度高。

---

### P11-17: SIMD 种子提取

**文件:** `bsmap/src/align/seed.rs:107-129`

**问题:**
- `extract_seed_at_pos` 逐位置提取种子（一次一个 32-bit XT3）
- 现代 x86_64 的 AVX2 可以一次处理更多

**修复:**
- 对连续位置的种子提取使用批量 SIMD
- 但种子位置由 profile 决定，通常不连续
- 收益有限，可作为研究方向

**预估收益:** 边际（< 3%），种子提取不在最热路径。

---

### P11-18: 参考序列 vtable dispatch 消除

**文件:** `bsmap/src/reference/binseq.rs:51-53`, `bsmap/src/reference/storage.rs:8-19`

**问题:**
- `refcat: Box<dyn BinSeqStorage>` 每次 `as_slice()` 调用有 dyn dispatch 开销
- P10 已实现在 extend.rs 中缓存 `ref_seq = coll.refcat.as_slice()`，但多处仍在热路径中重复调用

**修复:**
- 对所有热路径调用点审查：extend.rs、mismatch.rs、index.rs
- 确保 as_slice() 结果在循环外缓存为局部变量
- 实际上 extend.rs 中 `snp_align_segment` 已缓存（`ref_seq = if ref_chain == 0 { coll.refcat.as_slice() } else { coll.crefcat.as_slice() }`）

**当前状态:** 主要热路径已缓存，剩余调用点在索引构建阶段（不在比对热路径）。

**预估收益:** 边际（热路径已优化）。

---

### P11-19: 全局线程池规模自适应

**文件:** `bsmap/src/param.rs:219-221`

**问题:**
- `num_threads` 默认取 `available_parallelism().min(8)`，上限 8
- 在有更多核心的机器上（16+ 核），未能充分利用

**修复:**
```rust
let num_cpus = std::thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4);
```

当前 `min(8)` 限制了高端机器的性能。但需评估：线程过多可能导致竞争（锁、缓存抖动）。

**预估收益:** 在 16+ 核机器上速度 +10~20%。

---

### P11-20: BAM 输出路径的 PE 适配

**文件:** `bsmap/src/main.rs` PE 输出路径

**问题:**
- P10-6 实现了 BAM 直接构造 Record 的 SE 路径
- PE 路径（`output_pair_alignment` / `output_unpaired`）仍保留旧的 String→parse→BAM 路径

**修复:**
为 PE 路径实现类似的 `build_bam_record_pe()` 直接构造逻辑。

**预估收益:** PE BAM 输出速度 +2-3x（仅在 BAM 输出模式下有影响）。

---

## 执行路线图

```
Phase A (1-2天): P11-1 ~ P11-6 — 内存释放 + 热路径优化
├── P11-1: 释放 seqs 死数据 ✅           ← -3 GB 内存
├── P11-2: 消除 mask 构建                ← -1.5 GB 内存
├── P11-3: 索引后 drop refs              ← -3 GB 内存
├── P11-4: batch_raw.clone → mem::take   ← 消除每批深拷贝
├── P11-5: mismatch_pattern 计数版        ← 消除 gap 路径 Vec 分配
└── P11-6: ExtHit → GHit 直接构造        ← 消除中间转换
    目标: 峰值内存 889→~500 MB, 速度 +15~25%

Phase B (1-2天): P11-7 ~ P11-14 — 分配优化
├── P11-7: quick_gap_check Vec 分配消除
├── P11-8: chr_accessions 预计算
├── P11-9: start_offsets 克隆消除
├── P11-10: batch.rs 名字双分配
├── P11-11: seed Vec 复用 (可选)
├── P11-12: all_hits 预分配
├── P11-13: encode_read 避免 info 克隆 (可选)
└── P11-14: PairAlign 部分并行化
    目标: 速度 +10~15%, 内存进一步降低

Phase C (按需): P11-15 ~ P11-20 — 架构优化
├── P11-15: SAM buffer 容量
├── P11-16: 管道化 (复杂度高，按需)
├── P11-17: SIMD 种子提取 (研究)
├── P11-18: vtable 消除 (已有的审查)
├── P11-19: 线程池上限调整
└── P11-20: PE BAM 直接构造
    目标: 边际增益，长期维护
```

---

## 预期目标

| 指标 | P10 基线 | P11 目标 | 改善 |
|------|---------|---------|------|
| 峰值内存 (RSS) | 889 MB | ~500 MB | **-44%** |
| SE p=4 速度 | 1.43s | ~1.1s | **+30%** |
| 比对正确性 | 66,120 reads | 66,120 reads | 0 diff |
| SAM 输出一致性 | 基准 | 0 diff vs P10 | — |

---

## 验证标准

- [ ] 比对结果与 P10 完全一致（66,120 reads, 0 SAM diff）
- [ ] 峰值 RSS < 600 MB（目标 500 MB）
- [ ] SE p=4 速度 < 1.2s（目标 1.1s）
- [ ] 全部单元测试通过（`cargo test -p bsmap`）
- [ ] 4 场景测试通过（SE/PE WGBS/RRBS）
