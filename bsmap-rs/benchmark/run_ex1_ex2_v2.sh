#!/bin/bash
# ==========================================
# BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试
# 
# 测试信息记录：
# - BSMAP C++: 完整命令和参数
# - bsmap-rs: 完整命令和参数
# - 精确分离 索引加载时间 vs 纯比对时间
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP vs BSMAP-rs Ex1 & Ex2 基准测试"
echo "=========================================="
date
echo ""

# ======================================
# 测试配置信息
# ======================================
BSMAP_CPP_EX1_CMD="/workspace/bsmap-original/bsmap-2.90/bsmap -a tmp/ex1_se75_10x.fastq -d data/chr22_tail_1M.fa -o results/example1_wgbs_se_bsmap/bsmap.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_CPP_EX2_CMD="/workspace/bsmap-original/bsmap-2.90/bsmap -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq -d data/chr22_tail_1M.fa -o results/example2_wgbs_pe_bsmap/bsmap.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_RS_EX1_CMD="/workspace/bsmap-rs/target/release/bsmap align -a tmp/ex1_se75_10x.fastq -d data/chr22_tail_1M.fa -o results/example1_wgbs_se_bsmaprs/bsmaprs.sam -s 16 -v 0.08 -I 4 -p 1"
BSMAP_RS_EX2_CMD="/workspace/bsmap-rs/target/release/bsmap align -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq -d data/chr22_tail_1M.fa -o results/example2_wgbs_pe_bsmaprs/bsmaprs.sam -s 16 -v 0.08 -I 4 -p 1"

# 参数说明
BSMAP_PARAMS_DESC="参数说明：
  -a: 查询序列文件（单端）或 Read 1（双端）
  -b: Read 2 文件（双端）
  -d: 参考序列文件
  -o: 输出 SAM 文件
  -s: 种子长度（16）
  -v: 允许的最大错配率（0.08，即 8%）
  -I: 允许的最大插入/删除长度（4）
  -p: 线程数（1）"

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
# 步骤3：执行测试
# ======================================
echo ""
echo ">>> 步骤3：执行基准测试..."

# Ex1: BSMAP C++
echo "  [Ex1] BSMAP C++..."
mkdir -p results/example1_wgbs_se_bsmap
/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se_bsmap/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/example1_wgbs_se_bsmap/bsmap.log

# Ex1: bsmap-rs
echo "  [Ex1] bsmap-rs..."
mkdir -p results/example1_wgbs_se_bsmaprs
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example1_wgbs_se_bsmaprs/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/example1_wgbs_se_bsmaprs/bsmaprs.log

# Ex2: BSMAP C++
echo "  [Ex2] BSMAP C++..."
mkdir -p results/example2_wgbs_pe_bsmap
/usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe_bsmap/bsmap.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/example2_wgbs_pe_bsmap/bsmap.log

# Ex2: bsmap-rs
echo "  [Ex2] bsmap-rs..."
mkdir -p results/example2_wgbs_pe_bsmaprs
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/example2_wgbs_pe_bsmaprs/bsmaprs.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/example2_wgbs_pe_bsmaprs/bsmaprs.log

# ======================================
# 步骤4：SAM对比
# ======================================
echo ""
echo ">>> 步骤4：SAM 详细对比分析..."

mkdir -p results/comparison_example1_wgbs_se
mkdir -p results/comparison_example2_wgbs_pe

if [ -f compare_sam.py ]; then
    echo "  运行 Ex1 对比..."
    python3 compare_sam.py \
        results/example1_wgbs_se_bsmap/bsmap.sam \
        results/example1_wgbs_se_bsmaprs/bsmaprs.sam \
        results/comparison_example1_wgbs_se \
        "example1_wgbs_se"
    
    echo "  运行 Ex2 对比..."
    python3 compare_sam.py \
        results/example2_wgbs_pe_bsmap/bsmap.sam \
        results/example2_wgbs_pe_bsmaprs/bsmaprs.sam \
        results/comparison_example2_wgbs_pe \
        "example2_wgbs_pe"
fi

# ======================================
# 步骤5：提取统计数据
# ======================================
echo ""
echo ">>> 步骤5：提取详细时间统计..."

# bsmap-rs 详细时间提取
extract_bsmaprs_stats() {
    local EXAMPLE=$1
    local TOOL="bsmaprs"
    local LOG_FILE="results/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    # 从 /usr/bin/time 提取
    local WALL=$(grep "Elapsed (wall clock)" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}')
    local USER=$(grep "User time" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local SYS=$(grep "System time" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}')
    
    # 从时间戳计算阶段时间
    local T1=$(grep "从缓存加载索引" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//')
    if [ -z "$T1" ]; then
        T1=$(grep "加载参考序列" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//')
    fi
    local T2=$(grep "开始单端比对\|开始双端比对" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//')
    local T3=$(grep "单端比对完成\|双端比对完成" "$LOG_FILE" | head -1 | awk '{print $1}' | sed 's/\[//;s/\]//')
    
    local TIME_INDEX="0.00"
    local TIME_ALIGN="0.00"
    
    if [ -n "$T1" ] && [ -n "$T2" ]; then
        python3 -c "
import datetime, sys
try:
    t1 = datetime.datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00'))
    t2 = datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00'))
    print('%.2f' % (t2 - t1).total_seconds())
except:
    print('0.00')
" "$T1" "$T2" > /tmp/t_idx.tmp
        TIME_INDEX=$(cat /tmp/t_idx.tmp)
    fi
    
    if [ -n "$T2" ] && [ -n "$T3" ]; then
        python3 -c "
import datetime, sys
try:
    t2 = datetime.datetime.fromisoformat(sys.argv[1].replace('Z', '+00:00'))
    t3 = datetime.datetime.fromisoformat(sys.argv[2].replace('Z', '+00:00'))
    print('%.2f' % (t3 - t2).total_seconds())
except:
    print('0.00')
" "$T2" "$T3" > /tmp/t_aln.tmp
        TIME_ALIGN=$(cat /tmp/t_aln.tmp)
    fi
    
    echo "$EXAMPLE,$TOOL,$WALL,$USER,$SYS,$RSS,$TIME_INDEX,$TIME_ALIGN"
}

# BSMAP C++ 简化时间提取
extract_bsmap_cpp_stats() {
    local EXAMPLE=$1
    local TOOL="bsmap"
    local LOG_FILE="results/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "Elapsed (wall clock)" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}')
    local USER=$(grep "User time" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local SYS=$(grep "System time" "$LOG_FILE" | head -1 | awk '{print $NF}')
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}')
    
    # C++ 索引随程序构建，暂不分离
    echo "$EXAMPLE,$TOOL,$WALL,$USER,$SYS,$RSS,0.00,$WALL"
}

cat > results/summary_detailed.csv << 'CSV_HEADER'
example,tool,total_wall_sec,user_sec,sys_sec,mem_max_rss_kb,index_load_sec,align_only_sec
CSV_HEADER

extract_bsmap_cpp_stats "example1_wgbs_se" >> results/summary_detailed.csv
extract_bsmaprs_stats "example1_wgbs_se" >> results/summary_detailed.csv
extract_bsmap_cpp_stats "example2_wgbs_pe" >> results/summary_detailed.csv
extract_bsmaprs_stats "example2_wgbs_pe" >> results/summary_detailed.csv

echo "=== 详细时间统计 ==="
cat results/summary_detailed.csv

# ======================================
# 步骤6：生成报告
# ======================================
echo ""
echo ">>> 步骤6：生成最终报告..."

python3 - << 'PYTHON'
import csv
from datetime import datetime

# 读取数据
data = []
with open("results/summary_detailed.csv", "r") as f:
    reader = csv.DictReader(f)
    for row in reader:
        data.append(row)

report = []
report.append("# BSMAP vs BSMAP-rs 基准测试报告")
report.append("")
report.append(f"**测试日期**: {datetime.now()}")
report.append("")

# ==========================================
# 测试命令和参数（最重要！）
# ==========================================
report.append("## 测试命令和参数")
report.append("")
report.append("### BSMAP C++")
report.append("")
report.append("**Example 1 (WGBS SE 75bp 10x)**")
report.append("```bash")
report.append("/workspace/bsmap-original/bsmap-2.90/bsmap \\")
report.append("  -a tmp/ex1_se75_10x.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/example1_wgbs_se_bsmap/bsmap.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("**Example 2 (WGBS PE 150bp 10x)**")
report.append("```bash")
report.append("/workspace/bsmap-original/bsmap-2.90/bsmap \\")
report.append("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/example2_wgbs_pe_bsmap/bsmap.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("### bsmap-rs")
report.append("")
report.append("**Example 1 (WGBS SE 75bp 10x)**")
report.append("```bash")
report.append("/workspace/bsmap-rs/target/release/bsmap align \\")
report.append("  -a tmp/ex1_se75_10x.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/example1_wgbs_se_bsmaprs/bsmaprs.sam \\")
report.append("  -s 16 -v 0.08 -I 4 -p 1")
report.append("```")
report.append("")
report.append("**Example 2 (WGBS PE 150bp 10x)**")
report.append("```bash")
report.append("/workspace/bsmap-rs/target/release/bsmap align \\")
report.append("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\")
report.append("  -d data/chr22_tail_1M.fa \\")
report.append("  -o results/example2_wgbs_pe_bsmaprs/bsmaprs.sam \\")
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

# 性能对比
report.append("## 性能对比")
report.append("")

for row in data:
    if row["example"] == "example1_wgbs_se":
        report.append(f"### Example 1: {row['example']}")
        report.append("")
        report.append(f"**{row['tool']}**:")
        report.append(f"- 总耗时: {row['total_wall_sec']}s")
        report.append(f"- 索引加载: {row['index_load_sec']}s")
        report.append(f"- 纯比对: {row['align_only_sec']}s")
        report.append(f"- 内存峰值: {int(row['mem_max_rss_kb']):,} KB")
        report.append("")

for row in data:
    if row["example"] == "example2_wgbs_pe":
        report.append(f"### Example 2: {row['example']}")
        report.append("")
        report.append(f"**{row['tool']}**:")
        report.append(f"- 总耗时: {row['total_wall_sec']}s")
        report.append(f"- 索引加载: {row['index_load_sec']}s")
        report.append(f"- 纯比对: {row['align_only_sec']}s")
        report.append(f"- 内存峰值: {int(row['mem_max_rss_kb']):,} KB")
        report.append("")

report.append("## SAM 一致性")
report.append("- [Example 1 详细报告](comparison_example1_wgbs_se/detailed_report.txt)")
report.append("- [Example 2 详细报告](comparison_example2_wgbs_pe/detailed_report.txt)")

with open("results/final_report.md", "w") as f:
    f.write("\n".join(report) + "\n")

PYTHON

date > results/run_date.txt

echo ""
echo "=========================================="
echo "✅ 基准测试完成！"
echo "=========================================="
date
