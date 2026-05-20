#!/bin/bash
# ==========================================
# BSMAP vs BSMAP-rs 纯比对性能测试
# 
# 测试方案：
# 1. 预构建参考索引（只做一次）
# 2. 仅测试纯比对时间（不包含索引构建/加载）
# 3. 多次运行取平均，减少波动
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP vs BSMAP-rs 纯比对性能测试"
echo "  (仅测试比对，不包含索引构建)"
echo "=========================================="
date
echo ""

# ======================================
# 测试配置
# ======================================
NUM_RUNS=3  # 每次测试运行3次取平均

# 测试命令
BSMAP_CPP_EX1="/workspace/bsmap-original/bsmap-2.90/bsmap -a tmp/ex1_se75_10x.fastq -d data/chr22_tail_1M.fa -o results/ex1_cpp.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_CPP_EX2="/workspace/bsmap-original/bsmap-2.90/bsmap -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq -d data/chr22_tail_1M.fa -o results/ex2_cpp.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_RS_EX1="/workspace/bsmap-rs/target/release/bsmap align -a tmp/ex1_se75_10x.fastq -d data/chr22_tail_1M.fa -o results/ex1_rs.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_RS_EX2="/workspace/bsmap-rs/target/release/bsmap align -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq -d data/chr22_tail_1M.fa -o results/ex2_rs.sam -s 16 -v 0.08 -I 4 -p 1"

# ======================================
# 步骤1：准备数据
# ======================================
echo ">>> 步骤1：准备测试数据..."
mkdir -p tmp results

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
# 步骤2：构建索引（只做一次）
# ======================================
echo ""
echo ">>> 步骤2：构建参考索引..."

# BSMAP C++ - 构建索引
echo "  [BSMAP C++] 构建索引..."
/workspace/bsmap-original/bsmap-2.90/bsmap \
    -d data/chr22_tail_1M.fa \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/cpp_index_build.log

# bsmap-rs - 构建索引（使用 index 子命令）
echo "  [bsmap-rs] 构建索引..."
/workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa \
    -s 16 \
    2>&1 | tee results/rs_index_build.log

echo "  索引构建完成！"

# ======================================
# 步骤3：运行纯比对测试（多次取平均）
# ======================================
echo ""
echo ">>> 步骤3：运行纯比对性能测试（运行 $NUM_RUNS 次取平均）..."

run_benchmark() {
    local NAME=$1
    local CMD=$2
    local RUNS=$3
    
    echo "  [$NAME] 运行 $RUNS 次取平均..."
    
    local TOTAL_TIME=0
    local TOTAL_MEM=0
    
    for i in $(seq 1 $RUNS); do
        echo "    第 $i/$RUNS 次..."
        
        # 运行并提取时间和内存
        local OUTPUT=$(/usr/bin/time -v bash -c "$CMD" 2>&1)
        
        # 提取 Elapsed 时间
        local WALL=$(echo "$OUTPUT" | grep "Elapsed (wall clock)" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}')
        
        # 提取内存
        local MEM=$(echo "$OUTPUT" | grep "Maximum resident" | awk '{print $NF}')
        
        TOTAL_TIME=$(python3 -c "print($TOTAL_TIME + $WALL)")
        TOTAL_MEM=$(python3 -c "print($TOTAL_MEM + $MEM)")
        
        echo "      时间: ${WALL}s, 内存: ${MEM} KB"
    done
    
    # 计算平均值
    local AVG_TIME=$(python3 -c "print('%.2f' % ($TOTAL_TIME / $RUNS))")
    local AVG_MEM=$(python3 -c "print(int($TOTAL_MEM / $RUNS))")
    
    echo "$NAME,$AVG_TIME,$AVG_MEM"
}

# 运行测试
echo ""
echo "=== Example 1 (WGBS SE 75bp 10x) ==="
EX1_CPP=$(run_benchmark "ex1_bsmap_cpp" "$BSMAP_CPP_EX1" $NUM_RUNS)
EX1_RS=$(run_benchmark "ex1_bsmaprs" "$BSMAP_RS_EX1" $NUM_RUNS)

echo ""
echo "=== Example 2 (WGBS PE 150bp 10x) ==="
EX2_CPP=$(run_benchmark "ex2_bsmap_cpp" "$BSMAP_CPP_EX2" $NUM_RUNS)
EX2_RS=$(run_benchmark "ex2_bsmaprs" "$BSMAP_RS_EX2" $NUM_RUNS)

# ======================================
# 步骤4：SAM 对比
# ======================================
echo ""
echo ">>> 步骤4：SAM 对比..."

if [ -f compare_sam.py ]; then
    echo "  Ex1 SAM 对比..."
    mkdir -p results/comparison_ex1
    python3 compare_sam.py results/ex1_cpp.sam results/ex1_rs.sam results/comparison_ex1 "ex1_wgbs_se"
    
    echo "  Ex2 SAM 对比..."
    mkdir -p results/comparison_ex2
    python3 compare_sam.py results/ex2_cpp.sam results/ex2_rs.sam results/comparison_ex2 "ex2_wgbs_pe"
fi

# ======================================
# 步骤5：生成报告
# ======================================
echo ""
echo ">>> 步骤5：生成测试报告..."

cat > results/summary.csv << 'HEADER'
example,tool,avg_time_sec,avg_mem_kb
HEADER

echo "$EX1_CPP" >> results/summary.csv
echo "$EX1_RS" >> results/summary.csv
echo "$EX2_CPP" >> results/summary.csv
echo "$EX2_RS" >> results/summary.csv

echo ""
echo "=== 测试结果汇总 ==="
cat results/summary.csv

# 生成详细报告
python3 - << 'PYTHON'
import csv
from datetime import datetime

data = []
with open("results/summary.csv", "r") as f:
    reader = csv.DictReader(f)
    for row in reader:
        data.append(row)

report = []
report.append("# BSMAP vs BSMAP-rs 纯比对性能测试报告")
report.append("")
report.append(f"**测试日期**: {datetime.now()}")
report.append(f"**测试方案**: 仅测试纯比对时间，索引已预构建")
report.append(f"**运行次数**: 3次取平均")
report.append("")

report.append("## 测试命令和参数")
report.append("")
report.append("### BSMAP C++")
report.append("")
report.append("**索引构建:**")
report.append("```bash")
report.append("/workspace/bsmap-original/bsmap-2.90/bsmap \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("**Example 1 (WGBS SE 75bp 10x) 比对:**")
report.append("```bash")
report.append("/workspace/bsmap-original/bsmap-2.90/bsmap \\")
report.append("  -a tmp/ex1_se75_10x.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/ex1_cpp.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("**Example 2 (WGBS PE 150bp 10x) 比对:**")
report.append("```bash")
report.append("/workspace/bsmap-original/bsmap-2.90/bsmap \\")
report.append("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/ex2_cpp.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("### bsmap-rs")
report.append("")
report.append("**索引构建:**")
report.append("```bash")
report.append("/workspace/bsmap-rs/target/release/bsmap index \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -s 16")
report.append("```")
report.append("")
report.append("**Example 1 (WGBS SE 75bp 10x) 比对:**")
report.append("```bash")
report.append("/workspace/bsmap-rs/target/release/bsmap align \\")
report.append("  -a tmp/ex1_se75_10x.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/ex1_rs.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("**Example 2 (WGBS PE 150bp 10x) 比对:**")
report.append("```bash")
report.append("/workspace/bsmap-rs/target/release/bsmap align \\")
report.append("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/ex2_rs.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("### 参数说明")
report.append("| 参数 | 值 | 说明 |")
report.append("|------|-----|------|")
report.append("| -a | 文件路径 | 查询序列文件（单端）或 Read 1（双端） |")
report.append("| -b | 文件路径 | Read 2 文件（双端模式） |")
report.append("| -d | 文件路径 | 参考序列文件 |")
report.append("| -o | 文件路径 | 输出 SAM 文件 |")
report.append("| -s | 16 | 种子长度 |")
report.append("| -v | 0.08 | 允许的最大错配率（8%） |")
report.append("| -I | 4 | 允许的最大插入/删除长度 |")
report.append("| -p | 1 | 线程数 |")
report.append("")

report.append("## 纯比对性能对比")
report.append("")

for row in data:
    if "ex1" in row["example"]:
        report.append(f"### Example 1: {row['example']}")
        report.append("")
        if "bsmap_cpp" in row["example"]:
            report.append(f"**BSMAP C++**: {row['avg_time_sec']}s, 内存: {int(row['avg_mem_kb']):,} KB")
        else:
            report.append(f"**bsmap-rs**: {row['avg_time_sec']}s, 内存: {int(row['avg_mem_kb']):,} KB")
        report.append("")

for row in data:
    if "ex2" in row["example"]:
        report.append(f"### Example 2: {row['example']}")
        report.append("")
        if "bsmap_cpp" in row["example"]:
            report.append(f"**BSMAP C++**: {row['avg_time_sec']}s, 内存: {int(row['avg_mem_kb']):,} KB")
        else:
            report.append(f"**bsmap-rs**: {row['avg_time_sec']}s, 内存: {int(row['avg_mem_kb']):,} KB")
        report.append("")

report.append("## SAM 一致性")
report.append("- [Example 1 详细报告](comparison_ex1/detailed_report.txt)")
report.append("- [Example 2 详细报告](comparison_ex2/detailed_report.txt)")

with open("results/final_report.md", "w") as f:
    f.write("\n".join(report) + "\n")

PYTHON

echo ""
echo "=========================================="
echo "✅ 纯比对性能测试完成！"
echo "=========================================="
date
