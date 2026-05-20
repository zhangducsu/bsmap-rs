#!/bin/bash
# ==========================================
# 测试索引加载性能
# 专门用于测试 V3 格式的加载优化
# ==========================================
set -e

# 自动检测工作目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 设置 bsmap-rs 的基准目录
BSMAP_RS_DIR="$(dirname "$SCRIPT_DIR")"
WORK_DIR="$BSMAP_RS_DIR"

echo "=========================================="
echo "索引加载性能测试"
echo "日期: $(date)"
echo "=========================================="
echo

# ======================================
# 1. 准备数据
# ======================================
echo "> 步骤1: 准备数据"
mkdir -p tmp results

# ======================================
# 2. 构建 V3 索引
# ======================================
echo
echo "> 步骤2: 构建 V3 格式索引"

# 2.1 删除旧索引（从头开始）
echo "  - 删除旧索引"
rm -f data/chr22_tail_1M.fa.bsi

# 2.2 构建 bsmap-rs 索引 (V3 格式)
echo "  - 构建 V3 格式索引"
start_time=$(date +%s.%N)
"$BSMAP_RS_DIR/target/release/bsmap" index \
  -d data/chr22_tail_1M.fa \
  -s 16 \
  2>&1 | tee results/index_build_v3.log
end_time=$(date +%s.%N)
build_time=$(echo "$end_time - $start_time" | bc)

# 2.3 检查索引是否真的生成了
if [ -f data/chr22_tail_1M.fa.bsi ]; then
    echo "  ✓ V3 索引文件生成成功: data/chr22_tail_1M.fa.bsi"
    ls -lh data/chr22_tail_1M.fa.bsi
else
    echo "  ✗ V3 索引文件未生成！"
    exit 1
fi

# ======================================
# 3. 测试索引加载性能
# ======================================
echo
echo "> 步骤3: 测试索引加载性能"

# 创建一个简单的测试程序，只加载索引而不进行比对
cat > /tmp/test_index_load.rs << 'RUST'
use std::path::Path;
use std::time::Instant;

fn main() {
    let index_path = Path::new("/workspace/bsmap-rs/benchmark/data/chr22_tail_1M.fa.bsi");
    println!("测试索引加载性能...");
    println!("索引路径: {:?}", index_path);
    
    // 测试多次加载取平均值
    let num_tests = 5;
    let mut total_time = 0.0;
    
    for i in 0..num_tests {
        println!("测试 {} / {}", i + 1, num_tests);
        let start = Instant::now();
        
        // 加载索引
        let result = bsmap::reference::load_index_with_mode(
            index_path,
            bsmap::reference::LoadMode::Memory,
        );
        
        let duration = start.elapsed();
        let secs = duration.as_secs_f64();
        total_time += secs;
        println!("  加载时间: {:.6} 秒", secs);
        
        match result {
            Ok((_, _, meta)) => {
                println!("  ✓ 索引加载成功");
                println!("  - 种子长度: {}", meta.seed_size);
                println!("  - 总 k-mers: {}", meta.total_kmers);
            }
            Err(e) => {
                println!("  ✗ 索引加载失败: {}", e);
            }
        }
    }
    
    let avg_time = total_time / num_tests as f64;
    println!("\n==========================================");
    println!("平均加载时间: {:.6} 秒", avg_time);
    println!("==========================================");
}
RUST

# 使用现有的二进制通过调试方式测试（通过实际的比对命令，但只关注索引加载阶段）
echo
echo "--- 测试1: 使用实际比对命令测试索引加载 ---"

# 运行一次完整比对，记录索引加载时间
time "$BSMAP_RS_DIR/target/release/bsmap" align \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o /dev/null \
  -s 16 -v 0.08 -I 4 -p 1 -v \
  2>&1 | tee results/index_load_test.log

# ======================================
# 4. 生成报告
# ======================================
echo
echo "> 步骤4: 生成报告"

python3 << 'PYTHON'
import re
import csv
from datetime import datetime

def extract_index_load_time(log_file):
    """从日志中提取索引加载时间"""
    with open(log_file, 'r', errors='ignore') as f:
        content = f.read()
    
    # 查找索引加载相关的日志
    index_load_match = re.search(r'索引已从.*加载.*\((.*?)\)', content)
    if index_load_match:
        return index_load_match.group(1)
    return None

# 读取日志
index_info = extract_index_load_time('results/index_load_test.log')
build_log = ''
with open('results/index_build_v3.log', 'r', errors='ignore') as f:
    build_log = f.read()

# 写报告
with open('results/index_loading_report.md', 'w') as f:
    f.write(f"# V3 索引格式加载性能测试报告\n\n")
    f.write(f"**日期**: {datetime.now()}\n\n")
    f.write("## 测试结果\n\n")
    f.write("### 索引构建\n\n")
    f.write("```\n")
    f.write(build_log)
    f.write("\n```\n\n")
    f.write("### 索引加载\n\n")
    if index_info:
        f.write(f"索引加载信息: {index_info}\n\n")
    f.write("详细日志见: index_load_test.log\n")

print("✓ 报告生成完成")
PYTHON

echo
echo "=========================================="
echo "✅ 索引加载性能测试完成！"
echo "=========================================="
echo "查看 results/index_loading_report.md 获取详细报告"
ls -lh results/
