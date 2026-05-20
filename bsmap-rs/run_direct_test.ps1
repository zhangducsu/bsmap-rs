# P-series test - non-interactive version
$WORK_DIR = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs"

Write-Host "========================================"
Write-Host "P-series Optimization Test (Direct)"
Write-Host "========================================"
Write-Host ""

# Check Docker
Write-Host "[Check] Docker..."
$dockerCheck = docker --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "[OK] $dockerCheck" -ForegroundColor Green
} else {
    Write-Host "[Error] Docker not working" -ForegroundColor Red
    exit 1
}

# Prepare and run in one command
Write-Host ""
Write-Host "[Execute] Running tests..."
Write-Host ""

$dockerCmd = @"
apt-get update >/dev/null 2>&1 && apt-get install -y build-essential curl wget git python3 python3-pip time >/dev/null 2>&1 && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1 && export PATH=/root/.cargo/bin:\$PATH && rustup default stable >/dev/null 2>&1 && chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh && cd /workspace/bsmap-rs && echo 'Building...' && cargo build --release 2>&1 && echo 'Running tests...' && cargo test --package bsmap 2>&1 && cd benchmark && ./run_ex1_ex2.sh 2>&1
"@

docker run --rm `
    -v "$WORK_DIR`:/workspace/bsmap-rs" `
    -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" `
    -w /workspace/bsmap-rs `
    --memory=20g `
    --cpus=4 `
    ubuntu:22.04 bash -c $dockerCmd 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\test_output.log"

Write-Host ""
Write-Host "========================================"
Write-Host "Test execution finished"
Write-Host "========================================"
Write-Host ""
Write-Host "Check: benchmark/results/test_output.log"
Write-Host ""
