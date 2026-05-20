#!/bin/bash
set -e

# bsmap-rs vs BSMAP C++ 对比测试 - 阶段3-6执行脚本 v2
# 更新日期: 2026-05-17
# 主要修改:
# - 修正所有参数 (-s, -I, -D)
# - 使用预编译二进制
# - 解压到 tmp/ 目录，避免进程替换问题

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "开始执行阶段3-6 benchmark测试 (v2)"
echo "=========================================="
date
echo ""

# 创建必要目录
mkdir -p index results report tmp

# ========================================
# 阶段3: 索引构建测试
# ========================================
echo ""
echo "=========================================="
echo "阶段3: 索引构建测试"
echo "=========================================="

# Step 1: 原版 BSMAP WGBS 索引
echo ""
echo "[3-1] 构建原版 BSMAP WGBS 索引..."
if [ ! -f index/bsmap_wgbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -o index/bsmap_wgbs.bsi -s 16 -I 4 2>&1 | tee index/bsmap_wgbs_build.log || {
    echo "原版 BSMAP WGBS 索引构建失败，继续执行..."
  }
else
  echo "索引已存在，跳过"
fi

# Step 2: 原版 BSMAP RRBS 索引
echo ""
echo "[3-2] 构建原版 BSMAP RRBS 索引..."
if [ ! -f index/bsmap_rrbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a data/chr22_tail_1M.fa -o index/bsmap_rrbs.bsi -s 12 -I 4 -D C-CGG 2>&1 | tee index/bsmap_rrbs_build.log || {
    echo "原版 BSMAP RRBS 索引构建失败，继续执行..."
  }
else
  echo "索引已存在，跳过"
fi

# Step 3: bsmap-rs WGBS 索引 (使用预编译二进制)
echo ""
echo "[3-3] 构建 bsmap-rs WGBS 索引..."
if [ ! -f index/bsmaprs_wgbs.bsi ]; then
  export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa -o index/bsmaprs_wgbs.bsi -s 16 -I 4 2>&1 | tee index/bsmaprs_wgbs_build.log || {
    echo "bsmap-rs WGBS 索引构建失败，继续执行..."
  }
else
  echo "索引已存在，跳过"
fi

# Step 4: bsmap-rs RRBS 索引 (使用预编译二进制)
echo ""
echo "[3-4] 构建 bsmap-rs RRBS 索引..."
if [ ! -f index/bsmaprs_rrbs.bsi ]; then
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa -o index/bsmaprs_rrbs.bsi -s 12 -I 4 -D C-CGG 2>&1 | tee index/bsmaprs_rrbs_build.log || {
    echo "bsmap-rs RRBS 索引构建失败，继续执行..."
  }
else
  echo "索引已存在，跳过"
fi

# Step 5: 记录索引大小
echo ""
echo "[3-5] 索引文件大小:"
ls -lh index/*.bsi || true

# ========================================
# 阶段2补充: 解压测试数据到临时目录
# ========================================
echo ""
echo "=========================================="
echo "解压测试数据到 tmp/ 目录"
echo "=========================================="

# 解压 WGBS 数据
echo ""
echo "解压 WGBS 数据..."
if [ ! -f tmp/ex1_se75_10x.fastq ]; then
  gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
else
  echo "ex1 已解压，跳过"
fi

if [ ! -f tmp/ex2_pe150_10x_1.fastq ]; then
  gunzip -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > tmp/ex2_pe150_10x_1.fastq
  gunzip -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > tmp/ex2_pe150_10x_2.fastq
else
  echo "ex2 已解压，跳过"
fi

if [ ! -f tmp/ex5_pe150_20x_1.fastq ]; then
  gunzip -c data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz > tmp/ex5_pe150_20x_1.fastq
  gunzip -c data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz > tmp/ex5_pe150_20x_2.fastq
else
  echo "ex5 已解压，跳过"
fi

# 解压 RRBS 数据
echo ""
echo "解压 RRBS 数据..."
if [ ! -f tmp/ex3_se75_10x.fastq ]; then
  gunzip -c data/rrbs/ex3_se75_10x.1.fq.gz > tmp/ex3_se75_10x.fastq
else
  echo "ex3 已解压，跳过"
fi

if [ ! -f tmp/ex4_pe150_10x_1.fastq ]; then
  gunzip -c data/rrbs/ex4_pe150_10x.1.fq.gz > tmp/ex4_pe150_10x_1.fastq
  gunzip -c data/rrbs/ex4_pe150_10x.2.fq.gz > tmp/ex4_pe150_10x_2.fastq
else
  echo "ex4 已解压，跳过"
fi

if [ ! -f tmp/ex6_pe150_20x_1.fastq ]; then
  gunzip -c data/rrbs/ex6_pe150_20x.1.fq.gz > tmp/ex6_pe150_20x_1.fastq
  gunzip -c data/rrbs/ex6_pe150_20x.2.fq.gz > tmp/ex6_pe150_20x_2.fastq
else
  echo "ex6 已解压，跳过"
fi

echo ""
echo "解压文件列表:"
ls -lh tmp/

# ========================================
# 阶段4: 比对测试 (6 Examples)
# ========================================
echo ""
echo "=========================================="
echo "阶段4: 比对测试 (6 Examples)"
echo "=========================================="

# 创建 compare_sam.sh 脚本
cat > compare_sam.sh << 'SAM_COMPARE_EOF'
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

cat $OUT/diff_report.txt
SAM_COMPARE_EOF
chmod +x compare_sam.sh

# --- Example 1: WGBS SE 75bp 10x ---
echo ""
echo "[4-1] Example 1: WGBS SE 75bp 10x"
mkdir -p results/example1_wgbs_se
if [ ! -f results/example1_wgbs_se/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example1_wgbs_se/bsmap.log || {
    echo "原版 BSMAP Example 1 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example1_wgbs_se/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  export PATH="/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/:$PATH"
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example1_wgbs_se/bsmaprs.log || {
    echo "bsmap-rs Example 1 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# --- Example 2: WGBS PE 150bp 10x ---
echo ""
echo "[4-2] Example 2: WGBS PE 150bp 10x"
mkdir -p results/example2_wgbs_pe
if [ ! -f results/example2_wgbs_pe/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example2_wgbs_pe/bsmap.log || {
    echo "原版 BSMAP Example 2 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example2_wgbs_pe/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example2_wgbs_pe/bsmaprs.log || {
    echo "bsmap-rs Example 2 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# --- Example 3: RRBS SE 75bp 10x ---
echo ""
echo "[4-3] Example 3: RRBS SE 75bp 10x"
mkdir -p results/example3_rrbs_se
if [ ! -f results/example3_rrbs_se/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex3_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example3_rrbs_se/bsmap.log || {
    echo "原版 BSMAP Example 3 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example3_rrbs_se/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex3_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example3_rrbs_se/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example3_rrbs_se/bsmaprs.log || {
    echo "bsmap-rs Example 3 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# --- Example 4: RRBS PE 150bp 10x ---
echo ""
echo "[4-4] Example 4: RRBS PE 150bp 10x"
mkdir -p results/example4_rrbs_pe
if [ ! -f results/example4_rrbs_pe/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex4_pe150_10x_1.fastq \
    -b tmp/ex4_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example4_rrbs_pe/bsmap.log || {
    echo "原版 BSMAP Example 4 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example4_rrbs_pe/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex4_pe150_10x_1.fastq \
    -b tmp/ex4_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example4_rrbs_pe/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example4_rrbs_pe/bsmaprs.log || {
    echo "bsmap-rs Example 4 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# --- Example 5: WGBS PE 150bp 20x ---
echo ""
echo "[4-5] Example 5: WGBS PE 150bp 20x"
mkdir -p results/example5_wgbs_pe_20x
if [ ! -f results/example5_wgbs_pe_20x/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex5_pe150_20x_1.fastq \
    -b tmp/ex5_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example5_wgbs_pe_20x/bsmap.log || {
    echo "原版 BSMAP Example 5 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example5_wgbs_pe_20x/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex5_pe150_20x_1.fastq \
    -b tmp/ex5_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example5_wgbs_pe_20x/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/example5_wgbs_pe_20x/bsmaprs.log || {
    echo "bsmap-rs Example 5 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# --- Example 6: RRBS PE 150bp 20x ---
echo ""
echo "[4-6] Example 6: RRBS PE 150bp 20x"
mkdir -p results/example6_rrbs_pe_20x
if [ ! -f results/example6_rrbs_pe_20x/bsmap.sam ]; then
  echo "    运行原版 BSMAP..."
  /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex6_pe150_20x_1.fastq \
    -b tmp/ex6_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmap.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example6_rrbs_pe_20x/bsmap.log || {
    echo "原版 BSMAP Example 6 运行失败"
  }
else
  echo "    原版结果已存在，跳过"
fi

if [ ! -f results/example6_rrbs_pe_20x/bsmaprs.sam ]; then
  echo "    运行 bsmap-rs..."
  /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex6_pe150_20x_1.fastq \
    -b tmp/ex6_pe150_20x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example6_rrbs_pe_20x/bsmaprs.sam \
    -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/example6_rrbs_pe_20x/bsmaprs.log || {
    echo "bsmap-rs Example 6 运行失败"
  }
else
  echo "    bsmap-rs 结果已存在，跳过"
fi

# ========================================
# 阶段5: SAM 一致性对比
# ========================================
echo ""
echo "=========================================="
echo "阶段5: SAM 一致性对比"
echo "=========================================="

echo "[5-1] Example 1: WGBS SE"
if [ -f results/example1_wgbs_se/bsmap.sam ] && [ -f results/example1_wgbs_se/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example1_wgbs_se/bsmap.sam \
    results/example1_wgbs_se/bsmaprs.sam \
    results/example1_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

echo "[5-2] Example 2: WGBS PE"
if [ -f results/example2_wgbs_pe/bsmap.sam ] && [ -f results/example2_wgbs_pe/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example2_wgbs_pe/bsmap.sam \
    results/example2_wgbs_pe/bsmaprs.sam \
    results/example2_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

echo "[5-3] Example 3: RRBS SE"
if [ -f results/example3_rrbs_se/bsmap.sam ] && [ -f results/example3_rrbs_se/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example3_rrbs_se/bsmap.sam \
    results/example3_rrbs_se/bsmaprs.sam \
    results/example3_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

echo "[5-4] Example 4: RRBS PE"
if [ -f results/example4_rrbs_pe/bsmap.sam ] && [ -f results/example4_rrbs_pe/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example4_rrbs_pe/bsmap.sam \
    results/example4_rrbs_pe/bsmaprs.sam \
    results/example4_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

echo "[5-5] Example 5: WGBS PE 20x"
if [ -f results/example5_wgbs_pe_20x/bsmap.sam ] && [ -f results/example5_wgbs_pe_20x/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example5_wgbs_pe_20x/bsmap.sam \
    results/example5_wgbs_pe_20x/bsmaprs.sam \
    results/example5_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

echo "[5-6] Example 6: RRBS PE 20x"
if [ -f results/example6_rrbs_pe_20x/bsmap.sam ] && [ -f results/example6_rrbs_pe_20x/bsmaprs.sam ]; then
  bash compare_sam.sh \
    results/example6_rrbs_pe_20x/bsmap.sam \
    results/example6_rrbs_pe_20x/bsmaprs.sam \
    results/example6_diff || true
else
  echo "结果文件不存在，跳过对比"
fi

# ========================================
# 阶段6: 报告生成
# ========================================
echo ""
echo "=========================================="
echo "阶段6: 报告生成"
echo "=========================================="

# Step 1: 生成 summary.csv
echo "[6-1] 生成 summary.csv..."
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
      WALL=$(grep "wall clock" $LOG | awk '{print $NF}' || echo "")
      USER=$(grep "user" $LOG | head -1 | awk '{print $NF}' || echo "")
      SYS=$(grep "sys" $LOG | head -1 | awk '{print $NF}' || echo "")
      RSS=$(grep "Maximum resident" $LOG | awk '{print $NF}' || echo "")
      echo "example${i},${tool},${MODE},,$WALL,$USER,$SYS,$RSS," >> results/summary.csv
    fi
  done
done

echo "summary.csv 内容:"
cat results/summary.csv

# Step 2: 生成 benchmark_report.md
echo ""
echo "[6-2] 生成 benchmark_report.md..."
cat > report/benchmark_report_v2.md << 'REPORT_EOF'
# bsmap-rs vs BSMAP C++ 对比测试报告 (v2)

**日期**: $(date +%Y-%m-%d)
**环境**: 3核心 5.8GB内存 (Docker)
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

echo ""
echo "=========================================="
echo "所有阶段执行完成！"
echo "=========================================="
echo "结果文件:"
echo "  - summary.csv: $WORK_DIR/results/summary.csv"
echo "  - benchmark_report_v2.md: $WORK_DIR/report/benchmark_report_v2.md"
echo "  - SAM 对比结果: $WORK_DIR/results/example*_diff/"
date
