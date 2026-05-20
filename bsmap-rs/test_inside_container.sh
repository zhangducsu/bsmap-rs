#!/bin/bash
# P-series optimization test script
set -e

cd /workspace/bsmap-rs

echo "=========================================="
echo "P-series Optimization Complete Test"
echo "=========================================="
date
echo ""

echo "[1/4] Build bsmap-rs (release)..."
cargo build --release 2>&1 | tee benchmark/results/build.log
echo ""

echo "[2/4] Run unit tests..."
cargo test --package bsmap 2>&1 | tee benchmark/results/tests.log
echo ""

echo "[3/4] Run Ex1/Ex2 benchmark..."
cd benchmark
./run_ex1_ex2.sh 2>&1 | tee ../benchmark/results/benchmark.log
cd ..
echo ""

echo "[4/4] Generate test report..."
cat > benchmark/results/P_SERIES_TEST_REPORT.md << EOF
# P-series Optimization Test Report
## Test Date: $(date)

## Performance Comparison
See summary.csv for detailed data.

## SAM Consistency
- Ex1: comparison_example1_wgbs_se/detailed_report.txt
- Ex2: comparison_example2_wgbs_pe/detailed_report.txt
EOF

echo ""
echo "=========================================="
echo "Test completed!"
echo "=========================================="
ls -lh benchmark/results/
