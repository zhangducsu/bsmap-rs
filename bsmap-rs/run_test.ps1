# P-series optimization complete test - PowerShell script
$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "P-series Optimization Complete Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$WORK_DIR = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs"
Set-Location $WORK_DIR

# Create results directory
New-Item -ItemType Directory -Force -Path "$WORK_DIR\benchmark\results" | Out-Null

# Check Docker
Write-Host "[Check] Docker status..." -ForegroundColor Yellow
$dockerVersion = docker --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] $dockerVersion" -ForegroundColor Green
} else {
    Write-Host "[Error] Docker not running" -ForegroundColor Red
    exit 1
}

# Prepare environment
Write-Host ""
Write-Host "[Prepare] Installing dependencies..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    apt-get update >/dev/null 2>&1
    apt-get install -y build-essential curl wget git python3 python3-pip time >/dev/null 2>&1
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1
    export PATH=/root/.cargo/bin:\$PATH
    rustup default stable >/dev/null 2>&1
    chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh
    echo 'Environment ready'
"

Write-Host ""
Write-Host "[1/4] Build bsmap-rs (release)..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    export PATH=/root/.cargo/bin:\$PATH
    cargo build --release 2>&1 | tee /workspace/bsmap-rs/benchmark/results/build.log
    echo 'Build completed'
"

Write-Host ""
Write-Host "[2/4] Run unit tests..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    export PATH=/root/.cargo/bin:\$PATH
    cargo test --package bsmap 2>&1 | tee /workspace/bsmap-rs/benchmark/results/tests.log
    echo 'Unit tests completed'
"

Write-Host ""
Write-Host "[3/4] Run Ex1/Ex2 benchmark tests..." -ForegroundColor Yellow
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "
    cd /workspace/bsmap-rs/benchmark
    ./run_ex1_ex2.sh 2>&1 | tee /workspace/bsmap-rs/benchmark/results/benchmark.log
    echo 'Benchmark completed'
"

Write-Host ""
Write-Host "[4/4] Generate test report..." -ForegroundColor Yellow
$testDate = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$reportContent = @"
# P-series Optimization Test Report

## Test Date: $testDate

## Test Content
- Build: bsmap-rs (release mode)
- Unit Tests: Verify code correctness
- Benchmark: Ex1 (WGBS SE 75bp 10x), Ex2 (WGBS PE 150bp 10x)

## Performance Comparison
See 'summary.csv' for detailed data.

## SAM Consistency
- Ex1: 'comparison_example1_wgbs_se/detailed_report.txt'
- Ex2: 'comparison_example2_wgbs_pe/detailed_report.txt'

## Test Logs
- Build log: 'build.log'
- Unit tests: 'tests.log'
- Benchmark: 'benchmark.log'
"@

$reportContent | Out-File -FilePath "$WORK_DIR\benchmark\results\P_SERIES_TEST_REPORT.md" -Encoding UTF8

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "P-series Optimization Test Completed!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Generated files:" -ForegroundColor White
Get-ChildItem "$WORK_DIR\benchmark\results" | Select-Object Name, Length | Format-Table -AutoSize
Write-Host ""
