#!/bin/bash
set -e
cd /workspace/bsmap-rs

echo '=========================================='
echo 'P-series Optimization Complete Test'
echo '=========================================='
date
echo ''

# Setup environment
echo '[1/5] Setting up build environment...'
apt-get update
apt-get install -y build-essential curl wget git python3 python3-pip time
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. /root/.cargo/env
rustup default stable
chmod +x benchmark/run_ex1_ex2.sh
echo ''

# Build
echo '[2/5] Building bsmap-rs...'
cargo build --release 2>&1 | tee benchmark/results/build.log
echo ''

# Unit tests
echo '[3/5] Running unit tests...'
cargo test --package bsmap 2>&1 | tee benchmark/results/tests.log
echo ''

# Benchmark
echo '[4/5] Running benchmark tests...'
cd benchmark
./run_ex1_ex2.sh 2>&1 | tee results/benchmark.log
cd ..
echo ''

# Report
echo '[5/5] Generating test report...'
cat > benchmark/results/P_SERIES_TEST_REPORT.md << 'EOF'
# P-series Optimization Test Report

**Test Date**: 05/18/2026 11:55:43

## Test Content
1. Build: bsmap-rs (release mode)
2. Unit Tests: Verify code correctness  
3. Benchmark: Ex1 (WGBS SE 75bp 10x), Ex2 (WGBS PE 150bp 10x)

## Test Logs
- Build: benchmark/results/build.log
- Unit Tests: benchmark/results/tests.log
- Benchmark: benchmark/results/benchmark.log

## Results
See benchmark/results/ directory for detailed data.
EOF

echo ''
echo '=========================================='
echo 'P-series Optimization Test Completed!'
echo '=========================================='
ls -lh benchmark/results/
date
