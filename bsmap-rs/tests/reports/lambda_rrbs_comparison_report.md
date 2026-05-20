# Lambda RRBS 比对测试报告

> 生成日期: 2026-05-14 09:12:01

## 测试数据

| 参数 | 值 |
|------|-----|
| 参考序列 | Lambda phage NC_001416.1 (48,502 bp) |
| 测序类型 | PE150 (双端 150bp) |
| 酶切 | MspI (CCGG) |
| 大小选择 | 150-220 bp |
| Read pairs | 1,844 |
| 生成方式 | 自定义 Python 脚本 |

## 发现问题

### 1. C++ BSMAP gzip 处理 bug

**问题**: C++ BSMAP 在处理某些 gzip 压缩文件时发生 buffer overflow 崩溃

** workaround**: 使用未压缩的 FASTQ 文件

```bash
# 崩溃
./bsmap -a R1.fastq.gz -b R2.fastq.gz ...

# 正常
zcat R1.fastq.gz > R1.fq
zcat R2.fastq.gz > R2.fq
./bsmap -a R1.fq -b R2.fq ...
```

### 2. C++ BSMAP 配对逻辑差异

**问题**: 在 RRBS 数据集上，C++ BSMAP 无法找到配对，所有 reads 输出为单端

| 工具 | 配对数 | 单端数 | 总记录数 |
|------|--------|--------|----------|
| C++ BSMAP | 0 | 1844 | 1844 |
| Rust bsmap-rs | 1844 | 0 | 3688 |

**分析**: 
- Rust 成功将 1,844 对 reads 全部配对
- C++ 将 1,844 对 reads 全部作为单端处理（只输出 R1）

### 3. 比对位置差异

C++ 和 Rust 的比对位置不同，说明它们选择了不同的 hit。

**示例 (QNAME=000000)**:

| 字段 | C++ BSMAP | Rust bsmap-rs |
|------|-----------|---------------|
| FLAG | 73 (0x49) | 65 (0x41) / 145 (0x91) |
| RNAME | NC_001416.1 | NC_001416.1 |
| POS | 39460 | 39460 / 39504 |
| CIGAR | 150M | 150M |
| SEQ | GTTAAATTT... | GTTAAATTT... / CAAACAAC... |

**说明**:
- C++: 只输出一条记录（R1），FLAG 73 = paired + mate_unmapped + first_in_pair
- Rust: 输出两条记录（配对），FLAG 65 = paired + first_in_pair, FLAG 145 = paired + mate_reverse + second_in_pair

## 结论

1. **C++ BSMAP 存在 gzip 处理 bug**，需要使用未压缩文件 workaround
2. **C++ BSMAP 在 RRBS 数据集上配对逻辑失效**，所有 reads 作为单端处理
3. **Rust bsmap-rs 在 RRBS 数据集上配对逻辑正常工作**，能够正确配对

## 建议

1. 修复 C++ BSMAP 的 gzip 处理 bug
2. 调查 C++ BSMAP 在 RRBS 数据集上的配对逻辑问题
3. 使用未压缩 FASTQ 文件作为 C++ BSMAP 的输入 workaround

## 命令记录

```bash
# C++ BSMAP (使用未压缩文件)
./bsmap -a R1.fq -b R2.fq -d genome.fa -o cpp.sam -n 0 -p 1 -v 0.08 -m 28 -x 500

# Rust bsmap-rs
./bsmap align -a R1.fq -b R2.fq -d genome.fa -o rust.sam -n 0 -p 1 -v 0.08 -m 28 -x 500
```
