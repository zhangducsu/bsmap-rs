# bsmap-rs vs BSMAP C++ 对比测试实施计划

> **For agentic workers:** Execute step-by-step. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 完成 6 个 Example 的对比测试，生成包含一致性、内存、时间对比的报告。

**Spec 文档**: `benchmark/benchmark-design.md`

---

## 阶段 1: 环境准备

### Task 1.1: 确认工具链和依赖

- [ ] **Step 1: 确认 Rust 编译环境**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
rustc --version
cargo --version
```

- [ ] **Step 2: 确认原版 BSMAP 可用**
```bash
ls /workspace/bsmap-original/bsmap-2.90/bsmap
```

- [ ] **Step 3: 确认 Sherman 可用**
```bash
ls /workspace/bsmap-rs/tools/sherman/Sherman
/workspace/bsmap-rs/tools/sherman/Sherman --help 2>&1 | head -10
```

- [ ] **Step 4: 确认 RRBSsim 可用**
```bash
ls /workspace/bsmap-rs/tools/rrbssim/RRBSsim
python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim --help 2>&1 | head -10
```

- [ ] **Step 5: 安装 Python 依赖**
```bash
pip install pyfaidx
```

- [ ] **Step 6: 编译 bsmap-rs release 版本**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
cd /workspace/bsmap-rs && cargo build --release -p bsmap 2>&1 | tail -3
```

---

### Task 1.2: 下载 chr22 并截取尾部 1M 参考基因组

- [ ] **Step 1: 创建工作目录**
```bash
mkdir -p /workspace/bsmap-rs/benchmark/data
mkdir -p /workspace/bsmap-rs/benchmark/data/ref
mkdir -p /workspace/bsmap-rs/benchmark/data/wgbs
mkdir -p /workspace/bsmap-rs/benchmark/data/rrbs
mkdir -p /workspace/bsmap-rs/benchmark/index
mkdir -p /workspace/bsmap-rs/benchmark/results
mkdir -p /workspace/bsmap-rs/benchmark/report
```

- [ ] **Step 2: 下载 hg38 chr22 完整序列**
```bash
cd /workspace/bsmap-rs/benchmark/data
curl -sL "https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chr22.fa.gz" | gunzip > chr22.fa

# 验证
head -3 chr22.fa
wc -c chr22.fa
```

- [ ] **Step 3: 截取 chr22 尾部 1M bp**
```bash
cd /workspace/bsmap-rs/benchmark/data
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

- [ ] **Step 4: 为 Sherman 准备 genome_folder (Sherman 需要目录而非文件)**
```bash
cp /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa /workspace/bsmap-rs/benchmark/data/ref/chr22_tail_1M.fa
```

---

## 阶段 2: 数据生成

### Task 2.1: 生成 WGBS 测试数据 (Sherman)

- [ ] **Step 1: 生成 WGBS 单端 75bp 10x (Example 1)**
```bash
cd /workspace/bsmap-rs/benchmark/data/wgbs
mkdir -p ex1_se75_10x

/workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 75 \
    -n 133334 \
    -cr 99.0 \
    -o ex1_se75_10x

# 验证
wc -l ex1_se75_10x/simulated.fastq
# 预期: 533336 行 (133334 reads x 4 行/read)
```

- [ ] **Step 2: 生成 WGBS 双端 150bp 10x (Example 2)**
```bash
mkdir -p ex2_pe150_10x

/workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 150 \
    -n 66667 \
    -pe \
    -cr 99.0 \
    -o ex2_pe150_10x

# 验证
wc -l ex2_pe150_10x/simulated_1.fastq ex2_pe150_10x/simulated_2.fastq
# 预期: 各 266668 行 (66667 reads x 4 行/read)
```

- [ ] **Step 3: 生成 WGBS 双端 150bp 20x (Example 5)**
```bash
mkdir -p ex5_pe150_20x

/workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 150 \
    -n 133334 \
    -pe \
    -cr 99.0 \
    -o ex5_pe150_20x

# 验证
wc -l ex5_pe150_20x/simulated_1.fastq ex5_pe150_20x/simulated_2.fastq
# 预期: 各 533336 行 (133334 reads x 4 行/read)
```

- [ ] **Step 4: 压缩所有 WGBS 数据为 fq.gz**
```bash
cd /workspace/bsmap-rs/benchmark/data/wgbs

gzip ex1_se75_10x/simulated.fastq
gzip ex2_pe150_10x/simulated_1.fastq
gzip ex2_pe150_10x/simulated_2.fastq
gzip ex5_pe150_20x/simulated_1.fastq
gzip ex5_pe150_20x/simulated_2.fastq

# 验证
ls -lh ex1_se75_10x/ ex2_pe150_10x/ ex5_pe150_20x/
```

---

### Task 2.2: 生成 RRBS 测试数据 (RRBSsim)

- [ ] **Step 1: 生成 RRBS 单端 75bp 10x (Example 3)**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs

python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 10 \
    -l 75 \
    -s \
    -o ex3_se75_10x

# 验证
wc -l ex3_se75_10x.1.fq
```

- [ ] **Step 2: 生成 RRBS 双端 150bp 10x (Example 4)**
```bash
python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 10 \
    -l 150 \
    -p \
    -o ex4_pe150_10x

# 验证
wc -l ex4_pe150_10x.1.fq ex4_pe150_10x.2.fq
```

- [ ] **Step 3: 生成 RRBS 双端 150bp 20x (Example 6)**
```bash
python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 20 \
    -l 150 \
    -p \
    -o ex6_pe150_20x

# 验证
wc -l ex6_pe150_20x.1.fq ex6_pe150_20x.2.fq
```

- [ ] **Step 4: 压缩所有 RRBS 数据为 fq.gz**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs

gzip ex3_se75_10x.1.fq
gzip ex4_pe150_10x.1.fq
gzip ex4_pe150_10x.2.fq
gzip ex6_pe150_20x.1.fq
gzip ex6_pe150_20x.2.fq

# 验证
ls -lh *.fq.gz
```

---

## 阶段 3: 索引构建测试

### Task 3.1: 构建索引

- [ ] **Step 1: 原版 WGBS 索引**
```bash
cd /workspace/bsmap-rs/benchmark

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -d wgbs -o index/bsmap_wgbs.bsi -v 16 -i 4 2>&1 | tee index/bsmap_wgbs_build.log
```

- [ ] **Step 2: 原版 RRBS 索引**
```bash
/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -d rrbs -o index/bsmap_rrbs.bsi -v 16 -i 4 -e MspI 2>&1 | tee index/bsmap_rrbs_build.log
```

- [ ] **Step 3: bsmap-rs WGBS 索引**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
/usr/bin/time -v cargo run --release -p bsmap -- index \
    -d data/chr22_tail_1M.fa -g wgbs -o index/bsmaprs_wgbs.bsi -v 16 -i 4 2>&1 | tee index/bsmaprs_wgbs_build.log
```

- [ ] **Step 4: bsmap-rs RRBS 索引**
```bash
/usr/bin/time -v cargo run --release -p bsmap -- index \
    -d data/chr22_tail_1M.fa -g rrbs -o index/bsmaprs_rrbs.bsi -v 16 -i 4 -e MspI 2>&1 | tee index/bsmaprs_rrbs_build.log
```

- [ ] **Step 5: 记录索引大小**
```bash
ls -lh index/*.bsi
```

---

## 阶段 4: 比对测试 (6 Examples)

### Task 4.1: Example 1 -- WGBS SE 75bp 10x (133,334 reads)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example1_wgbs_se

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmap.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example1_wgbs_se/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example1_wgbs_se

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmaprs.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example1_wgbs_se/bsmaprs.log
```

---

### Task 4.2: Example 2 -- WGBS PE 150bp 10x (66,667 pairs)

- [ ] **原版 BSMAP**
```bash
mkdir -p results/example2_wgbs_pe

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz) \
    -b <(gunzip -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmap.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example2_wgbs_pe/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example2_wgbs_pe

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz) \
    -b <(gunzip -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmaprs.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example2_wgbs_pe/bsmaprs.log
```

---

### Task 4.3: Example 3 -- RRBS SE 75bp 10x (~133K reads)

- [ ] **原版 BSMAP**
```bash
mkdir -p results/example3_rrbs_se

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/rrbs/ex3_se75_10x.1.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmap.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example3_rrbs_se/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example3_rrbs_se

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/rrbs/ex3_se75_10x.1.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmaprs.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example3_rrbs_se/bsmaprs.log
```

---

### Task 4.4: Example 4 -- RRBS PE 150bp 10x (~67K pairs)

- [ ] **原版 BSMAP**
```bash
mkdir -p results/example4_rrbs_pe

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/rrbs/ex4_pe150_10x.1.fq.gz) \
    -b <(gunzip -c data/rrbs/ex4_pe150_10x.2.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmap.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example4_rrbs_pe/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example4_rrbs_pe

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/rrbs/ex4_pe150_10x.1.fq.gz) \
    -b <(gunzip -c data/rrbs/ex4_pe150_10x.2.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmaprs.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example4_rrbs_pe/bsmaprs.log
```

---

### Task 4.5: Example 5 -- WGBS PE 150bp 20x (133,334 pairs)

- [ ] **原版 BSMAP**
```bash
mkdir -p results/example5_wgbs_pe_20x

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz) \
    -b <(gunzip -c data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmap.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example5_wgbs_pe_20x/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example5_wgbs_pe_20x

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz) \
    -b <(gunzip -c data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmaprs.sam \
    -v 16 -i 4 -g wgbs 2>&1 | tee results/example5_wgbs_pe_20x/bsmaprs.log
```

---

### Task 4.6: Example 6 -- RRBS PE 150bp 20x (~133K pairs)

- [ ] **原版 BSMAP**
```bash
mkdir -p results/example6_rrbs_pe_20x

/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a <(gunzip -c data/rrbs/ex6_pe150_20x.1.fq.gz) \
    -b <(gunzip -c data/rrbs/ex6_pe150_20x.2.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmap.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example6_rrbs_pe_20x/bsmap.log
```

- [ ] **bsmap-rs**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
mkdir -p results/example6_rrbs_pe_20x

/usr/bin/time -v cargo run --release -p bsmap -- \
    -a <(gunzip -c data/rrbs/ex6_pe150_20x.1.fq.gz) \
    -b <(gunzip -c data/rrbs/ex6_pe150_20x.2.fq.gz) \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmaprs.sam \
    -v 16 -i 4 -g rrbs 2>&1 | tee results/example6_rrbs_pe_20x/bsmaprs.log
```

---

## 阶段 5: SAM 一致性对比

### Task 5.1: SAM 对比脚本

```bash
#!/bin/bash
# compare_sam.sh - SAM 一致性对比

SAM1=$1
SAM2=$2
OUT=$3

mkdir -p $OUT

# 过滤 @HD, @SQ, @PG 行（保留比对记录 + @CO 注释）
grep -v "^@HD\|^@SQ\|^@PG" $SAM1 > $OUT/sam1_filtered.sam
grep -v "^@HD\|^@SQ\|^@PG" $SAM2 > $OUT/sam2_filtered.sam

# 统计行数
echo "=== SAM 记录数 ===" > $OUT/diff_report.txt
wc -l $OUT/sam1_filtered.sam >> $OUT/diff_report.txt
wc -l $OUT/sam2_filtered.sam >> $OUT/diff_report.txt

# 逐行 diff
echo "" >> $OUT/diff_report.txt
echo "=== 行差异 ===" >> $OUT/diff_report.txt
diff $OUT/sam1_filtered.sam $OUT/sam2_filtered.sam | head -100 >> $OUT/diff_report.txt

# 统计差异类型
echo "" >> $OUT/diff_report.txt
echo "=== 差异分类 ===" >> $OUT/diff_report.txt
DIFF_COUNT=$(diff $OUT/sam1_filtered.sam $OUT/sam2_filtered.sam | grep "^>" | wc -l)
echo "总差异行数: $DIFF_COUNT" >> $OUT/diff_report.txt

# 分析 FLAG 差异
echo "" >> $OUT/diff_report.txt
echo "=== FLAG 差异分析 ===" >> $OUT/diff_report.txt
diff $OUT/sam1_filtered.sam $OUT/sam2_filtered.sam | grep "^[<>]" | grep -E "FLAG|YS:Z:" | head -20 >> $OUT/diff_report.txt

cat $OUT/diff_report.txt
```

### Task 5.2: 对每个 Example 执行 SAM 对比

- [ ] **Example 1: WGBS SE**
```bash
cd /workspace/bsmap-rs/benchmark
bash compare_sam.sh \
    results/example1_wgbs_se/bsmap.sam \
    results/example1_wgbs_se/bsmaprs.sam \
    results/example1_diff
```

- [ ] **Example 2: WGBS PE**
```bash
bash compare_sam.sh \
    results/example2_wgbs_pe/bsmap.sam \
    results/example2_wgbs_pe/bsmaprs.sam \
    results/example2_diff
```

- [ ] **Example 3: RRBS SE**
```bash
bash compare_sam.sh \
    results/example3_rrbs_se/bsmap.sam \
    results/example3_rrbs_se/bsmaprs.sam \
    results/example3_diff
```

- [ ] **Example 4: RRBS PE**
```bash
bash compare_sam.sh \
    results/example4_rrbs_pe/bsmap.sam \
    results/example4_rrbs_pe/bsmaprs.sam \
    results/example4_diff
```

- [ ] **Example 5: WGBS PE 20x**
```bash
bash compare_sam.sh \
    results/example5_wgbs_pe_20x/bsmap.sam \
    results/example5_wgbs_pe_20x/bsmaprs.sam \
    results/example5_diff
```

- [ ] **Example 6: RRBS PE 20x**
```bash
bash compare_sam.sh \
    results/example6_rrbs_pe_20x/bsmap.sam \
    results/example6_rrbs_pe_20x/bsmaprs.sam \
    results/example6_diff
```

---

## 阶段 6: 报告生成

### Task 6.1: 生成 summary.csv

- [ ] **Step 1: 提取所有测试结果到 summary.csv**
```bash
cd /workspace/bsmap-rs/benchmark

echo "example,tool,mode,reads,time_wall,time_user,time_sys,mem_max_rss_kb,aligned_count" > results/summary.csv

for i in 1 2 3 4 5 6; do
    for tool in bsmap bsmaprs; do
        # 根据编号确定结果目录名
        case $i in
            1) DIR="results/example1_wgbs_se"; MODE="wgbs";;
            2) DIR="results/example2_wgbs_pe"; MODE="wgbs";;
            3) DIR="results/example3_rrbs_se"; MODE="rrbs";;
            4) DIR="results/example4_rrbs_pe"; MODE="rrbs";;
            5) DIR="results/example5_wgbs_pe_20x"; MODE="wgbs";;
            6) DIR="results/example6_rrbs_pe_20x"; MODE="rrbs";;
        esac

        LOG="${DIR}/${tool}.log"
        if [ -f "$LOG" ]; then
            WALL=$(grep "wall clock" $LOG | awk '{print $NF}')
            USER=$(grep "user" $LOG | head -1 | awk '{print $NF}')
            SYS=$(grep "sys" $LOG | head -1 | awk '{print $NF}')
            RSS=$(grep "Maximum resident" $LOG | awk '{print $NF}')
            echo "example${i},${tool},${MODE},,$WALL,$USER,$SYS,$RSS," >> results/summary.csv
        fi
    done
done

cat results/summary.csv
```

### Task 6.2: 生成 Markdown 报告

- [ ] **Step 1: 生成 benchmark_report.md**
```bash
cd /workspace/bsmap-rs/benchmark

cat > report/benchmark_report.md << 'REPORT_EOF'
# bsmap-rs vs BSMAP C++ 对比测试报告

**日期**: $(date +%Y-%m-%d)
**环境**: 3核 5.8GB 内存
**参考基因组**: hg38 chr22 尾部 1M bp

---

## 1. 测试概览

| Example | 模式 | 数据类型 | 读段数 | 覆盖度 | 状态 |
|---------|------|---------|--------|--------|------|
| Example 1 | WGBS | 单端 75bp | 133,334 | 10x | TODO |
| Example 2 | WGBS | 双端 150bp | 66,667 pairs | 10x | TODO |
| Example 3 | RRBS | 单端 75bp | ~133K | 10x | TODO |
| Example 4 | RRBS | 双端 150bp | ~67K pairs | 10x | TODO |
| Example 5 | WGBS | 双端 150bp | 133,334 pairs | 20x | TODO |
| Example 6 | RRBS | 双端 150bp | ~133K pairs | 20x | TODO |

---

## 2. 索引构建对比

| 工具 | 模式 | 构建时间 | 最大 RSS | 索引大小 |
|------|------|---------|---------|---------|
| BSMAP C++ | WGBS | TBD | TBD | TBD |
| bsmap-rs | WGBS | TBD | TBD | TBD |
| BSMAP C++ | RRBS | TBD | TBD | TBD |
| bsmap-rs | RRBS | TBD | TBD | TBD |

---

## 3. 比对性能对比

[TODO: 填入实际测试数据]

### 3.1 Example 1: WGBS 单端 133K (10x)

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.2-3.6 [同上模板]

---

## 4. 结论

[TODO: 根据实际测试结果填写]

REPORT_EOF
```

---

## 验证标准

- [ ] chr22_tail_1M.fa 参考基因组生成成功 (1,000,000 bp)
- [ ] Sherman 生成 3 套 WGBS 数据成功
- [ ] RRBSsim 生成 3 套 RRBS 数据成功
- [ ] 原版和 Rust 版各运行 6 次比对
- [ ] SAM 对比结果记录到 diff 目录
- [ ] summary.csv 和 benchmark_report.md 生成完成

---

## Self-Review 检查清单

| 检查项 | 状态 |
|--------|------|
| 6 个 Example 全部执行 | |
| 内存使用在 5.8GB 内 | |
| SAM diff 分析完成 | |
| 报告生成 | |
| 结果可复现 | |
