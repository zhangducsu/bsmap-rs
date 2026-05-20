# Lambda SE150 端到端测试对比报告

## 测试时间
2026-05-13

## 测试数据
- **参考序列**: Lambda phage NC_001416.1 (48,502 bp)
- **测序数据**: SE150 单端模拟数据
- **Reads**: 9,700
- **覆盖度**: ~30x

## 比对结果对比

### C++ BSMAP 2.90
```
总 reads: 9700
比对 reads: 9700 (100.0%)
唯一比对: 9700 (100.0%)
非唯一比对: 0 (0.0%)
耗时: 1 秒
```

### Rust bsmap-rs
```
比对读段数: 0
唯一比对: 0
多重比对: 0
总耗时: 5.90s
```

## SAM 文件对比

| 指标 | C++ BSMAP | Rust bsmap-rs |
|------|-----------|---------------|
| 总行数 | 9,703 | 3 |
| 头部行 | 3 | 3 |
| 比对记录 | 9,700 | 0 |
| 比对率 | 100% | 0% |

## 关键差异

### 1. 比对成功率
- **C++ BSMAP**: 100% 成功比对 (9,700/9,700)
- **Rust bsmap-rs**: 0% 比对失败 (0/9,700)

### 2. SAM 头部差异

**C++ BSMAP**:
```
@HD	VN:1.0
@SQ	SN:NC_001416.1	LN:48502
@PG	ID:BSMAP	VN:2.90	CL:"..."
```

**Rust bsmap-rs**:
```
@HD	VN:1.0	SO:unsorted
@SQ	SN:NC_001416.1 Enterobacteria phage lambda, complete genome	LN:48502
@PG	ID:bsmap	PN:bsmap	VN:0.1.0
```

### 3. 比对记录

**C++ BSMAP** 示例:
```
1_NC_001416.1:25942-26091	0	NC_001416.1	25942	255	150M	*	0	0	TATTTGAATTTGTG...	IIIIIIIIIIIIII...	NM:i:0	ZS:Z:++
```

**Rust bsmap-rs**: 无任何比对记录

## 问题分析

### Rust bsmap-rs 的问题

从日志可以看出:
1. 索引构建成功
2. 单端比对开始
3. 大部分 reads 显示 `0 candidates with positions`
4. 少数 reads 有候选位置但 `0 passed mismatch check`

### 与之前测试的对比

| 数据集 | 类型 | C++ BSMAP | Rust bsmap-rs |
|--------|------|-----------|---------------|
| lambda_wgbs (PE150) | 双端 | 100% | 0% |
| **lambda_se150** | **单端** | **100%** | **0%** |
| ex1_small (SE32) | 单端 | 100% | 50% |

**关键发现**:
- Rust bsmap-rs 在 32bp 短读段上可比对 50%
- 但在 150bp 长读段上完全失败
- 这表明种子查找或扩展逻辑在较长读段上存在问题

### 可能的原因

1. **种子提取问题**: 150bp 读段的种子提取可能与 32bp 不同
2. **mismatch 阈值**: 长读段的 mismatch 容忍度设置可能过于严格
3. **种子位置**: 150bp 读段可能跨越更多种子位置，需要不同的处理

## 调试信息分析

从 Rust 日志中可以看到:
```
[DEBUG] read=9659_NC_001416.1:38181-38330, first_seed=27257603, fwd=1, rev=0
[INFO] WGBS read_chain=0: 1 candidates with positions, 1 total positions checked, 0 passed mismatch check
```

这表明:
1. 找到了候选位置 (1 candidates)
2. 但在 mismatch 检查时失败 (0 passed mismatch check)

问题可能出在 mismatch 检查逻辑，而不是种子查找。

## 建议修复方向

1. **检查 mismatch 检查逻辑**:
   - 对比 32bp 和 150bp 读段的 mismatch 计算
   - 检查 BS 转换后的碱基匹配逻辑

2. **调试种子扩展**:
   - 添加更多调试日志
   - 对比 C++ 和 Rust 的种子扩展结果

3. **验证种子提取**:
   - 确保 150bp 读段的种子提取正确
   - 检查种子哈希是否匹配参考序列

## 结论

C++ BSMAP 能够完美比对 100% 的 Lambda SE150 数据，而 Rust bsmap-rs 完全失败。这与之前 ex1_small (32bp) 测试中 50% 比对率形成鲜明对比，表明 Rust 实现在处理较长读段时存在严重问题。

## 后续行动

1. 优先修复 Rust bsmap-rs 的长读段比对逻辑
2. 重点检查 mismatch 检查和种子扩展逻辑
3. 添加详细的调试日志以定位问题
4. 重新测试验证修复效果
