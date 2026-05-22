#!/bin/bash
set -e
source ~/.profile
export PATH="$HOME/.cargo/bin:$PATH"

RESULTS=/home/zhang_i5edc0/bsmap_benchmark/results_p11
mkdir -p "$RESULTS"

RUST=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
REF=/home/zhang_i5edc0/bsmap_benchmark/data/chr22_tail_1M.fa
READS=/home/zhang_i5edc0/bsmap_benchmark/tmp/ex1_se75_10x.fastq

echo "=== P11 Benchmark: Ex1 SE (chr22 1M tail, 75bp, 10x) ==="
echo "Start: $(date)"
echo "Results: $RESULTS"
echo ""

# 1. P11 Rust p=1
echo ">>> [1/2] P11 bsmap-rs Ex1 SE p=1 ..."
/usr/bin/time -v -o "$RESULTS/ex1_se_p1.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/ex1_se_rust_p1.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -5
echo ""

# 2. P11 Rust p=4
echo ">>> [2/2] P11 bsmap-rs Ex1 SE p=4 ..."
/usr/bin/time -v -o "$RESULTS/ex1_se_p4.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/ex1_se_rust_p4.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tail -5
echo ""

echo "=== Benchmark runs complete: $(date) ==="

# Performance summary
echo ""
echo "=== Performance Summary ==="
for cfg in ex1_se_p1 ex1_se_p4; do
    elapsed=$(grep "Elapsed" "$RESULTS/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    maxrss=$(grep "Maximum resident" "$RESULTS/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$cfg: Elapsed=${elapsed}s  Max RSS=${maxrss}KB"
done

echo ""
echo "=== SAM Line Counts ==="
echo "P11 p=1: $(grep -cv '^@' "$RESULTS/ex1_se_rust_p1.sam")"
echo "P11 p=4: $(grep -cv '^@' "$RESULTS/ex1_se_rust_p4.sam")"

echo ""
echo "RESULTS_DIR=$RESULTS"
