@echo off
REM ==========================================
REM 在 Windows 上启动 Docker 测试
REM ==========================================

echo ==========================================
echo 在 Docker 中构建并测试 bsmap-rs
echo ==========================================
echo.

REM 设置工作目录
cd /d "%~dp0"

REM 确保测试脚本有执行权限（在 Docker 中会处理）
echo 步骤1: 构建 Docker 镜像
docker build -t bsmap-rs-test .

echo.
echo 步骤2: 运行 Docker 容器进行测试
docker run --rm -it ^
  -v "%cd%:/workspace/bsmap-rs" ^
  -v "%cd%/../bsmap-original:/workspace/bsmap-original" ^
  -w /workspace/bsmap-rs ^
  bsmap-rs-test ^
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

echo.
echo 按任意键退出...
pause > nul
