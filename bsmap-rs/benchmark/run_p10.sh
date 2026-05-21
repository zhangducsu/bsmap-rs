#!/bin/bash
set -e
source ~/.profile
export PATH="$HOME/.cargo/bin:$PATH"

RESULTS=/home/zhang_i5edc0/bsmap_benchmark/results_p10_final
mkdir -p "$RESULTS"/cpp "$RESULTS"/rust

CPP=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
RUST=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
REF=/home/zhang_i5edc0/bsmap_benchmark/data/chr22_tail_1M.fa
READS=/home/zhang_i5edc0/bsmap_benchmark/tmp/ex1_se75_10x.fastq

echo "=== P10 Benchmark: Ex1 SE (chr22 1M tail, 75bp, 10x) ==="
echo "Start: $(date)"
echo "Results: $RESULTS"
echo ""

# 1. C++ p=1
echo ">>> [1/4] C++ BSMAP Ex1 SE p=1 ..."
/usr/bin/time -v -o "$RESULTS/cpp/ex1_se_p1.time" \
    "$CPP" -a "$READS" -d "$REF" -o "$RESULTS/cpp/ex1_se_cpp_p1.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -5
echo ""

# 2. C++ p=4
echo ">>> [2/4] C++ BSMAP Ex1 SE p=4 ..."
/usr/bin/time -v -o "$RESULTS/cpp/ex1_se_p4.time" \
    "$CPP" -a "$READS" -d "$REF" -o "$RESULTS/cpp/ex1_se_cpp_p4.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tail -5
echo ""

# 3. P10 Rust p=1
echo ">>> [3/4] P10 bsmap-rs Ex1 SE p=1 ..."
/usr/bin/time -v -o "$RESULTS/rust/ex1_se_p1.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/rust/ex1_se_rust_p1.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -5
echo ""

# 4. P10 Rust p=4
echo ">>> [4/4] P10 bsmap-rs Ex1 SE p=4 ..."
/usr/bin/time -v -o "$RESULTS/rust/ex1_se_p4.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/rust/ex1_se_rust_p4.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tail -5
echo ""

echo "=== Benchmark runs complete: $(date) ==="

# Performance summary
echo ""
echo "=== Performance Summary ==="
for cfg in ex1_se_p1 ex1_se_p4; do
    cpp_elapsed=$(grep "Elapsed" "$RESULTS/cpp/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    rust_elapsed=$(grep "Elapsed" "$RESULTS/rust/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    cpp_maxrss=$(grep "Maximum resident" "$RESULTS/cpp/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    rust_maxrss=$(grep "Maximum resident" "$RESULTS/rust/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$cfg: C++=${cpp_elapsed}s (${cpp_maxrss}KB)  Rust=${rust_elapsed}s (${rust_maxrss}KB)"
done

echo ""
echo "=== SAM Line Counts ==="
echo "C++ p=1: $(grep -cv '^@' "$RESULTS/cpp/ex1_se_cpp_p1.sam")"
echo "C++ p=4: $(grep -cv '^@' "$RESULTS/cpp/ex1_se_cpp_p4.sam")"
echo "Rust p=1: $(grep -cv '^@' "$RESULTS/rust/ex1_se_rust_p1.sam")"
echo "Rust p=4: $(grep -cv '^@' "$RESULTS/rust/ex1_se_rust_p4.sam")"

# SAM diff
echo ""
echo "=== SAM Diff: C++ vs P10 ==="
for cfg in p1 p4; do
    cpp_sorted="$RESULTS/cpp/cpp_${cfg}_sorted.sam"
    rust_sorted="$RESULTS/rust/rust_${cfg}_sorted.sam"
    grep -v '^@' "$RESULTS/cpp/ex1_se_cpp_${cfg}.sam" | sort > "$cpp_sorted"
    grep -v '^@' "$RESULTS/rust/ex1_se_rust_${cfg}.sam" | sort > "$rust_sorted"
    diff_lines=$(diff "$cpp_sorted" "$rust_sorted" | wc -l)
    echo "Ex1 SE $cfg: diff lines = $diff_lines"
done

echo ""
echo "=== FLAG Distribution (C++ p=1) ==="
grep -v '^@' "$RESULTS/cpp/ex1_se_cpp_p1.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

echo ""
echo "=== FLAG Distribution (P10 p=1) ==="
grep -v '^@' "$RESULTS/rust/ex1_se_rust_p1.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

echo ""
echo "RESULTS_DIR=$RESULTS"
