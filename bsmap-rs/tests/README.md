# BSMAP-rs 测试数据目录

本目录包含用于测试 bsmap-rs 的所有测试数据、结果和报告。

## 目录结构

```
tests/
├── README.md                    # 本文件
├── data/                        # 测试数据
│   ├── lambda_wgbs/            # Lambda DNA WGBS 双端测序数据
│   │   ├── reads/
│   │   │   ├── R1.fastq.gz     # 双端测序 R1 reads
│   │   │   └── R2.fastq.gz     # 双端测序 R2 reads
│   │   └── reference/
│   │       └── genome.fa       # Lambda DNA 参考序列 (48,502 bp)
│   ├── lambda_wgbs_sim/        # Lambda DNA WGBS 双端测序数据 (BSBolt 模拟)
│   │   ├── reads/
│   │   │   ├── R1.fastq.gz     # 双端测序 R1 reads
│   │   │   └── R2.fastq.gz     # 双端测序 R2 reads
│   │   └── reference/
│   │       └── genome.fa       # Lambda DNA 参考序列 (48,502 bp)
│   ├── lambda_rrbs/            # Lambda DNA RRBS 双端测序数据
│   │   ├── reads/
│   │   │   ├── R1.fastq.gz     # 双端测序 R1 reads
│   │   │   └── R2.fastq.gz     # 双端测序 R2 reads
│   │   └── reference/
│   │       └── genome.fa       # Lambda DNA 参考序列 (48,502 bp)
│   ├── rrbs_random_v2/         # 随机参考基因组 RRBS 数据
│   │   ├── reads/
│   │   │   ├── R1.fastq.gz     # 双端测序 R1 reads
│   │   │   └── R2.fastq.gz     # 双端测序 R2 reads
│   │   └── reference/
│   │       ├── random_genome.fa       # 随机参考序列 (48,500 bp)
│   │       └── statistics.txt         # 参考基因组统计信息
│   ├── ex1_small/              # 小型测试数据 (单端)
│   │   ├── reads/
│   │   │   └── reads.fq        # 10 条 32bp reads
│   │   └── reference/
│   │       └── genome.fa       # 参考序列 (2 条序列, 共 3,158 bp)
│   └── realistic/              # 真实基因组片段测试数据
│       ├── reads/
│       │   └── reads.fq        # 32bp reads
│       └── reference/
│           └── genome.fa       # 参考序列
├── results/                     # 比对结果
│   ├── lambda_wgbs/
│   ├── lambda_wgbs_sim/
│   └── lambda_rrbs/
└── reports/                     # 测试报告
    ├── sam_diff_fix_plan.md         # SAM 输出差异修复计划
    ├── lambda_wgbs_data_summary.md  # Lambda WGBS 数据说明
    └── ex1_comparison_report.md     # ex1_small 测试对比报告
```

## 测试数据集说明

### 1. Lambda WGBS (lambda_wgbs)

**用途**: 双端 WGBS 比对功能测试

**数据特点**:
- 参考序列: Lambda phage NC_001416.1 (48,502 bp)
- 测序类型: PE150 (双端 150bp)
- 覆盖度: 30x
- Read pairs: 9,700
- 生成工具: Sherman v0.1.9
- BS 转换率: 99.5%

**文件位置**:
- reads: `data/lambda_wgbs/reads/`
- reference: `data/lambda_wgbs/reference/`

### 2. Lambda WGBS Sim (lambda_wgbs_sim)

**用途**: Rust vs C++ BSMAP SAM 输出一致性验证（主要测试数据集）

**数据特点**:
- 参考序列: Lambda phage NC_001416.1 (48,502 bp)
- 测序类型: PE150 (双端 150bp)
- Read pairs: 4,850
- 生成工具: BSBolt
- BS 转换率: ~99.5%

**验证结果** (2026-05-14):
- C++ BSMAP: 4,186 配对 + 311 单端 a + 327 单端 b = 9,010 条记录
- Rust bsmap-rs: 4,186 配对 + 311 单端 a + 327 单端 b = 9,010 条记录
- **所有 13 个 SAM 字段 0 差异** ✅

**文件位置**:
- reads: `data/lambda_wgbs_sim/reads/`
- reference: `data/lambda_wgbs_sim/reference/`

### 3. Lambda RRBS (lambda_rrbs)

**用途**: RRBS 模式比对测试

**数据特点**:
- 参考序列: Lambda phage NC_001416.1 (48,502 bp)
- 测序类型: PE150 (双端 150bp)
- 酶切: MspI (CCGG)
- 大小选择: 150-220bp
- Read pairs: 1,844
- 生成方式: 自定义 Python 脚本（MspI 酶切 + BS 转换模拟）
- 测序错误率: 0.5%

**当前状态**:
- C++ BSMAP: RRBS 模式 (`-D C-CGG`) 下 buffer overflow 崩溃 ❌
- Rust bsmap-rs: RRBS 模式 0 配对，WGBS 模式 1,844 单端 ⚠️
- 待修复: C++ buffer overflow、Rust RRBS 配对逻辑

**文件位置**:
- reads: `data/lambda_rrbs/reads/`
- reference: `data/lambda_rrbs/reference/`

### 4. ex1_small (ex1_small)

**用途**: 基础功能测试、快速验证

**数据特点**:
- 参考序列: 2 条序列 (seq1, seq2), 共 3,158 bp
- 测序类型: 单端 32bp
- Reads: 10 条
- 来源: samtools 示例数据

**文件位置**:
- reads: `data/ex1_small/reads/`
- reference: `data/ex1_small/reference/`

### 5. Realistic (realistic)

**用途**: 真实基因组片段测试

**数据特点**:
- 参考序列: 真实基因组片段
- 测序类型: 单端 32bp

**文件位置**:
- reads: `data/realistic/reads/`
- reference: `data/realistic/reference/`

## 使用示例

### 比对 Lambda WGBS Sim 数据（推荐）

```bash
# C++ BSMAP
cd /workspace/bsmap-original/bsmap-2.90
./bsmap \
    -a /workspace/bsmap-rs/tests/data/lambda_wgbs_sim/reads/R1.fastq.gz \
    -b /workspace/bsmap-rs/tests/data/lambda_wgbs_sim/reads/R2.fastq.gz \
    -d /workspace/bsmap-rs/tests/data/lambda_wgbs_sim/reference/genome.fa \
    -o /tmp/cpp.sam -n 0 -p 1 -v 0.08 -m 28 -x 1000

# Rust bsmap-rs
cd /workspace/bsmap-rs
./target/release/bsmap align \
    -a tests/data/lambda_wgbs_sim/reads/R1.fastq.gz \
    -b tests/data/lambda_wgbs_sim/reads/R2.fastq.gz \
    -d tests/data/lambda_wgbs_sim/reference/genome.fa \
    -o /tmp/rust.sam -n 0 -p 1 -v 0.08 -m 28 -x 1000
```

### 比对 Lambda RRBS 数据

```bash
# Rust bsmap-rs (WGBS 模式)
./target/release/bsmap align \
    -a tests/data/lambda_rrbs/reads/R1.fastq.gz \
    -b tests/data/lambda_rrbs/reads/R2.fastq.gz \
    -d tests/data/lambda_rrbs/reference/genome.fa \
    -o /tmp/rrbs.sam -n 0 -p 1 -v 0.08 -m 28 -x 500

# Rust bsmap-rs (RRBS 模式, 待修复)
./target/release/bsmap align \
    -a tests/data/lambda_rrbs/reads/R1.fastq.gz \
    -b tests/data/lambda_rrbs/reads/R2.fastq.gz \
    -d tests/data/lambda_rrbs/reference/genome.fa \
    -o /tmp/rrbs.sam -n 0 -p 1 -v 0.08 -m 28 -x 500 -D C-CGG
```

## 测试结果对比

### Lambda WGBS Sim 测试结果 (2026-05-14)

| 指标 | C++ BSMAP | Rust bsmap-rs | 状态 |
|------|-----------|---------------|------|
| 配对数 | 4,186 | 4,186 | ✅ |
| 单端 a | 311 | 311 | ✅ |
| 单端 b | 327 | 327 | ✅ |
| 总记录数 | 9,010 | 9,010 | ✅ |
| 字段差异 | - | 0 | ✅ |

### Lambda RRBS 测试结果 (2026-05-14)

| 指标 | C++ BSMAP | Rust bsmap-rs | 状态 |
|------|-----------|---------------|------|
| RRBS 模式 | buffer overflow | 0 配对 | ❌ |
| WGBS 模式 | buffer overflow | 1,844 单端 | ⚠️ |

### RRBS Random Genome v2 测试结果 (2026-05-14)

| 指标 | C++ BSMAP | Rust bsmap-rs | 状态 |
|------|-----------|---------------|------|
| 配对数 | 0 | 5,578 | ❌ |
| 单端 a | 5,578 | 19 | ⚠️ |
| 单端 b | 5,578 | 19 | ⚠️ |
| 总记录数 | 5,616 | 11,194 | ⚠️ |
| 字段差异 | - | 0 (配对记录) | ✅ |

**关键发现**:
- Rust bsmap-rs 配对逻辑正常工作，成功配对 5,578 对 reads
- C++ BSMAP 配对逻辑失效，所有 reads 作为单端处理
- 比对位置一致，SAM 字段无差异

**文件位置**:
- reads: `data/rrbs_random_v2/reads/`
- reference: `data/rrbs_random_v2/reference/`
- 报告: `reports/rrbs_random_v2_comparison_report.md`

## 报告文件

- `reports/sam_diff_fix_plan.md`: SAM 输出差异修复计划（含完整修复历史）
- `reports/lambda_wgbs_data_summary.md`: Lambda WGBS 数据说明
- `reports/ex1_comparison_report.md`: ex1_small 测试对比报告
- `reports/rrbs_random_v2_comparison_report.md`: RRBS Random Genome v2 测试对比报告

## 注意事项

1. **索引文件**: 参考序列的索引文件 (`.bsi`) 会自动生成在 reference 目录下
2. **结果覆盖**: 重新运行比对会覆盖现有的 `.sam` 文件
3. **数据备份**: 原始测试数据已备份，可随时恢复

## 更新历史

- 2026-05-14: 新增 rrbs_random_v2 随机参考基因组 RRBS 测试数据集；Rust vs C++ 配对逻辑对比
- 2026-05-14: 新增 lambda_rrbs RRBS 测试数据集；更新 lambda_wgbs_sim 验证结果为 0 差异
- 2026-05-13: 重新组织测试数据目录结构，使其更清晰
