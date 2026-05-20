# P-series complete test with environment setup
$WORK_DIR = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs"

Write-Host "========================================"
Write-Host "P-series Optimization Complete Test"
Write-Host "========================================"
Write-Host ""

# Create results directory
New-Item -ItemType Directory -Force -Path "$WORK_DIR\benchmark\results" | Out-Null

# Step 1: Setup environment
Write-Host "[1/5] Setting up build environment..."
$setupScript = @"
apt-get update
apt-get install -y build-essential curl wget git python3 python3-pip time
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. /root/.cargo/env
rustup default stable
chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh
chmod +x /workspace/bsmap-rs/test_inside_container.sh
echo 'Environment ready'
"@

$setupScript | Out-File -FilePath "$WORK_DIR\setup_env.sh" -Encoding ASCII
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash /workspace/bsmap-rs/setup_env.sh 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\setup.log"

# Step 2: Build
Write-Host ""
Write-Host "[2/5] Building bsmap-rs..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c ". /root/.cargo/env && cargo build --release" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\build.log"

# Step 3: Unit tests
Write-Host ""
Write-Host "[3/5] Running unit tests..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c ". /root/.cargo/env && cargo test --package bsmap" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\tests.log"

# Step 4: Benchmark
Write-Host ""
Write-Host "[4/5] Running benchmark tests..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs/benchmark --memory=20g --cpus=4 ubuntu:22.04 bash -c ". /root/.cargo/env && ./run_ex1_ex2.sh" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\benchmark.log"

# Step 5: Report
Write-Host ""
Write-Host "[5/5] Generating test report..."
$report = @"
# P-series Optimization Test Report

**Test Date**: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')

## Test Content
1. Build: bsmap-rs (release mode)
2. Unit Tests: Verify code correctness
3. Benchmark: Ex1 (WGBS SE 75bp 10x), Ex2 (WGBS PE 150bp 10x)

## Test Logs
- Setup: benchmark/results/setup.log
- Build: benchmark/results/build.log
- Unit Tests: benchmark/results/tests.log
- Benchmark: benchmark/results/benchmark.log

## Results
See benchmark/results/ directory for detailed data.
"@
$report | Out-File -FilePath "$WORK_DIR\benchmark\results\P_SERIES_TEST_REPORT.md" -Encoding UTF8

Write-Host ""
Write-Host "========================================"
Write-Host "P-series Optimization Test Completed!"
Write-Host "========================================"
Write-Host ""
Write-Host "Generated files:"
Get-ChildItem "$WORK_DIR\benchmark\results" | Select-Object Name, Length | Format-Table -AutoSize
Write-Host ""
Write-Host "Test report: benchmark/results/P_SERIES_TEST_REPORT.md"
Write-Host ""
