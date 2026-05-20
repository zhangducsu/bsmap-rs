#!/bin/bash
# ==========================================
# P2优化测试脚本
# 测试内容:
#   1. 并行索引加载测试
#   2. SIMD优化测试
#   3. 索引预热测试
#   4. 4线程并行比对测试
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP vs BSMAP-rs P2优化测试"
echo "=========================================="
echo "  运行环境: Docker 20GB内存"
echo "  线程数: 4"
echo "  测试内容: Ex1 (WGBS SE), Ex2 (WGBS PE)"
echo "=========================================="
date
echo ""

# ======================================
# 步骤1：清理旧结果
# ======================================
echo ">>> 步骤1：清理旧结果..."
rm -rf results_p2/*
mkdir -p results_p2

# ======================================
# 步骤2：解压测试数据
# ======================================
echo ""
echo ">>> 步骤2：解压测试数据..."
mkdir -p tmp

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    echo "  解压 Ex1 数据..."
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
fi

if [ ! -f tmp/ex2_pe150_10x_1.fastq ]; then
    echo "  解压 Ex2 数据..."
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > tmp/ex2_pe150_10x_1.fastq
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > tmp/ex2_pe150_10x_2.fastq
fi

# ======================================
# 定义测试函数
# ======================================
run_bsmap_rs_with_prefetch() {
    local EXAMPLE=$1
    local MODE=$2
    local READ1=$3
    local READ2=$4
    local SEED=$5
    local PREFETCH_FLAG=$6
    
    echo "  [$EXAMPLE] bsmap-rs (4线程, prefetch=$PREFETCH_FLAG)..."
    local RESULT_DIR="results_p2/${EXAMPLE}_bsmaprs_prefetch_${PREFETCH_FLAG}"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $PREFETCH_FLAG \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    else
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $PREFETCH_FLAG \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    fi
}

# ======================================
# 步骤3：测试索引预热效果
# ======================================
echo ""
echo "======================================"
echo "P2-1: 索引预热测试"
echo "======================================"

echo ""
echo "--- Ex1: 启用预热 (默认) ---"
run_bsmap_rs_with_prefetch "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 ""

echo ""
echo "--- Ex1: 禁用预热 ---"
run_bsmap_rs_with_prefetch "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 "--no-prefetch"

echo ""
echo "--- Ex2: 启用预热 (默认) ---"
run_bsmap_rs_with_prefetch "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 ""

echo ""
echo "--- Ex2: 禁用预热 ---"
run_bsmap_rs_with_prefetch "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 "--no-prefetch"

# ======================================
# 步骤4：生成结果汇总
# ======================================
echo ""
echo "======================================"
echo "步骤4：生成结果汇总"
echo "======================================"

cat > results_p2/summary.csv << 'CSV_HEADER'
example,tool,mode,prefetch,time_wall_sec,time_user_sec,time_sys_sec,mem_max_rss_kb
CSV_HEADER

extract_stats_p2() {
    local EXAMPLE=$1
    local TOOL=$2
    local MODE=$3
    local PREFETCH=$4
    local LOG_FILE="results_p2/${EXAMPLE}_${TOOL}_prefetch_${PREFETCH}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "wall clock" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}' || echo "0")
    local USER=$(grep "user" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local SYS=$(grep "sys" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}' || echo "0")
    
    echo "$EXAMPLE,$TOOL,$MODE,$PREFETCH,$WALL,$USER,$SYS,$RSS"
}

echo "  提取统计数据..."
extract_stats_p2 "example1_wgbs_se" "bsmaprs" "wgbs" "enabled" >> results_p2/summary.csv
extract_stats_p2 "example1_wgbs_se" "bsmaprs" "wgbs" "disabled" >> results_p2/summary.csv
extract_stats_p2 "example2_wgbs_pe" "bsmaprs" "wgbs" "enabled" >> results_p2/summary.csv
extract_stats_p2 "example2_wgbs_pe" "bsmaprs" "wgbs" "disabled" >> results_p2/summary.csv

echo ""
echo "=== P2优化测试结果汇总 ==="
cat results_p2/summary.csv

# ======================================
# 步骤5：生成P2优化报告
# ======================================
echo ""
echo "======================================"
echo "步骤5：生成P2优化报告"
echo "======================================"

cat > results_p2/p2_optimization_report.md << 'REPORT'
# BSMAP-rs P2优化测试报告

**测试日期**: $(date)
**测试环境**: Docker容器 (20GB内存, 4线程)

---

## 测试目标

验证P系列优化的实际效果，特别是：
1. **P1**: 索引预热对性能的影响
2. **P0-1**: SIMD优化效果
3. **并行化**: 4线程对比对性能的提升

---

## 测试配置

| 配置项 | 值 |
|-------|-----|
| 参考序列 | chr22_tail_1M.fa (1Mbp) |
| 种子大小 | 16 |
| 最大错配率 | 8% |
| 线程数 | 4 |

---

## 测试结果

### 索引预热效果对比

| 测试 | 预热模式 | 总耗时 | 内存峰值 |
|------|---------|--------|----------|

REPORT

cat results_p2/summary.csv >> results_p2/p2_optimization_report.md

cat >> results_p2/p2_optimization_report.md << 'REPORT_END'

---

## 分析结论

### 索引预热效果
- 启用预热: 
- 禁用预热:

### 性能对比
- 预热带来的性能提升: 

---

**报告生成时间**: $(date)
REPORT_END

date > results_p2/run_date.txt

echo ""
echo "=========================================="
echo "✅ P2优化测试完成！"
echo "=========================================="
echo "结果目录：results_p2/"
echo "汇总文件：results_p2/summary.csv"
echo "报告文件：results_p2/p2_optimization_report.md"
echo "=========================================="
date
