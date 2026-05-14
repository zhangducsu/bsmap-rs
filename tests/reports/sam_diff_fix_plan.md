# SAM 输出差异修复计划

> **更新日期**: 2026-05-14
>
> **测试数据**: BSBolt 模拟 Lambda PE150 WGBS (4,850 pairs)
>
> **最终状态**: ✅ 全部 9,010 条记录与 C++ BSMAP 完全一致（0 差异）

## 当前状态

### 已修复问题（Phase 0-5）

| 问题 | 状态 | 说明 |
|------|------|------|
| Header 格式 | ✅ 已修复 | @HD/@SQ/@PG 对齐 C++ |
| MAPQ 硬编码 255 | ✅ 已修复 | 与 C++ 一致 |
| 单端 POS 坐标转换 | ✅ 已修复 | 反向链正确转换 |
| 双端 FLAG 计算 | ✅ 已修复 | effective_chain 逻辑 |
| 双端 POS/PNEXT 坐标转换 | ✅ 已修复 | 反向链正确转换 |
| 双端 insert size 计算 | ✅ 已修复 | 使用转换后坐标 |
| 双端 QNAME _R1/_R2 后缀 | ✅ 已修复 | strip_r_suffix |

### 已修复问题（Phase 6）

| 问题 | 状态 | 说明 |
|------|------|------|
| **P1: ZS tag 错误** | ✅ 已修复 | read_b 使用 `!chain` |
| **P2: 未输出单端记录** | ✅ 已修复 | 有 hit 的未配对 reads 始终输出 |
| **P3: 配对数差异** | ✅ 已修复 | `count_mismatch` N/padding 处理逻辑 |
| **P4: QNAME /1//2 后缀** | ✅ 已修复 | strip_r_suffix 扩展支持 |

### P3 修复详情

**根本原因**：`count_mismatch` 中 N 碱基/padding 处理逻辑错误

| Bug | 描述 | 修复 |
|-----|------|------|
| `diff \|= !mask` | padding 位置被错误计入 mismatch，导致提前终止误判 | 改为 `diff &= mask` |
| `n_count` 包含 padding | `count_n_in_mask` 统计了 read 末尾填充位置（mask=0） | 只统计 read_len 范围内的 N |
| 提前终止时机错误 | `total_mismatches` 包含 padding mismatch，在 `saturating_sub(n_count)` 之前就触发提前返回 | 初始值改为 `n_count`，不再需要 `saturating_sub` |

**修复文件**：
- `bsmap/src/align/mismatch.rs`: 标量版 `diff &= m_word` + `total_mismatches = n_count`；AVX2 版同步修改
- `bsmap/src/align/extend.rs`: `count_n_in_mask(mask, read_len)` 增加 read_len 参数

**修复结果**：

| 指标 | 修复前 | 修复后 | C++ BSMAP |
|------|--------|--------|-----------|
| 配对数 | 3,870 | **4,186** | 4,186 ✅ |
| 单端 a | 458 | 311 | 311 ✅ |
| 单端 b | 462 | 327 | 327 ✅ |

### 已修复问题（Phase 7）

| 问题 | 状态 | 说明 |
|------|------|------|
| **P5: 单端 FLAG reverse 错误** | ✅ 已修复 | `read_chain ^ ref_chain` 替代 `ref_chain == 1` |
| **P6: RNAME 格式** | ✅ 已修复 | `get_reference_name` 截取空格前部分 |
| **P7: 单端 ZS tag 格式** | ✅ 已修复 | 添加 `ZS:Z:` 前缀 |

### P5 修复详情

**根本原因**：`format_unpair_sam_single` 中 FLAG 0x10 的计算逻辑与 C++ 不一致

| | Rust（修复前） | C++ |
|--|---------------|-----|
| 公式 | `ref_chain == 1` | `read_chain ^ ref_chain` |
| ++ (0,0) | false ✅ | false ✅ |
| +- (0,1) | false ❌ | true |
| -+ (1,0) | true ✅ | true ✅ |
| -- (1,1) | true ❌ | false |

当 `read_chain == ref_chain`（++ 和 --）时，Rust 的 reverse 判断与 C++ 相反。

**修复**：将 `!Chain::from_strand(hit.strand).is_ref_forward()` 改为 `(read_chain ^ ref_chain) == 1`

### P7 修复详情

**根本原因**：`format_unpair_sam_single` 的 format 字符串中 ZS tag 缺少 `ZS:Z:` 前缀

**修复**：将 `\t{}` 改为 `\tZS:Z:{}`

### 最终验证结果

| 指标 | Rust bsmap-rs | C++ BSMAP |
|------|---------------|-----------|
| 总记录数 | 9,010 | 9,010 |
| 共同记录 | 9,010 | 9,010 |
| 仅 Rust | 0 | - |
| 仅 C++ | - | 0 |
| **字段差异** | **0** | **0** |

**所有 13 个字段完全一致**：QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL, NM, ZS

---

## 修改的文件汇总

| 文件 | Phase | 修改内容 |
|------|-------|---------|
| `pairs/output.rs` | 6 | P1: ZS tag read_b 使用 `!chain` |
| `main.rs` | 6 | P2: 有 hit 的未配对 reads 始终输出 |
| `align/output.rs` | 6 | `get_reference_name` 改为 `pub` |
| `align/mismatch.rs` | 6 | P3: `diff &= mask`, `total_mismatches = n_count`；标量+AVX2 |
| `align/extend.rs` | 6 | P3: `count_n_in_mask(mask, read_len)` |
| `align/output.rs` | 7 | P6: `get_reference_name` 截取空格前部分 |
| `main.rs` | 7 | P5: 单端 FLAG 0x10 改为 `read_chain ^ ref_chain` |
| `main.rs` | 7 | P7: ZS tag 添加 `ZS:Z:` 前缀 |

---

## 验证步骤

```bash
# 1. 修复后重新编译
cargo build --release

# 2. 运行 BSBolt 测试数据
./target/release/bsmap align \
  -a tests/data/lambda_wgbs_sim/R1.fastq.gz \
  -b tests/data/lambda_wgbs_sim/R2.fastq.gz \
  -d tests/data/lambda_wgbs/reference/genome.fa \
  -o rust_fixed.sam -n 0 -p 1 -v 0.08 -m 28 -x 1000

# 3. 对比 C++ 输出
python3 compare_sam.py cpp_bsbolt.sam rust_fixed.sam
```

**最终结果**: ✅ 全部 9,010 条记录，13 个字段，0 差异
