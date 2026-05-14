# Lambda DNA SE150 单端测序数据

## 数据概述

本数据集包含基于 Lambda DNA 生成的 Illumina SE150 单端测序模拟数据，适用于 bsmap-rs 的单端比对功能测试。

## 生成信息

- **生成时间**: 2026-05-13
- **生成工具**: Sherman v0.1.9
- **参考序列**: Lambda phage NC_001416.1 (48,502 bp)

## 数据参数

| 参数 | 值 |
|------|-----|
| 测序类型 | 单端 (Single-end) |
| 读长 | 150 bp |
| 覆盖度 | ~30x |
| Read 数量 | 9,700 |
| 错误率 | 0% |
| Phred 质量值 | 40 (恒定) |
| BS 转换率 | 99.5% |

## 文件说明

```
lambda_se150/
├── reads/
│   └── reads.fastq.gz    # SE150 reads (483 KB)
└── reference/
    └── genome.fa         # Lambda DNA 参考序列 (49 KB)
```

## 使用示例

### C++ BSMAP

```bash
cd /workspace/bsmap-original/bsmap-2.90
./bsmap \
    -a /workspace/bsmap-rs/tests/data/lambda_se150/reads/reads.fastq.gz \
    -d /workspace/bsmap-rs/tests/data/lambda_se150/reference/genome.fa \
    -o output.sam
```

### Rust bsmap-rs

```bash
cd /workspace/bsmap-rs
./target/release/bsmap \
    -a tests/data/lambda_se150/reads/reads.fastq.gz \
    -d tests/data/lambda_se150/reference/genome.fa \
    -o output.sam
```

## 数据验证

- **Read 数量**: 9,700
- **文件大小**: ~483 KB (gzip 压缩)
- **格式**: FASTQ, Phred+33 质量值

## 特点

1. **适合云端测试**: 文件小 (~483 KB)，传输和存储成本低
2. **已知参考**: 基于标准 Lambda DNA，便于验证比对准确性
3. **BS 转换**: 模拟真实的亚硫酸氢盐测序数据 (99.5% 转换率)
4. **无错误**: 零错误率，便于调试和验证比对算法

## 与其他测试数据对比

| 数据集 | 类型 | 读长 | Reads | 大小 | 用途 |
|--------|------|------|-------|------|------|
| lambda_wgbs | PE | 150 bp | 9,700 pairs | ~1 MB | 双端比对测试 |
| **lambda_se150** | **SE** | **150 bp** | **9,700** | **~483 KB** | **单端比对测试** |
| ex1_small | SE | 32 bp | 10 | ~1 KB | 快速功能验证 |

## 注意事项

1. 数据使用恒定质量值 (Phred 40)，不包含真实的质量波动
2. 错误率设置为 0%，reads 不包含测序错误
3. 仅包含 Lambda 基因组，复杂度较低
4. 适合用于功能测试和 CI/CD，不适合性能基准测试
