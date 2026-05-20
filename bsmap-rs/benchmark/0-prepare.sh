#!/bin/bash
# ==========================================
# 阶段0：环境准备和预编译
# 不统计时间和内存，仅准备环境
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "阶段0：环境准备和预编译"
echo "=========================================="
date
echo ""

# 检查数据是否存在
echo "检查测试数据完整性..."
if [ ! -f "data/wgbs/ex1_se75_10x/simulated.fastq.gz" ]; then
    echo "❌ 测试数据不完整，请先检查"
    exit 1
fi

echo "✅ 测试数据检查完成"

# 预编译 bsmap-rs
echo ""
echo "开始预编译 bsmap-rs (release模式)..."
cd /workspace/bsmap-rs
if [ ! -f "target/release/bsmap" ]; then
    cargo build --release -p bsmap 2>&1
    echo "✅ bsmap-rs 编译完成"
else
    echo "✅ bsmap-rs 已编译，跳过"
fi

cd "$WORK_DIR"
echo "✅ 阶段0完成！"
date
