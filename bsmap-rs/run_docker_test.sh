#!/bin/bash
# ==========================================
# 在 Docker 中构建并测试 bsmap-rs
# ==========================================
set -e

# 设置工作目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=========================================="
echo "在 Docker 中构建并测试 bsmap-rs"
echo "=========================================="
echo

# ======================================
# 1. 构建 Docker 镜像
# ======================================
echo "> 步骤1: 构建 Docker 镜像"
docker build -t bsmap-rs-test .

# ======================================
# 2. 运行 Docker 容器进行测试
# ======================================
echo
echo "> 步骤2: 运行 Docker 容器进行测试"

# 确保测试脚本有执行权限
chmod +x benchmark/test_index_loading.sh
chmod +x benchmark/run_simple_test.sh

# 运行 Docker 容器
docker run --rm -it \
  -v "$SCRIPT_DIR:/workspace/bsmap-rs" \
  -v "$SCRIPT_DIR/../bsmap-original:/workspace/bsmap-original" \
  -w /workspace/bsmap-rs \
  bsmap-rs-test \
  bash -c "
    # 进入工作目录
    cd /workspace/bsmap-rs
    
    echo '=========================================='
    echo '1. 编译 bsmap-rs (release 模式)'
    echo '=========================================='
    cargo build --release
    
    echo
    echo '=========================================='
    echo '2. 运行索引加载性能测试'
    echo '=========================================='
    cd benchmark
    ./test_index_loading.sh
    
    echo
    echo '=========================================='
    echo '3. 运行完整的性能测试'
    echo '=========================================='
    ./run_simple_test.sh
    
    echo
    echo '=========================================='
    echo '✅ 所有测试完成！'
    echo '=========================================='
    ls -lh results/
  "
