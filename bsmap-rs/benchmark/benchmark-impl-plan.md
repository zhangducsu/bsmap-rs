# bsmap-rs vs BSMAP C++ 对比测试实施计划

> **For agentic workers:** Execute step-by-step. Steps use checkbox (`- [ ]`) syntax.
>
> **日期**: 2026-05-17 (更新)
> **重要变更**:
> - 修正所有参数 (-s, -I, -D)
> - 添加预编译步骤避免重复编译
> - 使用临时文件替代进程替换 `<(...)>`
> - 添加 tmp/ 目录用于解压数据

**目标**: 完成 6 个 Example 的对比测试，生成包含一致性、内存、时间对比的报告。

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

- [ ] **Step 6: 预编译 bsmap-rs release 版本 (重要)**
```bash
export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
cd /workspace/bsmap-rs && cargo build --release -p bsmap 2>&1 | tail -3
ls -lh /workspace/bsmap-rs/target/release/bsmap
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
mkdir -p /workspace/bsmap-rs/benchmark/tmp
mkdir -p /workspace/bsmap-rs/benchmark/results
mkdir -p /workspace/bsmap-rs/benchmark/report
```

- [ ] **Step 2: 下载 hg38 chr22 完整序列 (如果不存在)**
```bash
cd /workspace/bsmap-rs/benchmark/data
if [ ! -f chr22.fa ]; then
  curl -sL "https://hgdownload.soe.ucsc.edu/goldenPath/hg38/chromosomes/chr22.fa.gz" | gunzip > chr22.fa
fi

# 验证
head -3 chr22.fa
wc -c chr22.fa
```

- [ ] **Step 3: 截取 chr22 尾部 1M bp (如果不存在)**
```bash
cd /workspace/bsmap-rs/benchmark/data
if [ ! -f chr22_tail_1M.fa ]; then
  python3 -c "
from Bio import SeqIO
rec = SeqIO.read('chr22.fa', 'fasta')
tail = rec.seq[-1000000:]
with open('chr22_tail_1M.fa', 'w') as f:
    f.write(f'>{rec.id}|tail_1M\n')
    for i in range(0, len(tail), 80):
        f.write(str(tail[i:i+80]) + '\n')
"
fi

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
if [ ! -f ex1_se75_10x/simulated.fastq.gz ]; then
  mkdir -p ex1_se75_10x
  /workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 75 \
    -n 133334 \
    -cr 99.0 \
    -o ex1_se75_10x
fi

# 验证
wc -l ex1_se75_10x/simulated.fastq
# 预期: 533336 行 (133334 reads x 4 行/read)
```

- [ ] **Step 2: 生成 WGBS 双端 150bp 10x (Example 2)**
```bash
cd /workspace/bsmap-rs/benchmark/data/wgbs
if [ ! -f ex2_pe150_10x/simulated_1.fastq.gz ]; then
  mkdir -p ex2_pe150_10x
  /workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 150 \
    -n 66667 \
    -pe \
    -cr 99.0 \
    -o ex2_pe150_10x
fi

# 验证
wc -l ex2_pe150_10x/simulated_1.fastq ex2_pe150_10x/simulated_2.fastq
# 预期: 各 266668 行 (66667 reads x 4 行/read)
```

- [ ] **Step 3: 生成 WGBS 双端 150bp 20x (Example 5)**
```bash
cd /workspace/bsmap-rs/benchmark/data/wgbs
if [ ! -f ex5_pe150_20x/simulated_1.fastq.gz ]; then
  mkdir -p ex5_pe150_20x
  /workspace/bsmap-rs/tools/sherman/Sherman \
    --genome_folder /workspace/bsmap-rs/benchmark/data/ref \
    -l 150 \
    -n 133334 \
    -pe \
    -cr 99.0 \
    -o ex5_pe150_20x
fi

# 验证
wc -l ex5_pe150_20x/simulated_1.fastq ex5_pe150_20x/simulated_2.fastq
# 预期: 各 533336 行 (133334 reads x 4 行/read)
```

- [ ] **Step 4: 压缩所有 WGBS 数据为 fq.gz (如果未压缩)**
```bash
cd /workspace/bsmap-rs/benchmark/data/wgbs
if [ -f ex1_se75_10x/simulated.fastq ]; then
  gzip ex1_se75_10x/simulated.fastq
fi
if [ -f ex2_pe150_10x/simulated_1.fastq ]; then
  gzip ex2_pe150_10x/simulated_1.fastq
  gzip ex2_pe150_10x/simulated_2.fastq
fi
if [ -f ex5_pe150_20x/simulated_1.fastq ]; then
  gzip ex5_pe150_20x/simulated_1.fastq
  gzip ex5_pe150_20x/simulated_2.fastq
fi

# 验证
ls -lh ex1_se75_10x/ ex2_pe150_10x/ ex5_pe150_20x/
```

---

### Task 2.2: 生成 RRBS 测试数据 (RRBSsim)

- [ ] **Step 1: 生成 RRBS 单端 75bp 10x (Example 3)**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs
if [ ! -f ex3_se75_10x.1.fq.gz ]; then
  python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 10 \
    -l 75 \
    -s \
    -o ex3_se75_10x
fi

# 验证
wc -l ex3_se75_10x.1.fq
```

- [ ] **Step 2: 生成 RRBS 双端 150bp 10x (Example 4)**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs
if [ ! -f ex4_pe150_10x.1.fq.gz ]; then
  python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 10 \
    -l 150 \
    -p \
    -o ex4_pe150_10x
fi

# 验证
wc -l ex4_pe150_10x.1.fq ex4_pe150_10x.2.fq
```

- [ ] **Step 3: 生成 RRBS 双端 150bp 20x (Example 6)**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs
if [ ! -f ex6_pe150_20x.1.fq.gz ]; then
  python3 /workspace/bsmap-rs/tools/rrbssim/RRBSsim \
    -f /workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa \
    -d 20 \
    -l 150 \
    -p \
    -o ex6_pe150_20x
fi

# 验证
wc -l ex6_pe150_20x.1.fq ex6_pe150_20x.2.fq
```

- [ ] **Step 4: 压缩所有 RRBS 数据为 fq.gz (如果未压缩)**
```bash
cd /workspace/bsmap-rs/benchmark/data/rrbs
if [ -f ex3_se75_10x.1.fq ]; then
  gzip ex3_se75_10x.1.fq
fi
if [ -f ex4_pe150_10x.1.fq ]; then
  gzip ex4_pe150_10x.1.fq
  gzip ex4_pe150_10x.2.fq
fi
if [ -f ex6_pe150_20x.1.fq ]; then
  gzip ex6_pe150_20x.1.fq
  gzip ex6_pe150_20x.2.fq
fi

# 验证
ls -lh *.fq.gz
```

---

### Task 2.3: 解压数据到临时文件 (重要 - 避免进程替换问题)

- [ ] **Step 1: 解压 WGBS 数据**
```bash
cd /workspace/bsmap-rs/benchmark/tmp

# Example 1
if [ ! -f ex1_se75_10x.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/wgbs/ex1_se75_10x/simulated.fastq.gz > ex1_se75_10x.fastq
fi

# Example 2
if [ ! -f ex2_pe150_10x_1.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > ex2_pe150_10x_1.fastq
  gunzip -c /workspace/bsmap-rs/benchmark/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > ex2_pe150_10x_2.fastq
fi

# Example 5
if [ ! -f ex5_pe150_20x_1.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz > ex5_pe150_20x_1.fastq
  gunzip -c /workspace/bsmap-rs/benchmark/data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz > ex5_pe150_20x_2.fastq
fi

# 验证
ls -lh *.fastq
```

- [ ] **Step 2: 解压 RRBS 数据**
```bash
cd /workspace/bsmap-rs/benchmark/tmp

# Example 3
if [ ! -f ex3_se75_10x.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/rrbs/ex3_se75_10x.1.fq.gz > ex3_se75_10x.fastq
fi

# Example 4
if [ ! -f ex4_pe150_10x_1.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/rrbs/ex4_pe150_10x.1.fq.gz > ex4_pe150_10x_1.fastq
  gunzip -c /workspace/bsmap-rs/benchmark/data/rrbs/ex4_pe150_10x.2.fq.gz > ex4_pe150_10x_2.fastq
fi

# Example 6
if [ ! -f ex6_pe150_20x_1.fastq ]; then
  gunzip -c /workspace/bsmap-rs/benchmark/data/rrbs/ex6_pe150_20x.1.fq.gz > ex6_pe150_20x_1.fastq
  gunzip -c /workspace/bsmap-rs/benchmark/data/rrbs/ex6_pe150_20x.2.fq.gz > ex6_pe150_20x_2.fastq
fi

# 验证
ls -lh *.fastq
```

---

## 阶段 3: 索引构建测试

### Task 3.1: 构建索引 (已修正参数)

- [ ] **Step 1: 原版 BSMAP WGBS 索引**
```bash
cd /workspace/bsmap-rs/benchmark
if [ ! -f index/bsmap_wgbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -o index/bsmap_wgbs.bsi -s 16 -I 4 2>&1 | tee index/bsmap_wgbs_build.log
fi
```

- [ ] **Step 2: 原版 BSMAP RRBS 索引**
```bash
cd /workspace/bsmap-rs/benchmark
if [ ! -f index/bsmap_rrbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -o index/bsmap_rrbs.bsi -s 12 -I 4 -D C-CGG 2>&1 | tee index/bsmap_rrbs_build.log
fi
```

- [ ] **Step 3: bsmap-rs WGBS 索引 (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
if [ ! -f index/bsmaprs_wgbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa -o index/bsmaprs_wgbs.bsi -s 16 -I 4 2>&1 | tee index/bsmaprs_wgbs_build.log
fi
```

- [ ] **Step 4: bsmap-rs RRBS 索引 (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
if [ ! -f index/bsmaprs_rrbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa -o index/bsmaprs_rrbs.bsi -s 12 -I 4 -D C-CGG 2>&1 | tee index/bsmaprs_rrbs_build.log
fi
```

- [ ] **Step 5: 记录索引大小**
```bash
ls -lh index/*.bsi
```

---

## 阶段 4: 比对测试 (6 Examples - 使用预编译二进制和临时文件)

### Task 4.1: Example 1 - WGBS SE 75bp 10x (133,334 reads)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example1_wgbs_se
if [ ! -f results/example1_wgbs_se/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example1_wgbs_se/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example1_wgbs_se
if [ ! -f results/example1_wgbs_se/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example1_wgbs_se/bsmaprs.log
fi
```

---

### Task 4.2: Example 2 - WGBS PE 150bp 10x (66,667 pairs)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example2_wgbs_pe
if [ ! -f results/example2_wgbs_pe/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example2_wgbs_pe/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example2_wgbs_pe
if [ ! -f results/example2_wgbs_pe/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example2_wgbs_pe/bsmaprs.log
fi
```

---

### Task 4.3: Example 3 - RRBS SE 75bp 10x (~133K reads)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example3_rrbs_se
if [ ! -f results/example3_rrbs_se/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex3_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example3_rrbs_se/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example3_rrbs_se
if [ ! -f results/example3_rrbs_se/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex3_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example3_rrbs_se/bsmaprs.log
fi
```

---

### Task 4.4: Example 4 - RRBS PE 150bp 10x (~67K pairs)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example4_rrbs_pe
if [ ! -f results/example4_rrbs_pe/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex4_pe150_10x_1.fastq \
    -b tmp/ex4_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example4_rrbs_pe/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example4_rrbs_pe
if [ ! -f results/example4_rrbs_pe/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex4_pe150_10x_1.fastq \
    -b tmp/ex4_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example4_rrbs_pe/bsmaprs.log
fi
```

---

### Task 4.5: Example 5 - WGBS PE 150bp 20x (133,334 pairs)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example5_wgbs_pe_20x
if [ ! -f results/example5_wgbs_pe_20x/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex5_pe150_20x_1.fastq \
    -b tmp/ex5_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example5_wgbs_pe_20x/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example5_wgbs_pe_20x
if [ ! -f results/example5_wgbs_pe_20x/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex5_pe150_20x_1.fastq \
    -b tmp/ex5_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example5_wgbs_pe_20x/bsmaprs.log
fi
```

---

### Task 4.6: Example 6 - RRBS PE 150bp 20x (~133K pairs)

- [ ] **原版 BSMAP**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example6_rrbs_pe_20x
if [ ! -f results/example6_rrbs_pe_20x/bsmap.sam ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex6_pe150_20x_1.fastq \
    -b tmp/ex6_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example6_rrbs_pe_20x/bsmap.log
fi
```

- [ ] **bsmap-rs (使用预编译二进制)**
```bash
cd /workspace/bsmap-rs/benchmark
mkdir -p results/example6_rrbs_pe_20x
if [ ! -f results/example6_rrbs_pe_20x/bsmaprs.sam ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex6_pe150_20x_1.fastq \
    -b tmp/ex6_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example6_rrbs_pe_20x/bsmaprs.log
fi
```

---

## 阶段 5: SAM 一致性对比

### Task 5.1: SAM 对比脚本

```bash
#!/bin/bash
# compare_sam.sh - SAM 一致性对比
# Usage: bash compare_sam.sh sam1.sam sam2.sam output_dir

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
cd /workspace/bsmap-rs/benchmark
bash compare_sam.sh \
  results/example2_wgbs_pe/bsmap.sam \
  results/example2_wgbs_pe/bsmaprs.sam \
  results/example2_diff
```

- [ ] **Example 3: RRBS SE**
```bash
cd /workspace/bsmap-rs/benchmark
bash compare_sam.sh \
  results/example3_rrbs_se/bsmap.sam \
  results/example3_rrbs_se/bsmaprs.sam \
  results/example3_diff
```

- [ ] **Example 4: RRBS PE**
```bash
cd /workspace/bsmap-rs/benchmark
bash compare_sam.sh \
  results/example4_rrbs_pe/bsmap.sam \
  results/example4_rrbs_pe/bsmaprs.sam \
  results/example4_diff
```

- [ ] **Example 5: WGBS PE 20x**
```bash
cd /workspace/bsmap-rs/benchmark
bash compare_sam.sh \
  results/example5_wgbs_pe_20x/bsmap.sam \
  results/example5_wgbs_pe_20x/bsmaprs.sam \
  results/example5_diff
```

- [ ] **Example 6: RRBS PE 20x**
```bash
cd /workspace/bsmap-rs/benchmark
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
**环境**: 3核心 5.8GB内存 (或16GB如果已提升)
**参考基因组**: hg38 chr22 尾部 1M bp

---

## 1. 测试概览

| Example | 模式 | 数据类型 | 读段数 | 覆盖度 | 状态 |
|---------|------|---------|--------|--------|------|
| Example 1 | WGBS | 单端 75bp | 133,334 | 10x | ✅ |
| Example 2 | WGBS | 双端 150bp | 66,667 pairs | 10x | ✅ |
| Example 3 | RRBS | 单端 75bp | ~133K | 10x | ✅ |
| Example 4 | RRBS | 双端 150bp | ~67K pairs | 10x | ✅ |
| Example 5 | WGBS | 双端 150bp | 133,334 pairs | 20x | ✅ |
| Example 6 | RRBS | 双端 150bp | ~133K pairs | 20x | ✅ |

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

### 3.1 Example 1: WGBS 单端 75bp 10x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.2 Example 2: WGBS 双端 150bp 10x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.3 Example 3: RRBS 单端 75bp 10x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.4 Example 4: RRBS 双端 150bp 10x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.5 Example 5: WGBS 双端 150bp 20x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

### 3.6 Example 6: RRBS 双端 150bp 20x

| 指标 | BSMAP C++ | bsmap-rs | 比率 |
|------|-----------|-----------|------|
| 运行时间 | TBD | TBD | x |
| RSS 内存 | TBD | TBD | x |
| 比对率 | TBD | TBD | - |

SAM 一致性: TBD

---

## 4. 结论

[待补充详细结论]

---

**测试执行完成时间**: $(date)
REPORT_EOF
```

---

## 验证标准

- [x] chr22_tail_1M.fa 参考基因组已存在 (或可生成)
- [x] 测试数据已存在 (或可生成)
- [x] 所有参数已修正 (-s, -I, -D, align 子命令)
- [x] bsmap-rs 已预编译
- [x] tmp/ 目录已创建用于解压数据
- [ ] 原版和 Rust 版各运行 6 次比对
- [ ] SAM 对比结果记录到 diff 目录
- [ ] summary.csv 和 benchmark_report.md 生成完成

---

## Self-Review 检查清单

| 检查项 | 状态 |
|--------|------|
| 6 个 Example 全部执行 | |
| 内存使用在 16GB 内 (建议提升) | |
| SAM diff 分析完成 | |
| 报告生成 | |
| 结果可复现 | |

---

## 更新历史

| 日期 | 版本 | 说明 |
|------|------|------|
| 2026-05-15 | 1.0 | 初始计划 |
| 2026-05-17 | 1.1 | 修正所有参数，添加预编译和tmp/目录步骤，避免进程替换 |
