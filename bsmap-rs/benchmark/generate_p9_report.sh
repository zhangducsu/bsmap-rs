#!/bin/bash
set -e

# Paths
CPP_RESULTS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_cpp_20260521_214745"
P9_RESULTS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p9_20260521_171934"
P8_RESULTS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p8_20260521_074711"
OUT_DIR="$P9_RESULTS/comparison_cpp"

mkdir -p "$OUT_DIR"

echo "============================================"
echo "C++ vs P9 vs P8 三路对比"
echo "============================================"

# =========== Ex1 SE p=1 ===========
echo ""
echo "=== Ex1 SE p=1 ==="
LABEL="Ex1_SE_p1"
D="$OUT_DIR/ex1_se_p1"
mkdir -p "$D"

CPP_SAM="$CPP_RESULTS/p1/ex1_se_cpp.sam"
P9_SAM="$P9_RESULTS/rust/p1/ex1_se_rust.sam"
P8_SAM="$P8_RESULTS/rust/p1/ex1_se_rust.sam"

CPP_LINES=$(grep -cv '^@' "$CPP_SAM")
P9_LINES=$(grep -cv '^@' "$P9_SAM")
P8_LINES=$(grep -cv '^@' "$P8_SAM")

echo "C++ 2.90: $CPP_LINES"
echo "P8:       $P8_LINES"
echo "P9:       $P9_LINES"

grep -v '^@' "$CPP_SAM" | sort > "$D/cpp_sorted.sam"
grep -v '^@' "$P9_SAM" | sort > "$D/p9_sorted.sam"
grep -v '^@' "$P8_SAM" | sort > "$D/p8_sorted.sam"

diff "$D/cpp_sorted.sam" "$D/p9_sorted.sam" > "$D/cpp_vs_p9.diff" 2>&1 || true
diff "$D/cpp_sorted.sam" "$D/p8_sorted.sam" > "$D/cpp_vs_p8.diff" 2>&1 || true
diff "$D/p8_sorted.sam" "$D/p9_sorted.sam" > "$D/p8_vs_p9.diff" 2>&1 || true

CPP_P9=$(wc -l < "$D/cpp_vs_p9.diff")
CPP_P8=$(wc -l < "$D/cpp_vs_p8.diff")
P8_P9=$(wc -l < "$D/p8_vs_p9.diff")

echo "C++ vs P9 diff: $CPP_P9   C++ vs P8 diff: $CPP_P8   P8 vs P9 diff: $P8_P9"

# FLAG distribution
echo ""
echo "FLAG 分布:"
echo "  C++: $(grep -v '^@' "$CPP_SAM" | cut -f2 | sort | uniq -c | sort -rn | tr '\n' ' ')"
echo "  P9:  $(grep -v '^@' "$P9_SAM" | cut -f2 | sort | uniq -c | sort -rn | tr '\n' ' ')"

# Stats
echo ""
echo "C++ stats:"
grep "aligned reads" "$CPP_RESULTS/p1/ex1_se_cpp.log"
echo "P9 stats:"
grep "比对读段数\|唯一比对\|多重比对" "$P9_RESULTS/rust/p1/ex1_se_rust.log" | head -3

# =========== Ex1 SE p=4 ===========
echo ""
echo "=== Ex1 SE p=4 ==="
D="$OUT_DIR/ex1_se_p4"
mkdir -p "$D"

CPP_SAM="$CPP_RESULTS/p4/ex1_se_cpp.sam"
P9_SAM="$P9_RESULTS/rust/p4/ex1_se_rust.sam"
P8_SAM="$P8_RESULTS/rust/p4/ex1_se_rust.sam"

CPP_LINES=$(grep -cv '^@' "$CPP_SAM")
P9_LINES=$(grep -cv '^@' "$P9_SAM")
P8_LINES=$(grep -cv '^@' "$P8_SAM")

echo "C++ 2.90: $CPP_LINES   P8: $P8_LINES   P9: $P9_LINES"

grep -v '^@' "$CPP_SAM" | sort > "$D/cpp_sorted.sam"
grep -v '^@' "$P9_SAM" | sort > "$D/p9_sorted.sam"
grep -v '^@' "$P8_SAM" | sort > "$D/p8_sorted.sam"

diff "$D/cpp_sorted.sam" "$D/p9_sorted.sam" > "$D/cpp_vs_p9.diff" 2>&1 || true
diff "$D/cpp_sorted.sam" "$D/p8_sorted.sam" > "$D/cpp_vs_p8.diff" 2>&1 || true
diff "$D/p8_sorted.sam" "$D/p9_sorted.sam" > "$D/p8_vs_p9.diff" 2>&1 || true

CPP_P9=$(wc -l < "$D/cpp_vs_p9.diff")
CPP_P8=$(wc -l < "$D/cpp_vs_p8.diff")
P8_P9=$(wc -l < "$D/p8_vs_p9.diff")

echo "C++ vs P9 diff: $CPP_P9   C++ vs P8 diff: $CPP_P8   P8 vs P9 diff: $P8_P9"

# =========== Ex2 PE (C++ crashed) ===========
echo ""
echo "=== Ex2 PE (C++ buffer overflow — PE only P8 vs P9) ==="
for threads in p1 p4; do
    P9_SAM="$P9_RESULTS/rust/$threads/ex2_pe_rust.sam"
    P8_SAM="$P8_RESULTS/rust/$threads/ex2_pe_rust.sam"
    P9_LINES=$(grep -cv '^@' "$P9_SAM")
    P8_LINES=$(grep -cv '^@' "$P8_SAM")
    echo "Ex2 PE $threads: P8=$P8_LINES  P9=$P9_LINES"
done

# =========== Timing ===========
echo ""
echo "=== 耗时 (Elapsed) ==="
for label in ex1_se_p1 ex1_se_p4 ex2_pe_p1 ex2_pe_p4; do
    cpp_time=$(grep "Elapsed" "$CPP_RESULTS/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p9_time=$(grep "Elapsed" "$P9_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p8_time=$(grep "Elapsed" "$P8_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$label: C++=$cpp_time  P8=$p8_time  P9=$p9_time"
done

# =========== Memory ===========
echo ""
echo "=== 内存 (Max RSS KB) ==="
for label in ex1_se_p1 ex1_se_p4; do
    cpp_mem=$(grep "Maximum resident" "$CPP_RESULTS/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p9_mem=$(grep "Maximum resident" "$P9_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p8_mem=$(grep "Maximum resident" "$P8_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$label: C++=$cpp_mem  P8=$p8_mem  P9=$p9_mem"
done

echo ""
echo "============================================"
echo "对比完成: $(date)"
echo "============================================"
