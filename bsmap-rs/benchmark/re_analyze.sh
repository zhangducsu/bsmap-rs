#!/bin/bash
# ===============================
# 重新分析已生成的SAM文件
# ===============================

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd $WORK_DIR

echo "=========================================="
echo "重新分析已生成的SAM比对结果"
echo "=========================================="
date
echo ""

# 确保输出目录存在
mkdir -p results/comparison_example1_wgbs_se
mkdir -p results/comparison_example2_wgbs_pe
mkdir -p results/comparison_example5_wgbs_pe_20x

# Example 1: WGBS SE 75bp 10x
if [ -f "results/example1_wgbs_se_bsmap/bsmap.sam" ] && [ -f "results/example1_wgbs_se_bsmaprs/bsmaprs.sam" ]; then
  echo "=== 分析 Example 1: WGBS SE 75bp 10x ==="
  python3 compare_sam.py \
    results/example1_wgbs_se_bsmap/bsmap.sam \
    results/example1_wgbs_se_bsmaprs/bsmaprs.sam \
    results/comparison_example1_wgbs_se \
    example1_wgbs_se
  echo ""
else
  echo "Example 1 SAM文件不存在，跳过"
  echo ""
fi

# Example 2: WGBS PE 150bp 10x
if [ -f "results/example2_wgbs_pe_bsmap/bsmap.sam" ] && [ -f "results/example2_wgbs_pe_bsmaprs/bsmaprs.sam" ]; then
  echo "=== 分析 Example 2: WGBS PE 150bp 10x ==="
  python3 compare_sam.py \
    results/example2_wgbs_pe_bsmap/bsmap.sam \
    results/example2_wgbs_pe_bsmaprs/bsmaprs.sam \
    results/comparison_example2_wgbs_pe \
    example2_wgbs_pe
  echo ""
else
  echo "Example 2 SAM文件不存在，跳过"
  echo ""
fi

# Example 5: WGBS PE 150bp 20x
if [ -f "results/example5_wgbs_pe_20x_bsmap/bsmap.sam" ] && [ -f "results/example5_wgbs_pe_20x_bsmaprs/bsmaprs.sam" ]; then
  echo "=== 分析 Example 5: WGBS PE 150bp 20x ==="
  python3 compare_sam.py \
    results/example5_wgbs_pe_20x_bsmap/bsmap.sam \
    results/example5_wgbs_pe_20x_bsmaprs/bsmaprs.sam \
    results/comparison_example5_wgbs_pe_20x \
    example5_wgbs_pe_20x
  echo ""
else
  echo "Example 5 SAM文件不存在，跳过"
  echo ""
fi

echo "=========================================="
echo "分析完成!"
echo "结果目录: results/comparison_*"
echo "=========================================="
