# ==========================================================================
# 4线程基准测试启动脚本 (PowerShell)
# ==========================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ScriptDir

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "BSMAP vs BSMAP-rs 4线程基准测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 检查Docker
try {
    docker --version | Out-Null
} catch {
    Write-Host "错误: 未检测到Docker" -ForegroundColor Red
    Write-Host "请先安装并启动 Docker Desktop" -ForegroundColor Red
    Read-Host "按任意键退出"
    exit 1
}

# 给Linux脚本权限
Write-Host "正在给测试脚本赋予执行权限..." -ForegroundColor Yellow
docker run --rm -v "${ScriptDir}:/workspace/bsmap-rs" ubuntu chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2_4threads.sh

# 运行测试
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "开始运行4线程基准测试..." -ForegroundColor Cyan
Write-Host "注意: 这可能需要较长时间" -ForegroundColor Yellow
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# 构建Docker命令
$dockerCmd = @(
    "docker", "run", "--rm", "-it",
    "-v", "${ScriptDir}:/workspace/bsmap-rs",
    "-v", "${ScriptDir}/../bsmap-original:/workspace/bsmap-original",
    "-w", "/workspace/bsmap-rs",
    "--memory=20g",
    "--cpus=4",
    "--name=bsmap-rs-test-4threads",
    "ubuntu:22.04", "bash", "-c", @"
set -e

echo ''
echo '准备测试环境...'

# 安装依赖
apt-get update >/dev/null 2>&1
apt-get install -y build-essential curl wget git python3 python3-pip time >/dev/null 2>&1

# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1
export PATH=/root/.cargo/bin:\$PATH
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
"@
)

# 执行Docker命令
& $dockerCmd

if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "测试过程中可能出现了错误" -ForegroundColor Red
    Read-Host "按任意键退出"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "测试完成！" -ForegroundColor Green
Write-Host "请查看 benchmark\results_4threads\ 目录获取结果" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

# 列出结果
$ResultsDir = Join-Path $ScriptDir "benchmark\results_4threads"
if (Test-Path $ResultsDir) {
    Get-ChildItem $ResultsDir | Select-Object Name
}

Write-Host ""
Read-Host "按任意键退出"
