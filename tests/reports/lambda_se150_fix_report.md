# Lambda SE150 Bug 修复报告

## 修复时间
2026-05-13

## 问题描述

Rust bsmap-rs 在比对 Lambda SE150 数据时完全失败（0% 比对率），而 C++ BSMAP 可以完美比对（100%）。

## 根本原因分析

经过多轮迭代调试，发现以下关键问题：

### 1. Bug #15: seg_snp_thres 阈值错误 (extend.rs)

**原代码** (`bsmap/src/align/extend.rs`):
```rust
let seg_snp_thres = if mode < profile.len() {
    mode as u32  // 错误：mode=0 时 threshold=0，不允许任何 mismatch
} else {
    snp_thres
};
```

**问题**: `mode` 是 segment 索引，当 `mode=0` 时，`seg_snp_thres=0`，不允许任何 mismatch。

**修复**:
```rust
let seg_snp_thres = snp_thres;
```

**效果**: 0% → 1.6%

### 2. 种子起始偏移优化缺失 (seed.rs)

**问题**: C++ BSMAP 的 `ReorderSeed` 算法缺失，无法对齐读段种子与索引网格。

**修复**:
- 修改 `extract_seeds` 提取所有位置种子（每 1bp）
- 实现 `find_best_start_offset` 选择最佳偏移
- 在 `reorder_seeds` 中应用最佳偏移

**效果**: 1.6% → 1.6%（种子开始找到索引命中）

### 3. mismatch.rs 参考序列位提取方向错误

**原代码** (`bsmap/src/align/mismatch.rs`):
```rust
let ref_low = ref_seq[word_offset + i] >> shift_left;
let ref_high = ref_seq[word_offset + i + 1] << shift_right;
```

**问题**: 移位方向与 `make_seed` 相反，导致参考序列错误对齐。

**修复**:
```rust
let ref_low = ref_seq[word_offset + i] << shift_left;
let ref_high = ref_seq[word_offset + i + 1] >> shift_right;
```

**效果**: 1.6% → 49.2%

### 4. 4 链比对支持缺失 (extend.rs)

**问题**: 只实现了 2 种链组合（++ 和 --），缺失交叉链组合（+- 和 -+）。

**修复**:
- 重构 WGBS 模式循环，支持 4 链比对
- `strand` 编码: `ref_chain << 1 | read_chain`
- 每个读段链同时查询正向和反向参考位置

**效果**: 49.2% → 100%

### 5. 反向链坐标转换错误 (output.rs)

**问题**: `crefcat` 坐标未正确转换为正向参考坐标。

**修复**:
```rust
// 反向链：hit.loc 是 crefcat 上的相对位置
let chr_len = get_chromosome_length(hit.chr, coll);
chr_len - hit.loc - read.seq.len() as u32 + 1
```

### 6. SAM FLAG 和序列方向错误 (output.rs)

**问题**: FLAG 0x10 和序列方向基于读段链而非参考链。

**修复**:
```rust
// 对于反向参考链（-+ 和 --），设置 0x10 标志并输出反向互补序列
if !chain.is_ref_forward() {
    flag |= 0x10;
}
let rev_seq = !chain.is_ref_forward();
```

## 修复效果

### 最终比对结果

| 指标 | C++ BSMAP | Rust bsmap-rs (修复后) | 状态 |
|------|-----------|------------------------|------|
| 比对读段数 | 9,700 (100%) | 9,700 (100%) | ✅ |
| 唯一比对 | 9,700 | 9,700 | ✅ |
| `++` 链 | 4,770 | 4,770 | ✅ |
| `-+` 链 | 4,930 | 4,930 | ✅ |
| POS 匹配 | - | - | 0 个不匹配 |
| FLAG 匹配 | - | - | 0 个不匹配 |

### 修复进度

| 阶段 | 比对率 | 修复内容 |
|------|--------|----------|
| 初始 | 0% | - |
| Bug #15 修复 | 1.6% | seg_snp_thres 阈值 |
| P2 修复 | 1.6% | 种子起始偏移优化 |
| mismatch 修复 | 49.2% | 参考序列位提取方向 |
| 4 链支持 | 100% | 完整链组合支持 |
| 坐标转换 | 100% | 反向链坐标转换 |
| SAM 格式 | 100% | FLAG 和序列方向 |

## 修复文件列表

- `bsmap/src/align/extend.rs`: seg_snp_thres 修复、4 链比对支持
- `bsmap/src/align/seed.rs`: 种子起始偏移优化、seed_positions 字段
- `bsmap/src/align/mismatch.rs`: 参考序列位提取方向
- `bsmap/src/align/output.rs`: 反向链坐标转换、SAM FLAG 和序列方向

## 关键发现

1. **种子哈希一致性**: 读段和参考序列使用相同的 `xt3` 哈希函数，C/T 合并到同一桶。
2. **4 链比对必要性**: WGBS 模式需要完整的 4 链比对（++、+-、-+、--）才能达到 100% 比对率。
3. **坐标转换复杂性**: 反向链（crefcat）坐标需要正确转换为正向参考坐标。
4. **SAM 格式规范**: FLAG 0x10 和序列方向应基于参考链而非读段链。

## 验证结果

所有 9,700 个读段的 POS 和 FLAG 与 C++ BSMAP 完全匹配，0 个不匹配。

## 结论

Rust bsmap-rs 现已达到与 C++ BSMAP 2.90 完全一致的比对结果，Lambda SE150 数据比对率达到 100%。
