#!/bin/bash
# ==========================================
# 阶段2：比对测试（核心！只统计比对环节）
# 此阶段使用20GB内存，预编译和索引已完成
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "阶段2：比对测试 (核心环节)"
echo "✅ 预编译和索引构建已完成"
echo "✅ 测试数据已解压到 tmp/"
echo "=========================================="
date
echo ""

# 清除旧结果
rm -rf results/*
mkdir -p results

# 创建 SAM 对比脚本
cat > compare_sam.sh << 'EOF'
#!/bin/bash
SAM1=$1
SAM2=$2
OUT=$3
mkdir -p $OUT
grep -v "^@" $SAM1 | sort > $OUT/sam1_sorted.sam
grep -v "^@" $SAM2 | sort > $OUT/sam2_sorted.sam
echo "=== SAM 记录数 ===" > $OUT/diff_report.txt
wc -l $OUT/sam1_sorted.sam >> $OUT/diff_report.txt
wc -l $OUT/sam2_sorted.sam >> $OUT/diff_report.txt
echo "" >> $OUT/diff_report.txt
echo "=== 差异统计 ===" >> $OUT/diff_report.txt
diff $OUT/sam1_sorted.sam $OUT/sam2_sorted.sam | grep "^[<>]" | wc -l >> $OUT/diff_report.txt
echo "=== 差异详情（前50条）===" >> $OUT/diff_report.txt
diff $OUT/sam1_sorted.sam $OUT/sam2_sorted.sam | head -100 >> $OUT/diff_report.txt
cat $OUT/diff_report.txt
EOF
chmod +x compare_sam.sh

# 比对函数
run_bsmap_cpp() {
    local EXAMPLE=$1
    local MODE=$2
    local READ1=$3
    local READ2=$4
    local SEED=$5
    local EXTRA=$6
    
    echo "[$EXAMPLE] BSMAP C++ (Mode=$MODE, Seed=$SEED)..."
    local RESULT_DIR="results/${EXAMPLE}_bsmap"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        # 单端
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmap.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmap.log
    else
        # 双端
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmap.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
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
    
    echo "[$EXAMPLE] bsmap-rs (Mode=$MODE, Seed=$SEED)..."
    local RESULT_DIR="results/${EXAMPLE}_bsmaprs"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        # 单端
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    else
        # 双端
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    fi
}

compare_results() {
    local EXAMPLE=$1
    echo "[$EXAMPLE] SAM 一致性对比..."
    bash compare_sam.sh \
        results/${EXAMPLE}_bsmap/bsmap.sam \
        results/${EXAMPLE}_bsmaprs/bsmaprs.sam \
        results/${EXAMPLE}_diff
}

# ======================================
# 执行所有6个Examples
# ======================================

# Example 1: WGBS SE 75bp 10x
echo ""
echo "======================================"
echo "Example 1: WGBS SE 75bp 10x (133,334 reads)"
echo "======================================"
run_bsmap_cpp "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 ""
run_bsmap_rs "example1_wgbs_se" "wgbs" "ex1_se75_10x.fastq" "" 16 ""
compare_results "example1_wgbs_se"

# Example 2: WGBS PE 150bp 10x
echo ""
echo "======================================"
echo "Example 2: WGBS PE 150bp 10x (66,667 pairs)"
echo "======================================"
run_bsmap_cpp "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 ""
run_bsmap_rs "example2_wgbs_pe" "wgbs" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq" 16 ""
compare_results "example2_wgbs_pe"

# Example 3: RRBS SE 75bp 10x
echo ""
echo "======================================"
echo "Example 3: RRBS SE 75bp 10x"
echo "======================================"
run_bsmap_cpp "example3_rrbs_se" "rrbs" "ex3_se75_10x.fastq" "" 12 "-D C-CGG"
run_bsmap_rs "example3_rrbs_se" "rrbs" "ex3_se75_10x.fastq" "" 12 "-D C-CGG"
compare_results "example3_rrbs_se"

# Example 4: RRBS PE 150bp 10x
echo ""
echo "======================================"
echo "Example 4: RRBS PE 150bp 10x"
echo "======================================"
run_bsmap_cpp "example4_rrbs_pe" "rrbs" "ex4_pe150_10x_1.fastq" "ex4_pe150_10x_2.fastq" 12 "-D C-CGG"
run_bsmap_rs "example4_rrbs_pe" "rrbs" "ex4_pe150_10x_1.fastq" "ex4_pe150_10x_2.fastq" 12 "-D C-CGG"
compare_results "example4_rrbs_pe"

# Example 5: WGBS PE 150bp 20x
echo ""
echo "======================================"
echo "Example 5: WGBS PE 150bp 20x (133,334 pairs)"
echo "======================================"
run_bsmap_cpp "example5_wgbs_pe_20x" "wgbs" "ex5_pe150_20x_1.fastq" "ex5_pe150_20x_2.fastq" 16 ""
run_bsmap_rs "example5_wgbs_pe_20x" "wgbs" "ex5_pe150_20x_1.fastq" "ex5_pe150_20x_2.fastq" 16 ""
compare_results "example5_wgbs_pe_20x"

# Example 6: RRBS PE 150bp 20x
echo ""
echo "======================================"
echo "Example 6: RRBS PE 150bp 20x"
echo "======================================"
run_bsmap_cpp "example6_rrbs_pe_20x" "rrbs" "ex6_pe150_20x_1.fastq" "ex6_pe150_20x_2.fastq" 12 "-D C-CGG"
run_bsmap_rs "example6_rrbs_pe_20x" "rrbs" "ex6_pe150_20x_1.fastq" "ex6_pe150_20x_2.fastq" 12 "-D C-CGG"
compare_results "example6_rrbs_pe_20x"

# ======================================
# 汇总结果
# ======================================
echo ""
echo "=========================================="
echo "汇总测试结果"
echo "=========================================="

cat > results/summary.csv << 'CSV_HEADER'
example,tool,mode,time_wall_sec,time_user_sec,time_sys_sec,mem_max_rss_kb,aligned_reads,total_reads,aligned_pct
CSV_HEADER

# 从日志文件中提取数据
extract_stats() {
    local EXAMPLE=$1
    local TOOL=$2
    local MODE=$3
    local LOG_FILE="results/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "wall clock" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}')
    local USER=$(grep "user" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local SYS=$(grep "sys" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}')
    
    local ALIGNED=""
    local TOTAL=""
    if [ "$TOOL" = "bsmap" ]; then
        TOTAL=$(grep "total reads:" "$LOG_FILE" | awk '{print $NF}' || echo "")
        ALIGNED=$(grep "aligned reads:" "$LOG_FILE" | awk '{print $3}' || echo "")
    else
        TOTAL=$(grep "total reads" "$LOG_FILE" || echo "")
        ALIGNED=$(grep "aligned" "$LOG_FILE" || echo "")
    fi
    
    echo "$EXAMPLE,$TOOL,$MODE,$WALL,$USER,$SYS,$RSS,$ALIGNED,$TOTAL,"
}

echo "提取统计数据..."
extract_stats "example1_wgbs_se" "bsmap" "wgbs" >> results/summary.csv
extract_stats "example1_wgbs_se" "bsmaprs" "wgbs" >> results/summary.csv
extract_stats "example2_wgbs_pe" "bsmap" "wgbs" >> results/summary.csv
extract_stats "example2_wgbs_pe" "bsmaprs" "wgbs" >> results/summary.csv
extract_stats "example3_rrbs_se" "bsmap" "rrbs" >> results/summary.csv
extract_stats "example3_rrbs_se" "bsmaprs" "rrbs" >> results/summary.csv
extract_stats "example4_rrbs_pe" "bsmap" "rrbs" >> results/summary.csv
extract_stats "example4_rrbs_pe" "bsmaprs" "rrbs" >> results/summary.csv
extract_stats "example5_wgbs_pe_20x" "bsmap" "wgbs" >> results/summary.csv
extract_stats "example5_wgbs_pe_20x" "bsmaprs" "wgbs" >> results/summary.csv
extract_stats "example6_rrbs_pe_20x" "bsmap" "rrbs" >> results/summary.csv
extract_stats "example6_rrbs_pe_20x" "bsmaprs" "rrbs" >> results/summary.csv

echo ""
echo "=== 测试结果汇总 (summary.csv) ==="
cat results/summary.csv

# 生成最终报告
cat > report/final_report.md << 'REPORT'
# BSMAP vs BSMAP-rs 基准测试最终报告

## 测试环境
- Docker 内存限制：20GB
- 预编译和预建索引：是
- 统计范围：仅比对环节

## 测试执行日期
$(date)

## 目录
- [Example 1: WGBS SE 75bp 10x](#example-1)
- [Example 2: WGBS PE 150bp 10x](#example-2)
- [Example 3: RRBS SE 75bp 10x](#example-3)
- [Example 4: RRBS PE 150bp 10x](#example-4)
- [Example 5: WGBS PE 150bp 20x](#example-5)
- [Example 6: RRBS PE 150bp 20x](#example-6)
- [总结](#总结)

REPORT

cat results/summary.csv >> report/final_report.md

echo ""
echo "=========================================="
echo "✅ 阶段2：比对测试完成！"
echo "=========================================="
date
