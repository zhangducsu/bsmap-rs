# P系列优化完整测试 - PowerShell脚本
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "P系列优化完整测试" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$WORK_DIR = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs"
Set-Location $WORK_DIR

# 创建结果目录
New-Item -ItemType Directory -Force -Path "$WORK_DIR\benchmark\results" | Out-Null

# 检查Docker
Write-Host "[检查] Docker状态..." -ForegroundColor Yellow
$dockerVersion = docker --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] $dockerVersion" -ForegroundColor Green
} else {
    Write-Host "[错误] Docker未运行" -ForegroundColor Red
    exit 1
}

# 准备环境
Write-Host ""
Write-Host "[准备] 安装依赖..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    apt-get update >/dev/null 2>&1
    apt-get install -y build-essential curl wget git python3 python3-pip time >/dev/null 2>&1
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1
    export PATH=/root/.cargo/bin:\$PATH
    rustup default stable >/dev/null 2>&1
    chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh
    echo '环境准备完成'
"

Write-Host ""
Write-Host "[1/4] 编译 bsmap-rs (release)..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    export PATH=/root/.cargo/bin:\$PATH
    cargo build --release 2>&1 | tee /workspace/bsmap-rs/benchmark/results/build.log
    echo '编译完成'
"

Write-Host ""
Write-Host "[2/4] 运行单元测试..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    export PATH=/root/.cargo/bin:\$PATH
    cargo test --package bsmap 2>&1 | tee /workspace/bsmap-rs/benchmark/results/tests.log
    echo '单元测试完成'
"

Write-Host ""
Write-Host "[3/4] 运行 Ex1/Ex2 基准测试..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    cd /workspace/bsmap-rs/benchmark
    ./run_ex1_ex2.sh 2>&1 | tee /workspace/bsmap-rs/benchmark/results/benchmark.log
    echo '基准测试完成'
"

Write-Host ""
Write-Host "[4/4] 生成测试报告..." -ForegroundColor Yellow
$testDate = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$reportContent = @"
# P系列优化测试报告

## 测试日期: $testDate

## 测试内容
- 编译: bsmap-rs (release模式)
- 单元测试: 验证代码正确性
- 基准测试: Ex1 (WGBS SE 75bp 10x), Ex2 (WGBS PE 150bp 10x)

## 性能对比
请查看 `summary.csv` 获取详细数据。

## SAM一致性
- Ex1: `comparison_example1_wgbs_se/detailed_report.txt`
- Ex2: `comparison_example2_wgbs_pe/detailed_report.txt`

## 测试日志
- 构建日志: `build.log`
- 单元测试: `tests.log`
- 基准测试: `benchmark.log`
"@

$reportContent | Out-File -FilePath "$WORK_DIR\benchmark\results\P_SERIES_TEST_REPORT.md" -Encoding UTF8

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "✅ P系列优化测试完成！" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "生成的文件:" -ForegroundColor White
Get-ChildItem "$WORK_DIR\benchmark\results" | Select-Object Name, Length | Format-Table -AutoSize
Write-Host ""
