#!/bin/bash
# ==========================================
# 阶段1：索引构建
# 这一步可以选择统计或跳过（建议跳过，专注于比对）
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "阶段1：索引构建 (可选统计)"
echo "=========================================="
date
echo ""

# 清除旧索引
echo "清除旧索引..."
rm -f index/*.bsi
echo "✅ 旧索引已清除"

# 检查数据
if [ ! -f "data/chr22_tail_1M.fa" ]; then
    echo "❌ 参考基因组不存在"
    exit 1
fi

# 解压测试数据（提前解压好避免在比对阶段解压）
echo ""
echo "解压测试数据到 tmp/ 目录..."
mkdir -p tmp
if [ ! -f "tmp/ex1_se75_10x.fastq" ]; then
    gunzip -c "data/wgbs/ex1_se75_10x/simulated.fastq.gz" > tmp/ex1_se75_10x.fastq
fi
if [ ! -f "tmp/ex2_pe150_10x_1.fastq" ]; then
    gunzip -c "data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" > tmp/ex2_pe150_10x_1.fastq
    gunzip -c "data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" > tmp/ex2_pe150_10x_2.fastq
fi
if [ ! -f "tmp/ex5_pe150_20x_1.fastq" ]; then
    gunzip -c "data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz" > tmp/ex5_pe150_20x_1.fastq
    gunzip -c "data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz" > tmp/ex5_pe150_20x_2.fastq
fi
if [ ! -f "tmp/ex3_se75_10x.fastq" ]; then
    gunzip -c "data/rrbs/rrbssim/ex3_se75_10x.1.fq.gz" > tmp/ex3_se75_10x.fastq
fi
if [ ! -f "tmp/ex4_pe150_10x_1.fastq" ]; then
    gunzip -c "data/rrbs/rrbssim/ex4_pe150_10x.1.fq.gz" > tmp/ex4_pe150_10x_1.fastq
    gunzip -c "data/rrbs/rrbssim/ex4_pe150_10x.2.fq.gz" > tmp/ex4_pe150_10x_2.fastq
fi
if [ ! -f "tmp/ex6_pe150_20x_1.fastq" ]; then
    gunzip -c "data/rrbs/rrbssim/ex6_pe150_20x.1.fq.gz" > tmp/ex6_pe150_20x_1.fastq
    gunzip -c "data/rrbs/rrbssim/ex6_pe150_20x.2.fq.gz" > tmp/ex6_pe150_20x_2.fastq
fi
echo "✅ 所有测试数据解压完成"
ls -lh tmp/
echo ""

# 构建索引（不统计时间和内存，仅完成构建）
echo "构建 BSMAP C++ WGBS 索引 (seed=16)..."
cd /workspace/bsmap-original/bsmap-2.90
./bsmap -a "$WORK_DIR/data/chr22_tail_1M.fa" -o "$WORK_DIR/index/bsmap_wgbs.bsi" -s 16 -I 4 -p 1 2>&1
cd "$WORK_DIR"

echo "构建 BSMAP C++ RRBS 索引 (seed=12)..."
cd /workspace/bsmap-original/bsmap-2.90
./bsmap -a "$WORK_DIR/data/chr22_tail_1M.fa" -o "$WORK_DIR/index/bsmap_rrbs.bsi" -s 12 -I 4 -D C-CGG -p 1 2>&1
cd "$WORK_DIR"

echo "构建 bsmap-rs WGBS 索引 (seed=16)..."
/workspace/bsmap-rs/target/release/bsmap index -d data/chr22_tail_1M.fa -o index/bsmaprs_wgbs.bsi -s 16 -I 4 2>&1

echo "构建 bsmap-rs RRBS 索引 (seed=12)..."
/workspace/bsmap-rs/target/release/bsmap index -d data/chr22_tail_1M.fa -o index/bsmaprs_rrbs.bsi -s 12 -I 4 -D C-CGG 2>&1

echo ""
echo "✅ 所有索引构建完成！"
ls -lh index/*.bsi
date
