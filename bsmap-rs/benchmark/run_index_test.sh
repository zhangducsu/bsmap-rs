#!/bin/bash
# ==========================================
# 索引加载性能测试 - V3 格式优化验证
# ==========================================
set -e

echo "=========================================="
echo "索引加载性能测试 (V3 格式)"
echo "日期: $(date)"
echo "=========================================="
echo ""

# 切换到 benchmark 目录
cd /workspace/bsmap-benchmark/benchmark
BSMAP_RS_DIR="/workspace/bsmap-benchmark/bsmap-rs"

mkdir -p tmp results

# ======================================
# 步骤1: 准备测试数据
# ======================================
echo "> 步骤1: 准备测试数据"

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    echo "  - 解压测试数据..."
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq 2>/dev/null || true
fi

# ======================================
# 步骤2: 删除旧索引，构建新索引
# ======================================
echo ""
echo "> 步骤2: 构建 V3 格式索引"

echo "  - 删除旧索引"
rm -f data/chr22_tail_1M.fa.bsi

echo "  - 构建 bsmap-rs 索引 (V3 格式)"
start_build=$(date +%s.%N)
"$BSMAP_RS_DIR/target/release/bsmap" index \
  -d data/chr22_tail_1M.fa \
  -s 16 \
  2>&1 | tee results/index_build_v3.log
end_build=$(date +%s.%N)
build_time=$(echo "$end_build - $start_build" | bc)

if [ -f data/chr22_tail_1M.fa.bsi ]; then
    echo "  ✓ V3 索引构建成功"
    ls -lh data/chr22_tail_1M.fa.bsi
    echo "  - 构建耗时: $build_time 秒"
else
    echo "  ✗ 索引构建失败！"
    exit 1
fi

# ======================================
# 步骤3: 测试索引加载性能
# ======================================
echo ""
echo "> 步骤3: 测试索引加载性能"

echo "  - 运行比对测试（记录索引加载时间）..."
start_time=$(date +%s.%N)

"$BSMAP_RS_DIR/target/release/bsmap" align \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o /dev/null \
  -s 16 -v 0.08 -I 4 -p 1 -v \
  2>&1 | tee results/index_load_test.log

end_time=$(date +%s.%N)
total_time=$(echo "$end_time - $start_time" | bc)

echo ""
echo "  - 完整比对耗时: $total_time 秒"

# ======================================
# 步骤4: 分析日志，提取关键信息
# ======================================
echo ""
echo "> 步骤4: 分析结果"

echo ""
echo "--- 索引相关日志 ---"
grep -i "index\|加载\|k-mer\|kmer" results/index_load_test.log || true

# ======================================
# 步骤5: 生成报告
# ======================================
echo ""
echo "> 步骤5: 生成报告"

cat > results/index_loading_report.md << EOF
# V3 索引格式加载性能测试报告

**日期**: $(date)

## 测试配置
- 测试工具: bsmap-rs (V3 格式)
- 参考基因组: chr22_tail_1M.fa
- 测试数据: ex1_se75_10x (simulated WGBS)

## 测试结果

### 索引构建
\`\`\`
$(cat results/index_build_v3.log 2>/dev/null || echo "构建日志不可用")
\`\`\`

### 关键指标
| 指标 | 值 |
|------|-----|
| 索引构建耗时 | ${build_time}s |
| 完整比对耗时 | ${total_time}s |

### 索引加载日志
\`\`\`
$(grep -i "index\|加载\|k-mer\|kmer" results/index_load_test.log 2>/dev/null || echo "无匹配日志")
\`\`\`
EOF

echo ""
echo "=========================================="
echo "✅ 索引加载测试完成！"
echo "=========================================="
echo ""
echo "报告位置: results/index_loading_report.md"
echo "详细日志: results/index_load_test.log"
echo ""
ls -lh results/
