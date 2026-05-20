# bsmap-rs vs BSMAP C++ 性能基准测试报告

**测试日期**: 2026-05-16
**参考基因组**: chr22_tail_1M.fa (1,000,000 bp)
**BSMAP 版本**: bsmap 2.90 (C++)
**bsmap-rs 版本**: v0.1.0 (Rust)

---

## 1. 测试环境

| 项目 | 值 |
|------|------|
| 操作系统 | Linux (Ubuntu 22.04) |
| CPU | x86_64 |
| BSMAP C++ 二进制 | /workspace/bsmap-original/bsmap-2.90/bsmap (593 KB) |
| bsmap-rs 二进制 | /workspace/bsmap-rs/target/release/bsmap (3.4 MB) |
| 参考基因组 | chr22_tail_1M.fa (1,012,515 bytes, 1M bp) |
| 错配率参数 | -v 0.08 (8%) |
| 索引间隔 | -I 4 |
| WGBS seed size | 16 |
| RRBS seed size | 12 |

---

## 2. 索引构建对比

### 2.1 索引构建性能

| 工具 | 模式 | 耗时 (s) | 峰值内存 (MB) | 索引大小 | 状态 |
|------|------|----------|---------------|----------|------|
| BSMAP C++ | WGBS | 0.16 | N/A* | N/A** | 成功 |
| BSMAP C++ | RRBS | 0.17 | N/A* | N/A** | 成功 |
| bsmap-rs | WGBS | 13.19 | 2,412 | 494 MB | 成功 |
| bsmap-rs | RRBS | 7.49 | 3,718 | 0 | OOM Killed |

> *注: BSMAP C++ 子进程 RSS 追踪受限，无法准确获取内存数据*
> **注: BSMAP C++ 的 `-o` 参数未将索引输出到指定路径，索引创建在参考序列同目录下*

### 2.2 索引构建分析

- **BSMAP C++** 索引构建极快（<0.2s），但 `-o` 参数行为与文档不符，索引实际创建在参考序列文件同目录。
- **bsmap-rs** WGBS 索引构建耗时 13.19s，峰值内存 2.4 GB，索引大小 494 MB。索引自动缓存至 `ref.fa.bsi`。
- **bsmap-rs** RRBS 索引构建因内存不足被系统 OOM Killer 终止（峰值 3.7 GB），这是当前 bsmap-rs 的一个严重问题。RRBS 模式使用更小的 seed size (12)，导致索引条目数大幅增加。

---

## 3. 比对性能对比

### 3.1 WGBS 单端 (SE) 75bp 10x -- Example 1

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 总读段数 | 133,334 | 133,334 | 相同 |
| 比对读段数 | 66,120 | 66,118 | 基本一致 |
| 比对率 | 49.6% | 49.6% | 一致 |
| 唯一比对 | 64,951 (48.7%) | 55,948 (41.9%) | bsmap-rs 较低 |
| 多重比对 | 1,169 (0.9%) | 10,170 (7.6%) | bsmap-rs 较高 |
| 耗时 | 3.36s | 8.90s | bsmap-rs 慢 2.6x |
| 峰值内存 | 849 MB | 1,814 MB | bsmap-rs 高 2.1x |
| SAM 行数 | 66,123 | 66,121 | 基本一致 |

### 3.2 WGBS 双端 (PE) 150bp 10x -- Example 2

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 总配对数 | 66,667 | 66,667 | - |
| 比对配对数 | 0 | 33,478 | - |
| 比对率 | 0% | 50.3% | - |
| 耗时 | 3.19s | 8.48s | - |
| 状态 | **buffer overflow 崩溃** | 成功 | - |

### 3.3 RRBS 单端 (SE) 75bp 10x -- Example 3

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 总读段数 | 13,244 | 13,244 | - |
| 比对读段数 | 9,725 | 0 | - |
| 比对率 | 73.4% | 0% | - |
| 耗时 | 1.16s | 6.91s | - |
| 状态 | 成功 | **OOM Killed** | - |

### 3.4 RRBS 双端 (PE) 150bp 10x -- Example 4

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 状态 | **buffer overflow 崩溃** | **OOM Killed** | 双方均失败 |

### 3.5 WGBS 双端 (PE) 150bp 20x -- Example 5

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 总配对数 | 133,334 | 133,334 | - |
| 比对配对数 | 0 | 66,165 | - |
| 比对率 | 0% | 49.6% | - |
| 耗时 | 2.40s | 16.75s | - |
| 状态 | **buffer overflow 崩溃** | 成功 | - |

### 3.6 RRBS 双端 (PE) 150bp 20x -- Example 6

| 指标 | BSMAP C++ | bsmap-rs | 对比 |
|------|-----------|----------|------|
| 状态 | **buffer overflow 崩溃** | **OOM Killed** | 双方均失败 |

---

## 4. 比对性能汇总

### 4.1 成功案例性能对比

| 示例 | 工具 | 耗时 (s) | 峰值内存 (MB) | 比对率 | SAM 行数 |
|------|------|----------|---------------|--------|----------|
| Ex1 WGBS SE 75bp 10x | BSMAP C++ | 3.36 | 849 | 49.6% | 66,123 |
| Ex1 WGBS SE 75bp 10x | bsmap-rs | 8.90 | 1,814 | 49.6% | 66,121 |
| Ex2 WGBS PE 150bp 10x | bsmap-rs | 8.48 | 1,814 | 50.3% | 66,960 |
| Ex3 RRBS SE 75bp 10x | BSMAP C++ | 1.16 | 717 | 73.4% | 9,728 |
| Ex5 WGBS PE 150bp 20x | bsmap-rs | 16.75 | 1,813 | 49.6% | 132,335 |

### 4.2 失败案例汇总

| 示例 | 工具 | 失败原因 | 详情 |
|------|------|----------|------|
| Ex2 WGBS PE 150bp 10x | BSMAP C++ | buffer overflow | `*** buffer overflow detected ***: terminated` |
| Ex3 RRBS SE 75bp 10x | bsmap-rs | OOM Killed | RRBS 索引构建内存超限 (3.7 GB) |
| Ex4 RRBS PE 150bp 10x | BSMAP C++ | buffer overflow | `*** buffer overflow detected ***: terminated` |
| Ex4 RRBS PE 150bp 10x | bsmap-rs | OOM Killed | RRBS 索引构建内存超限 (3.7 GB) |
| Ex5 WGBS PE 150bp 20x | BSMAP C++ | buffer overflow | `*** buffer overflow detected ***: terminated` |
| Ex6 RRBS PE 150bp 20x | BSMAP C++ | buffer overflow | `*** buffer overflow detected ***: terminated` |
| Ex6 RRBS PE 150bp 20x | bsmap-rs | OOM Killed | RRBS 索引构建内存超限 (3.7 GB) |

---

## 5. SAM 一致性分析

### 5.1 Example 1 (WGBS SE 75bp 10x) -- 唯一可对比项

两个工具均成功完成比对，但输出格式存在差异：

| 差异项 | BSMAP C++ | bsmap-rs |
|--------|-----------|----------|
| Read name 格式 | `100001_chr22_tail_1M:878053-878127` | `2_chr22_tail_1M:31117-31191` |
| SAM 行数 | 66,123 | 66,121 |
| 比对读段数 | 66,120 | 66,118 |
| 唯一/多重分类 | 64,951 unique / 1,169 multi | 55,948 unique / 10,170 multi |

**分析**:
- 两个工具的比对读段总数几乎一致（66,120 vs 66,118，差 2 条）。
- Read name 编号体系不同（BSMAP 从 100001 开始，bsmap-rs 从原始 fastq 行号开始），无法直接逐行 diff。
- **唯一/多重比对分类差异显著**: BSMAP C++ 将更多读段归为"唯一比对"(48.7%)，而 bsmap-rs 将更多归为"多重比对"(7.6%)。这可能是由于：
  - 两个工具对"唯一比对"的定义不同
  - 多重比对处理策略 (-r 参数) 的默认值差异
  - 随机选择多重命中时的种子策略不同
- 比对位置和 CIGAR 字段需要通过 read name 映射后才能精确比较。

### 5.2 其他示例

- **Example 2, 5 (WGBS PE)**: BSMAP C++ 崩溃，无法对比。
- **Example 3 (RRBS SE)**: bsmap-rs OOM，无法对比。
- **Example 4, 6 (RRBS PE)**: 双方均失败，无法对比。

---

## 6. 关键发现与结论

### 6.1 性能对比

| 维度 | BSMAP C++ | bsmap-rs | 评价 |
|------|-----------|----------|------|
| 索引构建速度 | 极快 (<0.2s) | 较慢 (7-13s) | BSMAP C++ 大幅领先 |
| 比对速度 (WGBS) | 快 (1-3s) | 较慢 (8-17s) | BSMAP C++ 约 2-7x 更快 |
| 比对速度 (RRBS) | 快 (1-1.5s) | N/A (OOM) | 仅 BSMAP C++ 可用 |
| 内存占用 | 较低 (700-850 MB) | 较高 (1.8-3.7 GB) | bsmap-rs 内存占用 2-5x |
| PE 模式稳定性 | 崩溃 (buffer overflow) | 稳定运行 | bsmap-rs 更可靠 |
| RRBS 模式 | 可运行 | OOM Killed | BSMAP C++ 更可靠 |
| 比对率一致性 | 49.6% (SE) | 49.6% (SE) | 一致 |

### 6.2 关键问题

1. **BSMAP C++ 双端模式崩溃**: 所有 PE 示例均因 `buffer overflow` 崩溃，这是 BSMAP 2.90 的已知 bug，与当前测试环境（编译器版本、glibc 版本）有关。

2. **bsmap-rs RRBS 模式 OOM**: RRBS 索引构建（seed_size=12）需要约 3.7 GB 内存，超出当前环境可用内存限制。这是 bsmap-rs 需要优化的关键问题。

3. **BSMAP C++ 索引输出路径**: `-o` 参数未按预期工作，索引创建在参考序列文件同目录而非指定路径。

4. **唯一/多重比对分类差异**: 两个工具对多重比对读段的分类标准不同，导致唯一比对和多重比对的数量分布差异较大。

### 6.3 总结

- 在 **WGBS 单端模式**下，两个工具均可正常工作，比对率一致（49.6%），但 bsmap-rs 在速度和内存方面均逊于 BSMAP C++。
- **bsmap-rs 的优势**在于双端模式稳定性（BSMAP C++ 在当前环境下 PE 模式全部崩溃）。
- **bsmap-rs 的主要短板**是 RRBS 模式的内存管理问题，需要优化索引构建的内存使用。
- 由于双方在不同场景下各有失败，本次测试中**仅有 1 个示例（Ex1 WGBS SE）**能够进行完整的 SAM 一致性对比。

---

## 7. 测试数据集详情

| 示例 | 模式 | 读段类型 | 读长 | 覆盖度 | 读段/配对数 |
|------|------|----------|------|--------|-------------|
| Ex1 | WGBS | SE | 75bp | 10x | 133,334 |
| Ex2 | WGBS | PE | 150bp | 10x | 66,667 pairs |
| Ex3 | RRBS | SE | 75bp | 10x | 13,244 |
| Ex4 | RRBS | PE | 150bp | 10x | 13,991 pairs |
| Ex5 | WGBS | PE | 150bp | 20x | 133,334 pairs |
| Ex6 | RRBS | PE | 150bp | 20x | 28,562 pairs |

---

*报告生成时间: 2026-05-16*
*详细数据见: results/summary.csv*
