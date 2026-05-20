#!/bin/bash
# ==============================================================================
# P系列优化完整性能测试与对比脚本
# 内容: 编译+单元测试+Ex1/Ex2基准测试+SAM对比+报告生成
# ==============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
cd "$PROJECT_ROOT"

echo "=========================================="
echo "P系列优化完整性能测试"
echo "=========================================="
date
echo ""

# 检查Docker是否可用
if ! command -v docker &> /dev/null; then
    echo "错误: 未找到Docker，请先安装Docker"
    exit 1
fi

# 给测试脚本赋予执行权限
chmod +x "$SCRIPT_DIR/run_ex1_ex2.sh"
chmod +x "$SCRIPT_DIR/run_simple_test.sh"

# 创建结果目录
mkdir -p "$SCRIPT_DIR/results"
rm -rf "$SCRIPT_DIR/results/*"

echo ""
echo "=========================================="
echo "[1/4] 构建Docker镜像"
echo "=========================================="
docker build -t bsmap-rs-complete-test "$PROJECT_ROOT"
echo "[1/4] 完成 ✓"
echo ""

echo ""
echo "=========================================="
echo "[2/4] 运行完整测试"
echo "=========================================="

# 运行Docker容器进行测试
docker run --rm -it \
    -v "$PROJECT_ROOT:/workspace/bsmap-rs" \
    -v "$PROJECT_ROOT/../bsmap-original:/workspace/bsmap-original" \
    -w /workspace/bsmap-rs \
    --memory=20g \
    --cpus=4 \
    bsmap-rs-complete-test \
    bash -c "
        set -e

        echo ''
        echo '========================================'
        echo '  阶段1: 编译bsmap-rs (release模式)'
        echo '========================================'
        cargo build --release
        echo ''

        echo '========================================'
        echo '  阶段2: 运行单元测试'
        echo '========================================'
        cargo test --package bsmap 2>&1 | tee /workspace/bsmap-rs/benchmark/results/unit_tests.log
        echo ''

        echo '========================================'
        echo '  阶段3: 运行Ex1/Ex2基准测试'
        echo '========================================'
        cd /workspace/bsmap-rs/benchmark
        ./run_ex1_ex2.sh
        echo ''

        echo '========================================'
        echo '  阶段4: 生成最终总结报告'
        echo '========================================'
        
        # 复制单元测试结果
        if [ -f /workspace/bsmap-rs/benchmark/results/unit_tests.log ]; then
            cp /workspace/bsmap-rs/benchmark/results/unit_tests.log /workspace/bsmap-rs/benchmark/results/tests.log
        fi

        # 创建最终的合并报告
        cat > /workspace/bsmap-rs/benchmark/results/P_SERIES_FINAL_REPORT.md << 'EOF'
# P系列优化最终测试报告
## 测试日期: $(date)
## 测试环境: Docker (20GB内存)

## 1. 单元测试结果
[单元测试日志](tests.log)

## 2. 性能对比
[性能汇总](summary.csv)
[最终报告](final_report.md)

## 3. SAM一致性
- Example 1: [详细报告](comparison_example1_wgbs_se/detailed_report.txt)
- Example 2: [详细报告](comparison_example2_wgbs_pe/detailed_report.txt)
EOF

        echo ''
        echo '========================================'
        echo '  完成！'
        echo '========================================'
        ls -lh /workspace/bsmap-rs/benchmark/results/
    "

echo ""
echo "[2/4] 完成 ✓"
echo ""

echo ""
echo "=========================================="
echo "[3/4] 显示结果摘要"
echo "=========================================="
if [ -f "$SCRIPT_DIR/results/summary.csv" ]; then
    echo "=== 性能测试结果 ==="
    cat "$SCRIPT_DIR/results/summary.csv"
    echo ""
fi

if [ -f "$SCRIPT_DIR/results/unit_tests.log" ]; then
    echo "=== 单元测试摘要 ==="
    tail -20 "$SCRIPT_DIR/results/unit_tests.log"
    echo ""
fi

echo "[3/4] 完成 ✓"
echo ""

echo ""
echo "=========================================="
echo "[4/4] 所有测试完成！"
echo "=========================================="
echo "生成的文件:"
echo "  - $SCRIPT_DIR/results/summary.csv (性能数据)"
echo "  - $SCRIPT_DIR/results/final_report.md (最终报告)"
echo "  - $SCRIPT_DIR/results/tests.log (单元测试)"
echo "  - $SCRIPT_DIR/results/comparison_example1_wgbs_se/ (Ex1对比)"
echo "  - $SCRIPT_DIR/results/comparison_example2_wgbs_pe/ (Ex2对比)"
echo "  - $SCRIPT_DIR/results/P_SERIES_FINAL_REPORT.md (完整报告)"
echo ""
echo "测试结果已保存在 benchmark/results/ 目录下！"
echo ""
date
