#!/bin/bash
# ==========================================
# 对比原版C++ BSMAP与修复后的Rust BSMAP
# 重点测试Mmap模式的索引加载时长
# ==========================================
set -e

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "Mmap模式索引加载时长对比测试"
echo "日期: $(date)"
echo "=========================================="
echo

# ======================================
# 1. 准备数据
# ======================================
echo "> 步骤1: 准备数据"
mkdir -p tmp results

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    echo "  - 解压测试数据"
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
fi

# ======================================
# 2. 构建索引
# ======================================
echo
echo "> 步骤2: 构建索引"

# 2.1 删除旧索引
echo "  - 删除旧索引"
rm -f data/chr22_tail_1M.fa.bsi

# 2.2 构建 Rust BSMAP 索引 (V3格式)
echo "  - 构建 bsmap-rs V3 索引"
start_time=$(date +%s)
/workspace/bsmap-rs/target/release/bsmap index \
    -d data/chr22_tail_1M.fa \
    -s 16 \
    2>&1 | tee results/index_build_rs.log
end_time=$(date +%s)
index_build_time=$((end_time - start_time))

# 2.3 检查索引生成
if [ -f data/chr22_tail_1M.fa.bsi ]; then
    echo "  ✓ bsmap-rs 索引构建成功: $(ls -lh data/chr22_tail_1M.fa.bsi)"
else
    echo "  ✗ bsmap-rs 索引构建失败!"
    exit 1
fi

# ======================================
# 3. 测试原版C++ BSMAP
# ======================================
echo
echo "> 步骤3: 测试原版C++ BSMAP"

echo "  - 运行 C++ BSMAP (测试索引加载)"
start_time=$(date +%s)
time /usr/bin/time -v \
  /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/cpp_align.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/cpp_test.log
end_time=$(date +%s)
cpp_total_time=$((end_time - start_time))

# ======================================
# 4. 测试修复后的Rust BSMAP (Mmap模式)
# ======================================
echo
echo "> 步骤4: 测试修复后的Rust BSMAP (Mmap模式)"

echo "  - 运行 bsmap-rs (Mmap模式)"
start_time=$(date +%s)
time /usr/bin/time -v \
  /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/rs_mmap_align.sam \
    -s 16 -v 0.08 -I 4 -p 1 --verbose 2 \
    2>&1 | tee results/rs_mmap_test.log
end_time=$(date +%s)
rs_total_time=$((end_time - start_time))

# ======================================
# 5. 解析结果并生成报告
# ======================================
echo
echo "> 步骤5: 解析结果并生成报告"

python3 << 'PYTHON'
import re
import csv
from datetime import datetime

def extract_time_and_mem(log_file):
    time_sec = None
    mem_kb = None
    with open(log_file, 'r', errors='ignore') as f:
        content = f.read()
        
    time_match = re.search(r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\):\s*(\d+):([\d.]+)', content)
    mem_match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', content)
    
    if time_match:
        mins = float(time_match.group(1))
        secs = float(time_match.group(2))
        time_sec = mins * 60 + secs
    
    if mem_match:
        mem_kb = int(mem_match.group(1))
    
    return time_sec, mem_kb, content

def extract_index_load_time(log_content, tool_name):
    """从日志中提取索引加载相关时间信息"""
    index_load_info = ""
    
    # 查找索引加载相关日志
    if tool_name == "rs":
        # Rust版本的日志
        index_match = re.search(r'索引已从.*加载.*\((.*?)\)', log_content)
        if index_match:
            index_load_info = index_match.group(1)
        
        # 查找Mmap相关信息
        mmap_match = re.search(r'v3, mmap', log_content)
        if mmap_match:
            index_load_info += " (v3, mmap)"
    
    elif tool_name == "cpp":
        # C++版本通常没有明确的索引加载日志，我们标记一下
        index_load_info = "C++ BSMAP native format"
    
    return index_load_info

# 读取日志
cpp_time, cpp_mem, cpp_log = extract_time_and_mem('results/cpp_test.log')
rs_time, rs_mem, rs_log = extract_time_and_mem('results/rs_mmap_test.log')

# 提取索引加载信息
cpp_index_info = extract_index_load_time(cpp_log, "cpp")
rs_index_info = extract_index_load_time(rs_log, "rs")

# 写summary.csv
with open('results/index_load_summary.csv', 'w', newline='') as f:
    writer = csv.writer(f)
    writer.writerow(['tool', 'total_time_sec', 'memory_kb', 'index_info'])
    writer.writerow(['bsmap_cpp', cpp_time, cpp_mem, cpp_index_info])
    writer.writerow(['bsmap_rs_mmap', rs_time, rs_mem, rs_index_info])

# 写详细报告
with open('results/index_load_comparison_report.md', 'w') as f:
    f.write(f"# Mmap模式索引加载时长对比报告\n\n")
    f.write(f"**日期**: {datetime.now()}\n\n")
    
    f.write("## 测试摘要\n\n")
    
    f.write("| 工具 | 总时长 (秒) | 峰值内存 (KB) | 索引信息 |\n")
    f.write("|------|-----------|-------------|---------|\n")
    f.write(f"| 原版C++ BSMAP | {cpp_time:.2f} | {cpp_mem:,} | {cpp_index_info} |\n")
    f.write(f"| 修复后Rust BSMAP (Mmap) | {rs_time:.2f} | {rs_mem:,} | {rs_index_info} |\n")
    f.write("\n")
    
    # 计算改进
    if cpp_time and rs_time:
        improvement = ((cpp_time - rs_time) / cpp_time * 100)
        f.write("### 性能对比\n\n")
        if improvement > 0:
            f.write(f"✓ **改进**: {improvement:.1f}% (比原版快)\n\n")
        elif improvement < 0:
            f.write(f"✗ **性能下降**: {-improvement:.1f}% (比原版慢)\n\n")
        else:
            f.write("= 性能持平\n\n")
    
    f.write("## 详细日志\n\n")
    f.write("### 原版C++ BSMAP日志\n\n")
    f.write("```\n")
    f.write(cpp_log[:5000])  # 取前5000字符
    if len(cpp_log) > 5000:
        f.write("\n... (日志截断)\n")
    f.write("\n```\n\n")
    
    f.write("### 修复后Rust BSMAP (Mmap) 日志\n\n")
    f.write("```\n")
    f.write(rs_log[:5000])  # 取前5000字符
    if len(rs_log) > 5000:
        f.write("\n... (日志截断)\n")
    f.write("\n```\n\n")

print("✓ 报告生成完成")
PYTHON

echo
echo "=========================================="
echo "✅ 索引加载对比测试完成!"
echo "=========================================="
echo "报告文件:"
echo "  - results/index_load_summary.csv"
echo "  - results/index_load_comparison_report.md"
echo
ls -lh results/
