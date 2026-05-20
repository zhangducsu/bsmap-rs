#!/bin/bash
# ========================================
# 完整对比测试：原版C++ BSMAP vs bsmap-rs (Mmap模式)
# ========================================

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "============================================="
echo "BSMAP C++ vs bsmap-rs (Mmap) 完整对比测试"
echo "============================================="
date
echo ""

# 准备
mkdir -p tmp results report
export LC_ALL=C

# ========================================
# Step 1: 准备测试数据
# ========================================
echo "[步骤 1] 准备测试数据..."
if [ ! -f tmp/ex1_se75_10x.fastq ]; then
  gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
fi
echo "✓ 测试数据准备完成"

# ========================================
# Step 2: 构建V3索引（使用Mmap模式需要V3索引
# ========================================
echo ""
echo "[步骤 2] 构建 bsmap-rs V3 索引..."
cd /workspace/bsmap-rs
rm -f benchmark/data/chr22_tail_1M.fa.bsi
/workspace/bsmap-rs/target/release/bsmap index -d benchmark/data/chr22_tail_1M.fa -s 16 2>&1 | tee benchmark/results/index_build.log
cd "$WORK_DIR"
echo "✓ 索引构建完成"

# ========================================
# Step 3: 运行原版C++ BSMAP
# ========================================
echo ""
echo "[步骤 3] 运行原版 C++ BSMAP..."
/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o results/cpp_align.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/cpp_align.log
echo "✓ C++ BSMAP 运行完成"

# ========================================
# Step 4: 运行bsmap-rs (Mmap模式)
# ========================================
echo ""
echo "[步骤 4] 运行 bsmap-rs (Mmap模式)..."
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o results/rs_mmap_align.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/rs_mmap_align.log
echo "✓ bsmap-rs (Mmap) 运行完成"

# ========================================
# Step 5: SAM详细对比
# ========================================
echo ""
echo "[步骤 5] 执行 SAM 详细对比..."
chmod +x compare_sam_detailed.sh
./compare_sam_detailed.sh \
  results/cpp_align.sam \
  results/rs_mmap_align.sam \
  results/comparison \
  "example1_wgbs_se"

# ========================================
# Step 6: 性能数据提取和报告生成
# ========================================
echo ""
echo "[步骤 6] 生成完整报告..."

# 性能数据提取
extract_perf() {
  LOG=$1
  if [ -f "$LOG" ]; then
    WALL=$(grep "wall clock" $LOG | awk '{print $NF}' | sed 's/,/./g')
    USER=$(grep "user" $LOG | head -1 | awk '{print $NF}' | sed 's/,/./g')
    SYS=$(grep "sys" $LOG | head -1 | awk '{print $NF}' | sed 's/,/./g')
    RSS=$(grep "Maximum resident" $LOG | awk '{print $NF}')
    echo "$WALL,$USER,$SYS,$RSS"
  fi
}

CPP_PERF=$(extract_perf results/cpp_align.log)
RS_PERF=$(extract_perf results/rs_mmap_align.log)

# 生成性能汇总
echo "tool,wall_time,user_time,sys_time,max_rss_kb" > results/performance_summary.csv
echo "BSMAP_C++,${CPP_PERF}" >> results/performance_summary.csv
echo "bsmap-rs_Mmap,${RS_PERF}" >> results/performance_summary.csv

# 生成最终报告
cat > results/final_comparison_report.md << REPORT
# BSMAP C++ vs bsmap-rs (Mmap模式) 完整对比报告

## 测试时间
$(date)

## 1. 性能对比

| 工具 | 运行时间(秒) | 用户CPU时间(秒) | 系统CPU时间(秒) | 最大内存使用(KB) |
|------|-------------|----------------|----------------|----------------|
$(cat results/performance_summary.csv | sed 2,3d' | awk -F',' '{printf "| %s | %s | %s | %s | %s |\n", \$1, \$2, \$3, \$4, \$5}' )

## 2. SAM比对一致性

详细内容详见：results/comparison/detailed_report.txt

REPORT

echo "✓ 完整报告生成完成"

echo ""
echo "============================================="
echo "完整对比测试完成！"
echo "============================================="
echo "结果文件："
echo "  - results/cpp_align.sam"
echo "  - results/rs_mmap_align.sam"
echo "  - results/final_comparison_report.md"
echo "  - results/performance_summary.csv"
echo "  - results/comparison/detailed_report.txt"
echo "============================================="
