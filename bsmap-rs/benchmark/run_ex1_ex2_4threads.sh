#!/bin/bash
# ==========================================
# 仅运行 Example 1 和 Example 2 的基准测试 (4线程)
# Ex1: WGBS SE 75bp 10x
# Ex2: WGBS PE 150bp 10x
# ==========================================
set -e
WORK_DIR="/f/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试 (4线程)"
echo "=========================================="
echo "  运行环境: Docker 20GB内存"
echo "  线程数: 4"
echo "  测试内容: Ex1 (WGBS SE), Ex2 (WGBS PE)"
echo "  功能: 比对测试 + SAM对比 + 报告"
echo "=========================================="
date
echo ""

# ======================================
# 步骤1：清理旧结果并准备
# ======================================
echo ">>> 步骤1：清理旧结果并准备..."
rm -rf results_4threads/*
mkdir -p results_4threads

# ======================================
# 步骤2：解压测试数据
# ======================================
echo ""
echo ">>> 步骤2：解压测试数据到 tmp/..."
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
# 步骤3：定义比对函数
# ======================================
echo ""
echo ">>> 步骤3：定义测试函数..."

run_bsmap_cpp() {
    local EXAMPLE=$1
    local MODE=$2
    local READ1=$3
    local READ2=$4
    local SEED=$5
    local EXTRA=$6
    
    echo "  [$EXAMPLE] BSMAP C++ (4线程)..."
    local RESULT_DIR="results_4threads/${EXAMPLE}_bsmap"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmap.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmap.log
    else
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmap.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmap.log
    fi
}

run_bsmap_rs() {
    local EXAMPLE=$1
    local MODE=$2
    local READ1=$3
    local READ2=$4
    local SEED=$5
    local EXTRA=$6
    
    echo "  [$EXAMPLE] bsmap-rs (4线程)..."
    local RESULT_DIR="results_4threads/${EXAMPLE}_bsmaprs"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    else
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 4 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    fi
}

# ======================================
# 步骤4：执行 Ex1
# ======================================
echo ""
echo "======================================"
echo "Example 1: WGBS SE 75bp 10x"
echo "======================================"
run_bsmap_cpp "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 ""
run_bsmap_rs "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 ""

# ======================================
# 步骤5：执行 Ex2
# ======================================
echo ""
echo "======================================"
echo "Example 2: WGBS PE 150bp 10x"
echo "======================================"
run_bsmap_cpp "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 ""
run_bsmap_rs "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 ""

# ======================================
# 步骤6：SAM详细对比
# ======================================
echo ""
echo "======================================"
echo "步骤6：SAM 详细对比分析"
echo "======================================"

mkdir -p results_4threads/comparison_example1_wgbs_se
mkdir -p results_4threads/comparison_example2_wgbs_pe

if [ -f compare_sam.py ]; then
    echo "  运行详细对比..."
    python3 compare_sam.py \
        results_4threads/example1_wgbs_se_bsmap/bsmap.sam \
        results_4threads/example1_wgbs_se_bsmaprs/bsmaprs.sam \
        results_4threads/comparison_example1_wgbs_se \
        "example1_wgbs_se"
    
    python3 compare_sam.py \
        results_4threads/example2_wgbs_pe_bsmap/bsmap.sam \
        results_4threads/example2_wgbs_pe_bsmaprs/bsmaprs.sam \
        results_4threads/comparison_example2_wgbs_pe \
        "example2_wgbs_pe"
else
    echo "  compare_sam.py 不存在，使用简单对比..."
    # 简单版对比
    grep -v "^@" results_4threads/example1_wgbs_se_bsmap/bsmap.sam | sort > results_4threads/example1_wgbs_se_bsmap/sam1_sorted.sam
    grep -v "^@" results_4threads/example1_wgbs_se_bsmaprs/bsmaprs.sam | sort > results_4threads/example1_wgbs_se_bsmaprs/sam2_sorted.sam
    echo "Example1 SAM对比" > results_4threads/comparison_example1_wgbs_se/simple_report.txt
    echo "BSMAP C++比对数: $(wc -l results_4threads/example1_wgbs_se_bsmap/sam1_sorted.sam)" >> results_4threads/comparison_example1_wgbs_se/simple_report.txt
    echo "bsmap-rs比对数: $(wc -l results_4threads/example1_wgbs_se_bsmaprs/sam2_sorted.sam)" >> results_4threads/comparison_example1_wgbs_se/simple_report.txt
fi

# ======================================
# 步骤7：生成汇总CSV
# ======================================
echo ""
echo "======================================"
echo "步骤7：生成结果汇总"
echo "======================================"

cat > results_4threads/summary.csv << 'CSV_HEADER'
example,tool,mode,time_wall_sec,time_user_sec,time_sys_sec,mem_max_rss_kb
CSV_HEADER

extract_stats() {
    local EXAMPLE=$1
    local TOOL=$2
    local MODE=$3
    local LOG_FILE="results_4threads/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "wall clock" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}' || echo "0")
    local USER=$(grep "user" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local SYS=$(grep "sys" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}' || echo "0")
    
    echo "$EXAMPLE,$TOOL,$MODE,$WALL,$USER,$SYS,$RSS"
}

echo "  提取统计数据..."
extract_stats "example1_wgbs_se" "bsmap" "wgbs" >> results_4threads/summary.csv
extract_stats "example1_wgbs_se" "bsmaprs" "wgbs" >> results_4threads/summary.csv
extract_stats "example2_wgbs_pe" "bsmap" "wgbs" >> results_4threads/summary.csv
extract_stats "example2_wgbs_pe" "bsmaprs" "wgbs" >> results_4threads/summary.csv

echo ""
echo "=== 测试结果汇总 (summary.csv) ==="
cat results_4threads/summary.csv

# ======================================
# 步骤8：生成最终报告
# ======================================
echo ""
echo "======================================"
echo "步骤8：生成最终报告"
echo "======================================"

cat > results_4threads/final_report.md << 'REPORT'
# BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试报告 (4线程)

## 测试环境
- Docker 内存限制：20GB
- 预编译和预建索引：是
- 统计范围：仅比对环节
- 线程数：4
- 测试日期：$(date)

## 测试内容
1. **Example 1**: WGBS SE 75bp 10x
2. **Example 2**: WGBS PE 150bp 10x

## 性能对比

### Example 1: WGBS SE 75bp 10x
- BSMAP C++:
- bsmap-rs:

### Example 2: WGBS PE 150bp 10x
- BSMAP C++:
- bsmap-rs:

## SAM一致性对比
- Example 1: [详细报告](comparison_example1_wgbs_se/detailed_report.txt)
- Example 2: [详细报告](comparison_example2_wgbs_pe/detailed_report.txt)

## 数据汇总
REPORT

date > results_4threads/run_date.txt
cat results_4threads/summary.csv >> results_4threads/final_report.md

echo ""
echo "=========================================="
echo "✅ Ex1 & Ex2 4线程基准测试完成！"
echo "=========================================="
echo "结果目录：results_4threads/"
echo "汇总文件：results_4threads/summary.csv"
echo "最终报告：results_4threads/final_report.md"
echo "=========================================="
date
