@echo off
REM ==========================================================================
REM 4线程基准测试启动脚本
REM ==========================================================================
cd /d "%~dp0"

echo ========================================
echo BSMAP vs BSMAP-rs 4线程基准测试
echo ========================================
echo.

REM 检查Docker
docker --version >nul 2>&1
if %errorlevel% neq 0 (
    echo 错误: 未检测到Docker
    echo 请先安装并启动 Docker Desktop
    pause
    exit /b 1
)

REM 给Linux脚本权限
echo 正在给测试脚本赋予执行权限...
docker run --rm -v "%~dp0:/workspace/bsmap-rs" ubuntu chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2_4threads.sh

REM 运行测试
echo.
echo ========================================
echo 开始运行4线程基准测试...
echo 注意: 这可能需要较长时间
echo ========================================
echo.

REM 运行测试
docker run --rm -it ^
  -v "%~dp0:/workspace/bsmap-rs" ^
  -v "%~dp0..\bsmap-original:/workspace/bsmap-original" ^
  -w /workspace/bsmap-rs ^
  --memory=20g ^
  --cpus=4 ^
  --name=bsmap-rs-test-4threads ^
  ubuntu:22.04 bash -c "
    set -e
    
    echo ''
    echo '准备测试环境...'
    
    # 安装依赖
    apt-get update >/dev/null 2>&1
    apt-get install -y build-essential curl wget git python3 python3-pip time >/dev/null 2>&1
    
    # 安装Rust
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1
    export PATH=/root/.cargo/bin:$PATH
    rustup default stable >/dev/null 2>&1
    
    echo ''
    echo '========================================'
    echo '1. 编译 bsmap-rs (release模式 + AVX512支持)'
    echo '========================================'
    cd /workspace/bsmap-rs
    RUSTFLAGS='-C target-cpu=native' cargo build --release
    
    echo ''
    echo '========================================'
    echo '2. 运行 Ex1/Ex2 4线程基准测试'
    echo '========================================'
    cd benchmark
    ./run_ex1_ex2_4threads.sh
    
    echo ''
    echo '========================================'
    echo '✅ 4线程基准测试完成！'
    echo '========================================'
    ls -lh results_4threads/
"

if %errorlevel% neq 0 (
    echo.
    echo 测试过程中可能出现了错误
    pause
    exit /b %errorlevel%
)

echo.
echo ========================================
echo 测试完成！
echo 请查看 benchmark\results_4threads\ 目录获取结果
echo ========================================
echo.

REM 列出结果
if exist "%~dp0benchmark\results_4threads" (
    dir /b "%~dp0benchmark\results_4threads"
)

echo.
pause
