# Lambda DNA WGBS 模拟数据生成报告

## 生成时间
2026-05-13

## 工具信息
- **工具**: Sherman v0.1.9
- **作者**: Felix Krueger (Babraham Bioinformatics)
- **GitHub**: https://github.com/FelixKrueger/Sherman
- **安装位置**: `/workspace/bsmap-rs/tools/sherman/`

## 参考基因组
- **物种**: Enterobacteria phage lambda
- **NCBI ID**: NC_001416.1
- **基因组大小**: 48,502 bp
- **文件位置**: `/workspace/bsmap-rs/reference/lambda/lambda_genome.fa`

## 生成参数

| 参数 | 值 |
|------|-----|
| 读长 | 150 bp (PE150) |
| 覆盖度 | ~30x |
| Read pairs | 9,700 |
| 错误率 | 0% |
| Phred 质量值 | 40 (恒定) |
| BS 转换率 | 99.5% |
| 片段长度 | 70-400 bp |
| 文库类型 | Directional (定向) |

## 输出文件

| 文件 | 大小 | 描述 |
|------|------|------|
| `lambda_R1.fastq.gz` | 495 KB | 双端测序 R1 reads |
| `lambda_R2.fastq.gz` | 495 KB | 双端测序 R2 reads |

**总大小**: ~1 MB (gzip 压缩)

## 数据验证

### Read 数量
- R1: 9,700 reads
- R2: 9,700 reads
- 总计: 9,700 read pairs

### BS 转换验证
原始 Lambda 序列 (前 60bp):
```
GGGCGGCGACCTCGCGGGTTTTCGCTATTTATGAAAATTTTCCGGTTTAAGGCGTTTCCG
```

模拟 reads 中的序列 (示例):
```
TAGTTGGTTAGTTTTTTTTGTTGTTTTTGATTGTTTGTGTTTAGAATAAAATTTATTGTT
```

观察: C 碱基大部分已转换为 T (符合 99.5% 转换率)

### Read ID 格式
```
@1_NC_001416.1:3437-3764_R1
```
格式: `{序号}_{染色体}:{起始位置}-{结束位置}_{R1/R2}`

## 使用示例

### 使用 bsmap-rs 比对
```bash
cd /workspace/bsmap-rs
./target/release/bsmap \
    -1 test_data/lambda_R1.fastq.gz \
    -2 test_data/lambda_R2.fastq.gz \
    -d reference/lambda/lambda_genome.fa \
    -o output.sam
```

### 使用 C++ BSMAP 比对
```bash
cd /workspace/bsmap-original/bsmap-2.90
./bsmap \
    -a /workspace/bsmap-rs/test_data/lambda_R1.fastq.gz \
    -b /workspace/bsmap-rs/test_data/lambda_R2.fastq.gz \
    -d /workspace/bsmap-rs/reference/lambda/lambda_genome.fa \
    -o output.sam
```

## 适用场景

1. **CI/CD 测试**: 文件小 (~1MB)，适合云端自动化测试
2. **算法验证**: 已知参考序列，可验证比对准确性
3. **性能测试**: 30x 覆盖度，可测试工具性能
4. **甲基化分析验证**: Sherman 模拟了 BS 转换，可用于验证甲基化 calling 流程

## 注意事项

1. 数据使用恒定质量值 (Phred 40)，不包含真实的质量波动
2. 错误率设置为 0%，reads 不包含测序错误
3. 仅包含 Lambda 基因组，复杂度较低
4. 适合用于功能测试，不适合用于性能基准测试真实数据

## 目录结构

```
/workspace/bsmap-rs/
├── tools/
│   └── sherman/              # Sherman 模拟工具
├── reference/
│   └── lambda/
│       └── lambda_genome.fa  # Lambda DNA 参考序列
└── test_data/
    ├── lambda_R1.fastq.gz    # 生成的 WGBS 数据 (R1)
    ├── lambda_R2.fastq.gz    # 生成的 WGBS 数据 (R2)
    └── lambda_wgbs_data_summary.txt  # 本文件
```
