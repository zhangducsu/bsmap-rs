#!/bin/bash
# ==========================================
# 测试 Mmap 模式修复
# ==========================================
set -e

# 自动检测工作目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 设置 bsmap-rs 的基准目录
BSMAP_RS_DIR="$(dirname "$SCRIPT_DIR")"
WORK_DIR="$BSMAP_RS_DIR"

echo "=========================================="
echo "Mmap 模式修复验证测试"
echo "日期: $(date)"
echo "=========================================="
echo

# ======================================
# 1. 准备数据
# ======================================
echo "> 步骤1: 准备数据"
mkdir -p tmp results

# 准备测试数据
if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    echo "  - 解压测试数据"
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq 2>/dev/null || true
fi

# ======================================
# 2. 构建 V3 索引
# ======================================
echo
echo "> 步骤2: 构建 V3 格式索引"

# 删除旧索引（从头开始）
echo "  - 删除旧索引"
rm -f data/chr22_tail_1M.fa.bsi

# 构建 bsmap-rs 索引 (V3 格式)
echo "  - 构建 V3 格式索引"
"$BSMAP_RS_DIR/target/release/bsmap" index \
  -d data/chr22_tail_1M.fa \
  -s 16 \
  2>&1 | tee results/mmap_fix_index_build.log

# 检查索引是否真的生成了
if [ -f data/chr22_tail_1M.fa.bsi ]; then
    echo "  ✓ V3 索引文件生成成功: data/chr22_tail_1M.fa.bsi"
    ls -lh data/chr22_tail_1M.fa.bsi
else
    echo "  ✗ V3 索引文件未生成！"
    exit 1
fi

# ======================================
# 3. 测试 Mmap 模式
# ======================================
echo
echo "> 步骤3: 测试 Mmap 模式"

# 运行比对，使用 Mmap 模式 (main.rs 中设置为 Mmap)
echo "  - 运行 Mmap 模式比对"
start_time=$(date +%s.%N)
timeout 300 "$BSMAP_RS_DIR/target/release/bsmap" align \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o results/mmap_test_output.sam \
  -s 16 -v 0.08 -I 4 -p 1 -v 2 \
  2>&1 | tee results/mmap_fix_test.log
end_time=$(date +%s.%N)
test_time=$(echo "$end_time - $start_time" | bc)

# 检查是否成功运行
if [ $? -eq 0 ]; then
    echo "  ✓ Mmap 模式比对成功！"
else
    echo "  ✗ Mmap 模式比对失败！"
    echo
    echo "错误日志:"
    tail -50 results/mmap_fix_test.log
    exit 1
fi

# ======================================
# 4. 分析结果
# ======================================
echo
echo "> 步骤4: 分析结果"

# 提取日志中的关键信息
echo
echo "--- 索引加载信息 ---"
grep -i "index\|加载\|v3" results/mmap_fix_test.log || echo "(未找到索引加载日志)"

echo
echo "--- 比对统计 ---"
grep -i "读段\|比对\|reads\|aligned" results/mmap_fix_test.log || echo "(未找到比对统计)"

echo
echo "--- 最终 SAM 文件 ---"
if [ -f results/mmap_test_output.sam ]; then
    sam_count=$(grep -v "^@" results/mmap_test_output.sam | wc -l)
    echo "比对记录数: $sam_count"
    ls -lh results/mmap_test_output.sam
else
    echo "SAM 文件未生成"
fi

# ======================================
# 5. 生成报告
# ======================================
echo
echo "> 步骤5: 生成报告"

python3 << 'PYTHON'
import re
from datetime import datetime

# 读取日志
with open('results/mmap_fix_test.log', 'r', errors='ignore') as f:
    test_log = f.read()

with open('results/mmap_fix_index_build.log', 'r', errors='ignore') as f:
    build_log = f.read()

# 写报告
with open('results/mmap_fix_report.md', 'w') as f:
    f.write(f"# Mmap 模式修复验证报告\n\n")
    f.write(f"**日期**: {datetime.now()}\n\n")
    f.write("## 测试摘要\n\n")
    
    success = "✓ 测试通过" if "索引已从" in test_log and "开始单端比对" in test_log else "✗ 测试失败"
    f.write(f"- **状态**: {success}\n\n")
    
    f.write("## 索引构建\n\n")
    f.write("```\n")
    f.write(build_log)
    f.write("\n```\n\n")
    
    f.write("## 测试日志\n\n")
    f.write("```\n")
    f.write(test_log)
    f.write("\n```\n\n")

print("✓ 报告生成完成")
PYTHON

echo
echo "=========================================="
echo "✅ Mmap 模式修复测试完成！"
echo "=========================================="
echo "查看 results/mmap_fix_report.md 获取详细报告"
echo
ls -lh results/
