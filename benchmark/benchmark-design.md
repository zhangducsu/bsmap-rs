# bsmap-rs vs BSMAP C++ 性能与一致性对比测试设计文档

**日期**: 2026-05-15
**目标**: 为 bsmap-rs 生成与原版 BSMAP C++ 的 6 个对比测试用例，完成一致性、内存占用、运行时间的全面对比测试

---

## 1. 测试环境

| 项目 | 配置 |
|------|------|
| CPU | 3 核心 |
| 内存 | 5.8GB (可用 ~4.8GB) |
| 存储 | 1.5TB |
| Rust 版本 | 1.95.0 |
| 原版 BSMAP | C++ 2.90 |

**数据规模限制**:
- 覆盖度: Example 1-4 为 10x，Example 5-6 为 20x
- 读长: SE=75bp, PE=150bp
- 参考基因组: hg38 chr22 尾部 1M bp

---

## 2. 参考基因组

**选择**: hg38 chr22 尾部 1,000,000 bp

**理由**:
- 使用真实人类基因组序列，更贴近实际应用场景
- chr22 尾部序列包含自然分布的 MspI 酶切位点 (CCGG)，适合 RRBS 模式测试
- 1M bp 规模小，加速索引构建和比对测试，适合快速迭代验证

**获取方式**:
```bash
# 下载 chr22 完整序列
curl -sL "https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chr22.fa.gz" | gunzip > chr22.fa

# 截取尾部 1M bp (Python 脚本)
python3 -c "
from Bio import SeqIO
rec = SeqIO.read('chr22.fa', 'fasta')
tail = rec.seq[-1000000:]
with open('chr22_tail_1M.fa', 'w') as f:
    f.write(f'>{rec.id}|tail_1M\n')
    for i in range(0, len(tail), 80):
        f.write(str(tail[i:i+80]) + '\n')
"

# 验证
grep -v '^>' chr22_tail_1M.fa | tr -d '\n' | wc -c  # 应输出: 1000000
```

---

## 3. 6 个测试用例

| Example | 模式 | 工具 | 读段类型 | 读段数 | 覆盖度 | 目标 |
|---------|------|------|---------|--------|--------|------|
| 1 | WGBS SE | Sherman | 75bp | 133,334 | 10x | 基础功能一致性验证 |
| 2 | WGBS PE | Sherman | 150bp | 66,667 pairs | 10x | 配对逻辑一致性验证 |
| 3 | RRBS SE | RRBSsim | 75bp | ~133K (10x) | 10x | RRBS 模式一致性验证 |
| 4 | RRBS PE | RRBSsim | 150bp | ~67K pairs (10x) | 10x | RRBS 配对逻辑一致性验证 |
| 5 | WGBS PE | Sherman | 150bp | 133,334 pairs | 20x | 大规模性能对比 (时间+内存) |
| 6 | RRBS PE | RRBSsim | 150bp | ~133K pairs (20x) | 20x | RRBS 大规模性能对比 |

**读段数计算公式**: `n = genome_size x depth / read_length`
- 1M x 10 / 75 = 133,334 (Example 1)
- 1M x 10 / 150 = 66,667 (Example 2)
- 1M x 20 / 150 = 133,334 (Example 5)

> 注意: RRBS 的实际读段数由 RRBSsim 根据 `-d` 参数自动计算，可能因酶切位点分布而不同。

---

## 4. 测序数据生成

### 4.1 生成工具

- **WGBS 数据**: Sherman (`/workspace/bsmap-rs/tools/sherman/Sherman`)
- **RRBS 数据**: RRBSsim (`/workspace/bsmap-rs/tools/rrbssim/RRBSsim`)

### 4.2 Sherman (WGBS 模拟器)

- 不需要预建索引，直接读 FASTA 文件
- `--genome_folder <dir>`: 包含 FASTA 文件的目录
- `-l <读长>`: 读段长度
- `-n <读段数>`: 读段数量
- `-pe`: 双端模式 (不加则为单端)
- `-cr 99.0`: 转化率 99%
- `-o <输出目录>`: 输出目录
- 输出文件名固定: 单端 `simulated.fastq`，双端 `simulated_1.fastq` + `simulated_2.fastq`

### 4.3 RRBSsim (RRBS 模拟器)

- 依赖: `pip install pyfaidx`
- `-f <fasta>`: 直接指定 FASTA 文件
- `-d <深度>`: 覆盖度
- `-l <读长>`: 读段长度
- `-s`: 单端模式
- `-p`: 双端模式
- `-o <输出前缀>`: 输出文件前缀
- 输出文件名: 单端 `<prefix>.1.fq`，双端 `<prefix>.1.fq` + `<prefix>.2.fq`

### 4.4 数据量估算

| 数据集 | 覆盖度 | 读段数 | 单/双端 | 文件大小估算 |
|--------|--------|--------|---------|-------------|
| WGBS SE 75bp | 10x | 133,334 | 单端 | ~20MB |
| WGBS PE 150bp | 10x | 66,667 pairs | 双端 | ~20MB |
| RRBS SE 75bp | 10x | ~133K | 单端 | ~15MB (酶切后) |
| RRBS PE 150bp | 10x | ~67K pairs | 双端 | ~15MB |
| WGBS PE 150bp | 20x | 133,334 pairs | 双端 | ~40MB |
| RRBS PE 150bp | 20x | ~133K pairs | 双端 | ~30MB |

**总数据量**: ~140MB (可接受)

---

## 5. 比对参数

### 5.1 索引构建参数

```bash
# WGBS 模式
bsmap -a ref.fa -d wgbs -o index_wgbs.bsi -v 16 -i 4

# RRBS 模式
bsmap -a ref.fa -d rrbs -o index_rrbs.bsi -v 16 -i 4 -e MspI
```

### 5.2 比对参数

**单端 (Example 1, 3)**:
```bash
bsmap -a reads_1.fq -d ref.fa -o output.sam -v 16 -i 4 -g wgbs
```

**双端 (Example 2, 4, 5, 6)**:
```bash
bsmap -a reads_1.fq -b reads_2.fq -d ref.fa -o output.sam -v 16 -i 4 -g wgbs
```

### 5.3 原版 BSMAP 参数

原版 BSMAP 使用完全相同的参数，确保公平对比。

---

## 6. 对比指标

### 6.1 一致性对比

**SAM 过滤规则**:
```bash
# 过滤掉 @PG 行后逐行 diff
grep -v '^@PG' sam1 > sam1.filtered
grep -v '^@PG' sam2 > sam2.filtered
diff sam1.filtered sam2.filtered
```

**差异分类**:
- 完全一致 (diff=0)
- FLAG 差异 (可接受，配对逻辑差异)
- MAPQ 差异 (可接受，打分实现差异)
- CIGAR 差异 (需分析)
- 位置差异 (需分析)

### 6.2 性能对比

| 指标 | 测量方法 |
|------|---------|
| 运行时间 | `time -p` (wall clock) |
| 用户时间 | `time -p` (user time) |
| 系统时间 | `time -p` (sys time) |
| 最大 RSS | `/usr/bin/time -v` |
| 索引大小 | `ls -lh *.bsi` |

### 6.3 记录格式

测试结果记录到 CSV 文件:
```csv
example,tool,dataset,mode,reads,time_wall,time_user,mem_max_rss,index_size,alignment_rate,unique_pairs,multi_pairs
```

---

## 7. 测试流程

### 阶段 1: 环境准备
1. 确认工具链 (Rust、原版 BSMAP、Sherman、RRBSsim)
2. 安装 Python 依赖 (`pip install pyfaidx`)
3. 编译 bsmap-rs release 版本
4. 下载 hg38 chr22 并截取尾部 1M bp 参考基因组
5. 构建工作目录

### 阶段 2: 数据生成
1. 使用 Sherman 生成 3 套 WGBS 测试数据 (Example 1, 2, 5)
2. 使用 RRBSsim 生成 3 套 RRBS 测试数据 (Example 3, 4, 6)
3. 验证数据完整性

### 阶段 3: 索引构建
1. 原版 WGBS 索引构建
2. 原版 RRBS 索引构建
3. bsmap-rs WGBS 索引构建
4. bsmap-rs RRBS 索引构建
5. 记录时间/内存/大小

### 阶段 4: 比对测试 (6 Examples)
对每个 Example 依次:
1. 原版 BSMAP 比对
2. bsmap-rs 比对
3. SAM 输出对比
4. 记录时间/内存

### 阶段 5: 报告生成
1. 汇总所有数据
2. 生成对比报告 (Markdown)
3. 可视化图表

---

## 8. 预期结果

### 8.1 一致性预期

| Example | 预期 diff | 说明 |
|---------|-----------|------|
| WGBS SE 133K | 极小 | 单端，可能存在随机因素 |
| WGBS PE 67K pairs | 极小 | 可能 FLAG 顺序差异 |
| RRBS SE ~133K | 极小 | RRBS 模式，酶切位点确定性 |
| RRBS PE ~67K pairs | 极小 | 可能 FLAG 顺序差异 |
| WGBS PE 133K pairs (20x) | 极小 | 同上 |
| RRBS PE ~133K pairs (20x) | 极小 | 同上 |

### 8.2 性能预期

| 指标 | 原版 BSMAP | bsmap-rs | 说明 |
|------|-----------|-----------|------|
| 比对时间 | 基线 | 预计 0.8-1.2x | SIMD 优化效果 |
| 比对内存 | 基线 | 预计 0.5-0.8x | mmap 优化效果 |
| 索引大小 | 基线 | 预计 1.0-1.1x | 包含 refcat 数据 |
| 索引构建时间 | 基线 | 预计 1.0-1.2x | 额外写 refcat |

---

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 原版 BSMAP 未编译 | 无法对比 | 使用 `/workspace/bsmap-original/bsmap-2.90/bsmap` |
| Sherman 运行失败 | 无法生成 WGBS 数据 | 检查可执行权限，确认 `--genome_folder` 路径正确 |
| RRBSsim 依赖缺失 | 无法生成 RRBS 数据 | `pip install pyfaidx` |
| 内存接近上限 | 可能 OOM | 监控 RSS，必要时降低规模 |
| SAM diff 非零 | 需要分析 | 分类差异原因 |

---

## 10. 输出文件

```
/workspace/bsmap-rs/benchmark/
├── benchmark-design.md           # 本文档
├── benchmark-impl-plan.md        # 实施计划
├── data/
│   ├── chr22.fa                  # hg38 chr22 完整序列 (下载后可删除)
│   ├── chr22_tail_1M.fa          # chr22 尾部 1M bp 参考基因组
│   ├── ref/                      # Sherman 所需的参考基因组目录
│   │   └── chr22_tail_1M.fa      # chr22_tail_1M.fa 的副本
│   ├── wgbs/                     # WGBS 测试数据 (Sherman 生成)
│   │   ├── ex1_se75_10x/
│   │   │   └── simulated.fastq.gz
│   │   ├── ex2_pe150_10x/
│   │   │   ├── simulated_1.fastq.gz
│   │   │   └── simulated_2.fastq.gz
│   │   └── ex5_pe150_20x/
│   │       ├── simulated_1.fastq.gz
│   │       └── simulated_2.fastq.gz
│   └── rrbs/                     # RRBS 测试数据 (RRBSsim 生成)
│       ├── ex3_se75_10x.1.fq.gz
│       ├── ex4_pe150_10x.1.fq.gz
│       ├── ex4_pe150_10x.2.fq.gz
│       ├── ex6_pe150_20x.1.fq.gz
│       └── ex6_pe150_20x.2.fq.gz
├── index/                        # 索引文件
│   ├── bsmap_wgbs.bsi
│   ├── bsmap_rrbs.bsi
│   ├── bsmaprs_wgbs.bsi
│   └── bsmaprs_rrbs.bsi
├── results/                      # 测试结果
│   ├── example1_wgbs_se/
│   │   ├── bsmap.log
│   │   ├── bsmap_rust.log
│   │   └── diff.txt
│   ├── ... (6 examples)
│   └── summary.csv
└── report/
    └── benchmark_report.md        # 最终报告
```
