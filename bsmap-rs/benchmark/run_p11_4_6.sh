#!/bin/bash
set -e
source ~/.profile
export PATH="$HOME/.cargo/bin:$PATH"

RESULTS=/home/zhang_i5edc0/bsmap_benchmark/results_p11_4_6
mkdir -p "$RESULTS"

CPP=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
RUST=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
REF=/home/zhang_i5edc0/bsmap_benchmark/data/chr22_tail_1M.fa
READS=/home/zhang_i5edc0/bsmap_benchmark/tmp/ex1_se75_10x.fastq

echo "=== P11-4~6 Benchmark: Ex1 SE (chr22 1M tail, 75bp, 10x) ==="
echo "Start: $(date)"
echo "Results: $RESULTS"
echo ""

# 1. C++ p=1
echo ">>> [1/4] C++ BSMAP p=1 ..."
/usr/bin/time -v -o "$RESULTS/cpp_p1.time" \
    "$CPP" -a "$READS" -d "$REF" -o "$RESULTS/cpp_p1.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -5
echo ""

# 2. C++ p=4
echo ">>> [2/4] C++ BSMAP p=4 ..."
/usr/bin/time -v -o "$RESULTS/cpp_p4.time" \
    "$CPP" -a "$READS" -d "$REF" -o "$RESULTS/cpp_p4.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tail -5
echo ""

# 3. P11-4~6 Rust p=1
echo ">>> [3/4] P11-4~6 Rust p=1 ..."
/usr/bin/time -v -o "$RESULTS/rust_p1.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/rust_p1.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -5
echo ""

# 4. P11-4~6 Rust p=4
echo ">>> [4/4] P11-4~6 Rust p=4 ..."
/usr/bin/time -v -o "$RESULTS/rust_p4.time" \
    "$RUST" align -a "$READS" -d "$REF" -o "$RESULTS/rust_p4.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tail -5
echo ""

echo "=== Benchmark runs complete: $(date) ==="

# ── Performance Summary ──
echo ""
echo "=== Performance Summary ==="
echo ""
printf "%-12s %12s %14s %12s\n" "Config" "Elapsed(s)" "Max_RSS(KB)" "CPU%"
echo "----------------------------------------------------------------"
for cfg in cpp_p1 cpp_p4 rust_p1 rust_p4; do
    elapsed=$(grep "Elapsed" "$RESULTS/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    maxrss=$(grep "Maximum resident" "$RESULTS/${cfg}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    cpu=$(grep "CPU" "$RESULTS/${cfg}.time" 2>/dev/null | awk '{print $NF}' | tr -d '%' || echo "N/A")
    printf "%-12s %12s %14s %11s%%\n" "$cfg" "$elapsed" "$maxrss" "$cpu"
done

# ── SAM Line Counts ──
echo ""
echo "=== SAM Read Counts ==="
for f in cpp_p1 cpp_p4 rust_p1 rust_p4; do
    count=$(grep -cv '^@' "$RESULTS/${f}.sam" 2>/dev/null || echo "N/A")
    echo "$f: $count reads"
done

# ── SAM Diff vs C++ ──
echo ""
echo "=== SAM Diff: C++ vs P11-4~6 ==="
for cfg in p1 p4; do
    cpp_sorted="$RESULTS/cpp_${cfg}_sorted.sam"
    rust_sorted="$RESULTS/rust_${cfg}_sorted.sam"
    grep -v '^@' "$RESULTS/cpp_${cfg}.sam" | sort > "$cpp_sorted"
    grep -v '^@' "$RESULTS/rust_${cfg}.sam" | sort > "$rust_sorted"
    diff_lines=$(diff "$cpp_sorted" "$rust_sorted" | wc -l)
    if [ "$diff_lines" -eq 0 ]; then
        echo "Ex1 SE $cfg: 0 diff (完全一致)"
    else
        echo "Ex1 SE $cfg: $diff_lines diff lines"
    fi
done

# ── FLAG Distribution ──
echo ""
echo "=== FLAG Distribution (C++ p=1) ==="
grep -v '^@' "$RESULTS/cpp_p1.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

echo ""
echo "=== FLAG Distribution (P11-4~6 p=1) ==="
grep -v '^@' "$RESULTS/rust_p1.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

echo ""
echo "RESULTS_DIR=$RESULTS"
