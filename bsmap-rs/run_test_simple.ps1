# P-series test runner
$WORK_DIR = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs"

Write-Host "========================================"
Write-Host "P-series Optimization Test Runner"
Write-Host "========================================"

# Step 1: Build
Write-Host ""
Write-Host "[1/4] Building bsmap-rs..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "export PATH=/root/.cargo/bin:\$PATH && cargo build --release" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\build.log"

# Step 2: Unit tests
Write-Host ""
Write-Host "[2/4] Running unit tests..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "export PATH=/root/.cargo/bin:\$PATH && cargo test --package bsmap" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\tests.log"

# Step 3: Benchmark
Write-Host ""
Write-Host "[3/4] Running benchmark tests..."
docker run --rm -v "$WORK_DIR`:/workspace/bsmap-rs" -v "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-original:/workspace/bsmap-original" -w /workspace/bsmap-rs --memory=20g --cpus=4 ubuntu:22.04 bash -c "chmod +x /workspace/bsmap-rs/benchmark/run_ex1_ex2.sh && cd /workspace/bsmap-rs/benchmark && ./run_ex1_ex2.sh" 2>&1 | Tee-Object -FilePath "$WORK_DIR\benchmark\results\benchmark.log"

# Step 4: Report
Write-Host ""
Write-Host "[4/4] Generating report..."
$report = @"
# P-series Optimization Test Report
Test Date: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')

## Test Summary
- Build: bsmap-rs release mode
- Unit Tests: 26 tests
- Benchmark: Ex1 (WGBS SE) + Ex2 (WGBS PE)

## Results
See benchmark/results/ directory for detailed logs.
"@
$report | Out-File -FilePath "$WORK_DIR\benchmark\results\P_SERIES_TEST_REPORT.md"

Write-Host ""
Write-Host "========================================"
Write-Host "Test completed!"
Write-Host "========================================"
Get-ChildItem "$WORK_DIR\benchmark\results" | Select-Object Name, Length
