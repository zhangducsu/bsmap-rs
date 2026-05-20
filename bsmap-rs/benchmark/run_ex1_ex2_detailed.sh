#!/bin/bash
# ==========================================
# 仅运行 Example 1 和 Example 2 的基准测试
# Ex1: WGBS SE 75bp 10x
# Ex2: WGBS PE 150bp 10x
# 
# 改进版：精确分离 索引加载时间 vs 纯比对时间
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试"
echo "  (精确分离 索引加载/纯比对 时间)"
echo "=========================================="
echo "  运行环境: Docker 20GB内存"
echo "  测试内容: Ex1 (WGBS SE), Ex2 (WGBS PE)"
echo "  功能: 比对测试 + SAM对比 + 报告"
echo "=========================================="
date
echo ""

# ======================================
# 步骤1：清理旧结果并准备
# ======================================
echo ">>> 步骤1：清理旧结果并准备..."
rm -rf results/*
mkdir -p results

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
    
    echo "  [$EXAMPLE] BSMAP C++..."
    local RESULT_DIR="results/${EXAMPLE}_bsmap"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmap.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmap.log
    else
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
    
    echo "  [$EXAMPLE] bsmap-rs..."
    local RESULT_DIR="results/${EXAMPLE}_bsmaprs"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    else
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s $SEED -v 0.08 -I 4 -p 1 $EXTRA \
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

mkdir -p results/comparison_example1_wgbs_se
mkdir -p results/comparison_example2_wgbs_pe

if [ -f compare_sam.py ]; then
    echo "  运行详细对比..."
    python3 compare_sam.py \
        results/example1_wgbs_se_bsmap/bsmap.sam \
        results/example1_wgbs_se_bsmaprs/bsmaprs.sam \
        results/comparison_example1_wgbs_se \
        "example1_wgbs_se"
    
    python3 compare_sam.py \
        results/example2_wgbs_pe_bsmap/bsmap.sam \
        results/example2_wgbs_pe_bsmaprs/bsmaprs.sam \
        results/comparison_example2_wgbs_pe \
        "example2_wgbs_pe"
else
    echo "  compare_sam.py 不存在，使用简单对比..."
    # 简单版对比
    grep -v "^@" results/example1_wgbs_se_bsmap/bsmap.sam | sort > results/example1_wgbs_se_bsmap/sam1_sorted.sam
    grep -v "^@" results/example1_wgbs_se_bsmaprs/bsmaprs.sam | sort > results/example1_wgbs_se_bsmaprs/sam2_sorted.sam
    echo "Example1 SAM对比" > results/comparison_example1_wgbs_se/simple_report.txt
    echo "BSMAP C++比对数: $(wc -l results/example1_wgbs_se_bsmap/sam1_sorted.sam)" >> results/comparison_example1_wgbs_se/simple_report.txt
    echo "bsmap-rs比对数: $(wc -l results/example1_wgbs_se_bsmaprs/sam2_sorted.sam)" >> results/comparison_example1_wgbs_se/simple_report.txt
fi

# ======================================
# 步骤7：提取详细时间统计（分离索引和比对时间）
# ======================================
echo ""
echo "======================================"
echo "步骤7：提取详细时间统计"
echo "  (分离 索引加载 / 纯比对 时间)"
echo "======================================"

# 解析 bsmap-rs 的详细时间
extract_bsmaprs_detailed_stats() {
    local EXAMPLE=$1
    local TOOL="bsmaprs"
    local MODE=$2
    local LOG_FILE="results/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    echo "  提取 [$EXAMPLE] 详细统计..."
    
    # 从 /usr/bin/time 中提取完整数据
    local WALL=$(grep "Elapsed (wall clock)" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}' || echo "0")
    local USER=$(grep "User time" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local SYS=$(grep "System time" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}' || echo "0")
    
    # 从 log 时间戳中提取详细阶段时间
    local T1=""
    local T2=""
    local T3=""
    
    # 找到 "索引已从缓存加载" 的时间
    T1=$(grep "从缓存加载索引" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//' || echo "")
    if [ -z "$T1" ]; then
        # 尝试找 "加载索引" 的开始时间
        T1=$(grep "加载参考序列" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//' || echo "")
    fi
    
    # 找到 "开始单端比对" / "开始双端比对" 的时间
    T2=$(grep "开始单端比对\|开始双端比对" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//' || echo "")
    
    # 找到 "单端比对完成" / "双端比对完成" 的时间
    T3=$(grep "单端比对完成\|双端比对完成" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//' || echo "")
    
    # 计算时间差
    local TIME_INDEX="0.00"
    local TIME_ALIGN="0.00"
    
    if [ -n "$T1" ] && [ -n "$T2" ]; then
        # 解析 ISO 时间戳到秒级
        python3 -c "
import datetime
import sys
try:
    t1 = datetime.datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00'))
    t2 = datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00'))
    delta = (t2 - t1).total_seconds()
    print('%.2f' % delta)
except:
    print('0.00')
" "$T1" "$T2" > /tmp/index_time.tmp
        TIME_INDEX=$(cat /tmp/index_time.tmp)
        rm -f /tmp/index_time.tmp
    fi
    
    if [ -n "$T2" ] && [ -n "$T3" ]; then
        python3 -c "
import datetime
import sys
try:
    t2 = datetime.datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00'))
    t3 = datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00'))
    delta = (t3 - t2).total_seconds()
    print('%.2f' % delta)
except:
    print('0.00')
" "$T2" "$T3" > /tmp/align_time.tmp
        TIME_ALIGN=$(cat /tmp/align_time.tmp)
        rm -f /tmp/align_time.tmp
    fi
    
    echo "$EXAMPLE,$TOOL,$MODE,$WALL,$USER,$SYS,$RSS,$TIME_INDEX,$TIME_ALIGN"
}

# 解析 BSMAP C++ 时间（简化版，没有详细阶段信息）
extract_bsmap_cpp_stats() {
    local EXAMPLE=$1
    local TOOL="bsmap"
    local MODE=$2
    local LOG_FILE="results/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "Elapsed (wall clock)" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}' || echo "0")
    local USER=$(grep "User time" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local SYS=$(grep "System time" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}' || echo "0")
    
    # 对于 C++，暂时假设所有时间都是比对时间（需要进一步分析）
    echo "$EXAMPLE,$TOOL,$MODE,$WALL,$USER,$SYS,$RSS,0.00,$WALL"
}

cat > results/summary_detailed.csv << 'CSV_HEADER'
example,tool,mode,total_wall_sec,user_sec,sys_sec,mem_max_rss_kb,index_load_sec,align_only_sec
CSV_HEADER

extract_bsmap_cpp_stats "example1_wgbs_se" "wgbs" >> results/summary_detailed.csv
extract_bsmaprs_detailed_stats "example1_wgbs_se" "wgbs" >> results/summary_detailed.csv
extract_bsmap_cpp_stats "example2_wgbs_pe" "wgbs" >> results/summary_detailed.csv
extract_bsmaprs_detailed_stats "example2_wgbs_pe" "wgbs" >> results/summary_detailed.csv

echo ""
echo "=== 详细时间统计 (summary_detailed.csv) ==="
cat results/summary_detailed.csv

# 生成原始的 summary.csv 保持兼容性
cat > results/summary.csv << 'CSV_HEADER_OLD'
example,tool,mode,time_wall_sec,time_user_sec,time_sys_sec,mem_max_rss_kb
CSV_HEADER_OLD

python3 -c '
import csv
with open("results/summary_detailed.csv", "r") as f1, open("results/summary.csv", "a") as f2:
    reader = csv.DictReader(f1)
    for row in reader:
        out_row = [
            row["example"],
            row["tool"],
            row["mode"],
            row["total_wall_sec"],
            row["user_sec"],
            row["sys_sec"],
            row["mem_max_rss_kb"]
        ]
        f2.write(",".join(out_row) + "\n")
'

# ======================================
# 步骤8：生成最终报告
# ======================================
echo ""
echo "======================================"
echo "步骤8：生成最终报告"
echo "======================================"

python3 - << 'PYTHON'
import csv
from datetime import datetime

# 读取详细数据
data = []
with open("results/summary_detailed.csv", "r") as f:
    reader = csv.DictReader(f)
    for row in reader:
        data.append(row)

report = []
report.append("# BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试报告")
report.append("")
report.append("## 测试环境")
report.append("- Docker 内存限制：20GB")
report.append("- 预编译和预建索引：是")
report.append("- 统计方式：分离 索引加载时间 和 纯比对时间")
report.append(f"- 测试日期：{datetime.now()}")
report.append("")
report.append("## 测试内容")
report.append("1. **Example 1**: WGBS SE 75bp 10x")
report.append("2. **Example 2**: WGBS PE 150bp 10x")
report.append("")
report.append("## 性能对比 (重点看 align_only_sec)")
report.append("")
report.append("### Example 1: WGBS SE 75bp 10x")

for row in data:
    if row["example"] == "example1_wgbs_se":
        report.append(f"- **{row['tool']}**:")
        report.append(f"  - 总耗时: {row['total_wall_sec']}s")
        report.append(f"  - 索引加载: {row['index_load_sec']}s")
        report.append(f"  - **纯比对**: {row['align_only_sec']}s")
        report.append(f"  - 内存峰值: {row['mem_max_rss_kb']} KB")

report.append("")
report.append("### Example 2: WGBS PE 150bp 10x")

for row in data:
    if row["example"] == "example2_wgbs_pe":
        report.append(f"- **{row['tool']}**:")
        report.append(f"  - 总耗时: {row['total_wall_sec']}s")
        report.append(f"  - 索引加载: {row['index_load_sec']}s")
        report.append(f"  - **纯比对**: {row['align_only_sec']}s")
        report.append(f"  - 内存峰值: {row['mem_max_rss_kb']} KB")

report.append("")
report.append("## SAM一致性对比")
report.append("- Example 1: [详细报告](comparison_example1_wgbs_se/detailed_report.txt)")
report.append("- Example 2: [详细报告](comparison_example2_wgbs_pe/detailed_report.txt)")
report.append("")
report.append("## 详细数据 (summary_detailed.csv)")

with open("results/final_report.md", "w") as f:
    f.write("\n".join(report) + "\n")

PYTHON

date > results/run_date.txt

echo ""
echo "=========================================="
echo "✅ Ex1 & Ex2 基准测试完成！"
echo "=========================================="
echo "结果目录：results/"
echo "详细时间统计：results/summary_detailed.csv"
echo "最终报告：results/final_report.md"
echo "=========================================="
date
