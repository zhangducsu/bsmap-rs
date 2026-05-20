#!/bin/bash
# P系列优化完整测试脚本
set -e

WORK_DIR="/workspace/bsmap-rs"
cd "$WORK_DIR"

echo "=========================================="
echo "P系列优化完整测试"
echo "=========================================="
date
echo ""

# 1. 编译
echo "=========================================="
echo "[1/4] 编译 bsmap-rs (release)"
echo "=========================================="
cargo build --release 2>&1 | tee benchmark/results/build.log
echo ""

# 2. 单元测试
echo "=========================================="
echo "[2/4] 运行单元测试"
echo "=========================================="
cargo test --package bsmap 2>&1 | tee benchmark/results/tests.log
echo ""

# 3. 基准测试
echo "=========================================="
echo "[3/4] 运行 Ex1/Ex2 基准测试"
echo "=========================================="
chmod +x benchmark/run_ex1_ex2.sh
benchmark/run_ex1_ex2.sh
echo ""

# 4. 生成最终报告
echo "=========================================="
echo "[4/4] 生成最终报告"
echo "=========================================="
cat > benchmark/results/P_SERIES_TEST_REPORT.md << 'EOF'
# P系列优化测试报告
## 测试日期: $(date)

## 性能对比
EOF

if [ -f benchmark/results/summary.csv ]; then
    cat benchmark/results/summary.csv >> benchmark/results/P_SERIES_TEST_REPORT.md
fi

echo ""
echo "=========================================="
echo "✅ 所有测试完成！"
echo "=========================================="
ls -lh benchmark/results/
date
