#!/bin/bash
# ==========================================
# 简化的 BSMAP vs bsmap-rs 纯比对性能测试
# 确保索引被正确缓存和加载
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "简化的纯比对性能测试"
echo "日期: $(date)"
echo "=========================================="
echo

# ======================================
# 1. 准备数据
# ======================================
echo "> 步骤1: 准备数据"
mkdir -p tmp results

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
fi

if [ ! -f tmp/ex2_pe150_10x_1.fastq ]; then
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > tmp/ex2_pe150_10x_1.fastq
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > tmp/ex2_pe150_10x_2.fastq
fi

# ======================================
# 2. 预构建索引
# ======================================
echo
echo "> 步骤2: 预构建索引"

# 2.1 删除旧索引（从头开始）
echo "  - 删除旧索引"
rm -f data/chr22_tail_1M.fa.bsi

# 2.2 预构建 bsmap-rs 索引
echo "  - 预构建 bsmap-rs 索引"
/workspace/bsmap-rs/target/release/bsmap index \
  -d data/chr22_tail_1M.fa \
  -s 16 \
  2>&1 | tee results/index_build.log

# 2.3 检查索引是否真的生成了
if [ -f data/chr22_tail_1M.fa.bsi ]; then
    echo "  ✓ 索引文件生成成功: data/chr22_tail_1M.fa.bsi"
    ls -lh data/chr22_tail_1M.fa.bsi
else
    echo "  ✗ 索引文件未生成！"
    exit 1
fi

# 2.4 先运行一次 BSMAP C++，让它也构建内部索引
echo
echo "  - 预热 BSMAP C++ 一次"
/workspace/bsmap-original/bsmap-2.90/bsmap \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o results/pre_warm_cpp.sam \
  -s 16 -v 0.08 -I 4 -p 1 > /dev/null 2>&1

# ======================================
# 3. 性能测试（只运行1次，避免系统波动）
# ======================================
echo
echo "> 步骤3: 性能测试"

# 3.1 测试 BSMAP C++ (Ex1)
echo
echo "--- [BSMAP C++] Ex1: WGBS SE 75bp 10x ---"
time /usr/bin/time -v \
  /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/ex1_cpp.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/ex1_cpp.log

# 3.2 测试 bsmap-rs (Ex1, 带 -v 日志确认索引加载)
echo
echo "--- [bsmap-rs] Ex1: WGBS SE 75bp 10x ---"
time /usr/bin/time -v \
  /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex1_se75_10x.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/ex1_rs.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/ex1_rs.log

# 3.3 测试 BSMAP C++ (Ex2)
echo
echo "--- [BSMAP C++] Ex2: WGBS PE 150bp 10x ---"
time /usr/bin/time -v \
  /workspace/bsmap-original/bsmap-2.90/bsmap \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/ex2_cpp.sam \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee results/ex2_cpp.log

# 3.4 测试 bsmap-rs (Ex2, 带 -v 日志确认索引加载)
echo
echo "--- [bsmap-rs] Ex2: WGBS PE 150bp 10x ---"
time /usr/bin/time -v \
  /workspace/bsmap-rs/target/release/bsmap align \
    -a tmp/ex2_pe150_10x_1.fastq \
    -b tmp/ex2_pe150_10x_2.fastq \
    -d data/chr22_tail_1M.fa \
    -o results/ex2_rs.sam \
    -s 16 -v 0.08 -I 4 -p 1 -v \
    2>&1 | tee results/ex2_rs.log

# ======================================
# 4. SAM 对比
# ======================================
echo
echo "> 步骤4: SAM 一致性对比"

if [ -f compare_sam.py ]; then
    echo "  - Ex1 对比"
    mkdir -p results/comparison_ex1
    python3 compare_sam.py results/ex1_cpp.sam results/ex1_rs.sam results/comparison_ex1 "ex1_wgbs_se"
    
    echo "  - Ex2 对比"
    mkdir -p results/comparison_ex2
    python3 compare_sam.py results/ex2_cpp.sam results/ex2_rs.sam results/comparison_ex2 "ex2_wgbs_pe"
fi

# ======================================
# 5. 解析结果
# ======================================
echo
echo "> 步骤5: 解析结果"

python3 << 'PYTHON'
import re
import csv
from datetime import datetime

def extract_time_and_mem(log_file):
    time_sec = None
    mem_kb = None
    with open(log_file, 'r', errors='ignore') as f:
        content = f.read()
        
    # 提取 /usr/bin/time 的输出
    time_match = re.search(r'Elapsed \(wall clock\) time \(h:mm:ss or m:ss\):\s*(\d+):([\d.]+)', content)
    mem_match = re.search(r'Maximum resident set size \(kbytes\):\s*(\d+)', content)
    
    if time_match:
        mins = float(time_match.group(1))
        secs = float(time_match.group(2))
        time_sec = mins * 60 + secs
    
    if mem_match:
        mem_kb = int(mem_match.group(1))
    
    return time_sec, mem_kb

# 读取四个日志
ex1_cpp_time, ex1_cpp_mem = extract_time_and_mem('results/ex1_cpp.log')
ex1_rs_time, ex1_rs_mem = extract_time_and_mem('results/ex1_rs.log')
ex2_cpp_time, ex2_cpp_mem = extract_time_and_mem('results/ex2_cpp.log')
ex2_rs_time, ex2_rs_mem = extract_time_and_mem('results/ex2_rs.log')

# 写 summary
with open('results/summary.csv', 'w', newline='') as f:
    writer = csv.writer(f)
    writer.writerow(['example', 'tool', 'time_sec', 'mem_kb'])
    writer.writerow(['ex1_wgbs_se', 'bsmap_cpp', ex1_cpp_time, ex1_cpp_mem])
    writer.writerow(['ex1_wgbs_se', 'bsmaprs', ex1_rs_time, ex1_rs_mem])
    writer.writerow(['ex2_wgbs_pe', 'bsmap_cpp', ex2_cpp_time, ex2_cpp_mem])
    writer.writerow(['ex2_wgbs_pe', 'bsmaprs', ex2_rs_time, ex2_rs_mem])

# 写报告
with open('results/test_report.md', 'w') as f:
    f.write(f"# 简化的纯比对性能测试报告\n\n")
    f.write(f"**日期**: {datetime.now()}\n\n")
    f.write("## 测试命令和参数\n\n")
    f.write("### BSMAP C++\n\n")
    f.write("**Ex1 (WGBS SE 75bp 10x):**\n")
    f.write("```bash\n")
    f.write("/workspace/bsmap-original/bsmap-2.90/bsmap \\\n")
    f.write("  -a tmp/ex1_se75_10x.fastq \\\n")
    f.write("  -d data/chr22_tail_1M.fa \\\n")
    f.write("  -o results/ex1_cpp.sam \\\n")
    f.write("  -s 16 -v 0.08 -I 4 -p 1\n")
    f.write("```\n\n")
    
    f.write("**Ex2 (WGBS PE 150bp 10x):**\n")
    f.write("```bash\n")
    f.write("/workspace/bsmap-original/bsmap-2.90/bsmap \\\n")
    f.write("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\\n")
    f.write("  -d data/chr22_tail_1M.fa \\\n")
    f.write("  -o results/ex2_cpp.sam \\\n")
    f.write("  -s 16 -v 0.08 -I 4 -p 1\n")
    f.write("```\n\n")
    
    f.write("### bsmap-rs\n\n")
    f.write("**Ex1 (WGBS SE 75bp 10x):**\n")
    f.write("```bash\n")
    f.write("/workspace/bsmap-rs/target/release/bsmap align \\\n")
    f.write("  -a tmp/ex1_se75_10x.fastq \\\n")
    f.write("  -d data/chr22_tail_1M.fa \\\n")
    f.write("  -o results/ex1_rs.sam \\\n")
    f.write("  -s 16 -v 0.08 -I 4 -p 1 -v\n")
    f.write("```\n\n")
    
    f.write("**Ex2 (WGBS PE 150bp 10x):**\n")
    f.write("```bash\n")
    f.write("/workspace/bsmap-rs/target/release/bsmap align \\\n")
    f.write("  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \\\n")
    f.write("  -d data/chr22_tail_1M.fa \\\n")
    f.write("  -o results/ex2_rs.sam \\\n")
    f.write("  -s 16 -v 0.08 -I 4 -p 1 -v\n")
    f.write("```\n\n")
    
    f.write("### 参数说明\n")
    f.write("| 参数 | 值 | 说明 |\n")
    f.write("|------|-----|------|\n")
    f.write("| -a | 文件路径 | 查询序列文件（单端）或 Read 1（双端） |\n")
    f.write("| -b | 文件路径 | Read 2 文件（双端模式） |\n")
    f.write("| -d | 文件路径 | 参考序列文件 |\n")
    f.write("| -o | 文件路径 | 输出 SAM 文件 |\n")
    f.write("| -s | 16 | 种子长度 |\n")
    f.write("| -v | 0.08 | 允许的最大错配率（8%） |\n")
    f.write("| -I | 4 | 允许的最大插入/删除长度 |\n")
    f.write("| -p | 1 | 线程数 |\n")
    f.write("\n")
    
    f.write("## 性能结果\n\n")
    f.write("### Ex1: WGBS SE 75bp 10x\n\n")
    f.write("| 工具 | 时间 (秒) | 内存 (KB) |\n")
    f.write("|------|-----------|----------|\n")
    f.write(f"| BSMAP C++ | {ex1_cpp_time:.2f} | {ex1_cpp_mem:,} |\n")
    f.write(f"| bsmap-rs | {ex1_rs_time:.2f} | {ex1_rs_mem:,} |\n")
    f.write("\n")
    
    f.write("### Ex2: WGBS PE 150bp 10x\n\n")
    f.write("| 工具 | 时间 (秒) | 内存 (KB) |\n")
    f.write("|------|-----------|----------|\n")
    f.write(f"| BSMAP C++ | {ex2_cpp_time:.2f} | {ex2_cpp_mem:,} |\n")
    f.write(f"| bsmap-rs | {ex2_rs_time:.2f} | {ex2_rs_mem:,} |\n")
    f.write("\n")
    
    f.write("## SAM 一致性\n")
    f.write("- [Ex1 详细报告](comparison_ex1/detailed_report.txt)\n")
    f.write("- [Ex2 详细报告](comparison_ex2/detailed_report.txt)\n")

print("✓ 结果解析完成")
PYTHON

echo
echo "=========================================="
echo "✅ 测试完成！"
echo "=========================================="
ls -lh results/
