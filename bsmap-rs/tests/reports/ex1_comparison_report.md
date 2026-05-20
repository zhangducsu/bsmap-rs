# BSMAP C++ vs Rust 端到端测试对比报告

## 测试数据
- **参考序列**: `ref_ex1.fa` (2 条序列, 共 3158 bp)
- **读段**: `reads_ex1.fq` (10 条 32bp 读段)

## 比对结果对比

### C++ BSMAP 2.90
- **比对读段数**: 10/10 (100%)
- **唯一比对**: 10/10 (100%)

### Rust bsmap-rs
- **比对读段数**: 5/10 (50%)
- **唯一比对**: 5/10 (50%)

## 详细对比

| 读段 | C++ BSMAP | Rust bsmap-rs | 位置 | 状态 |
|------|-----------|---------------|------|------|
| read_1 | seq1:1 | seq1:1 | 位置 0-31 | ✅ 匹配 |
| read_2 | seq1:17 | - | 位置 16-47 | ❌ Rust 未比对 |
| read_3 | seq1:33 | seq1:33 | 位置 32-63 | ✅ 匹配 |
| read_4 | seq1:49 | - | 位置 48-79 | ❌ Rust 未比对 |
| read_5 | seq1:65 | seq1:65 | 位置 64-95 | ✅ 匹配 |
| read_6 | seq1:81 | - | 位置 80-111 | ❌ Rust 未比对 |
| read_7 | seq1:97 | seq1:97 | 位置 96-127 | ✅ 匹配 |
| read_8 | seq1:113 | - | 位置 112-143 | ❌ Rust 未比对 |
| read_9 | seq1:129 | seq1:129 | 位置 128-159 | ✅ 匹配 |
| read_10 | seq1:145 | - | 位置 144-175 | ❌ Rust 未比对 |

## 观察到的模式

Rust bsmap-rs 成功比对了位置为 **奇数倍数 32** 的读段：
- read_1: 位置 0 (0 * 32)
- read_3: 位置 32 (1 * 32)
- read_5: 位置 64 (2 * 32)
- read_7: 位置 96 (3 * 32)
- read_9: 位置 128 (4 * 32)

失败的读段位置为 **16 + 偶数倍数 32**：
- read_2: 位置 16
- read_4: 位置 48
- read_6: 位置 80
- read_8: 位置 112
- read_10: 位置 144

## 技术差异

### MAPQ 质量值
- C++ BSMAP: 使用 255 (未定义)
- Rust bsmap-rs: 使用 40 (固定值)

### SAM 头部
- C++ BSMAP: 包含 `@PG` 行的命令行信息
- Rust bsmap-rs: `@HD` 行包含 `SO:unsorted`

## 修复的 Bug

### Bug #14: 种子提取位操作错误
**问题**: `extract_seed_at_pos` 函数在处理 `bit_offset > 0` 时的位操作逻辑错误。

**修复**: 修改了 `make_seed` 和 `extract_seed_at_pos` 函数，正确处理种子跨越 word 边界的情况：
- 当 `bit_offset = 0`: 直接使用 `words[word_idx]`
- 当种子完全在当前 word 内: 使用 `(words[word_idx] >> bit_offset) >> seed_bits_lz`
- 当种子跨越 word 边界: 使用 `(words[word_idx] << bit_offset) | (words[word_idx + 1] >> (64 - bit_offset))`

## 剩余问题

Rust bsmap-rs 仍有 5 个读段未能比对。从调试日志看：
- 这些读段都有候选位置 (fwd=2)
- 但在 mismatch 检查时失败 (0 passed mismatch check)

这表明可能存在额外的比对过滤逻辑问题，需要进一步调查。

## 结论

种子提取 bug 修复后，Rust bsmap-rs 的比对率从 10% 提升到 50%。剩余的 50% 读段失败可能与 mismatch 检查或其他比对后处理逻辑有关，需要进一步分析。
