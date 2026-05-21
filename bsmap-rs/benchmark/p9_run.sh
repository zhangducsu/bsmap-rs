#!/bin/bash
set -e
export PATH="$HOME/.cargo/bin:$PATH"

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BENCH="/home/zhang_i5edc0/bsmap_benchmark"
RESULTS_DIR="$PROJECT/bsmap-rs/benchmark/results_p9_$(date +%Y%m%d_%H%M%S)"
BSMAP_RS="$PROJECT/bsmap-rs/target/release/bsmap"
NPROC=$(nproc)

REF_SRC="$BENCH/data/chr22_tail_1M.fa"
EX1_READS_10X="$BENCH/tmp/ex1_se75_10x.fastq"
EX2_READ1_10X="$BENCH/tmp/ex2_pe150_10x_1.fastq"
EX2_READ2_10X="$BENCH/tmp/ex2_pe150_10x_2.fastq"

P7_RESULTS="$PROJECT/bsmap-rs/benchmark/results_p7_20260521_055015"
P8_RESULTS="$PROJECT/bsmap-rs/benchmark/results_p8_20260521_074711"

mkdir -p "$RESULTS_DIR"/rust/{p1,p4}

echo "============================================"
echo "BSMAP P9 基准测试"
echo "============================================"
echo "开始时间: $(date)"
echo "CPU 核心: $NPROC"
echo "结果目录: $RESULTS_DIR"
echo ""

# 1. P9 Rust Ex1 SE p=1
echo ">>> [1/4] P9 - Ex1 SE p=1 ..."
rm -f "$REF_SRC.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS_10X" \
    -d "$REF_SRC" \
    -o "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee "$RESULTS_DIR/rust/p1/ex1_se_rust.log"
echo "  完成"

# 2. P9 Rust Ex2 PE p=1
echo ""
echo ">>> [2/4] P9 - Ex2 PE p=1 ..."
rm -f "$REF_SRC.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1_10X" -b "$EX2_READ2_10X" \
    -d "$REF_SRC" \
    -o "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee "$RESULTS_DIR/rust/p1/ex2_pe_rust.log"
echo "  完成"

# 3. P9 Rust Ex1 SE p=4
echo ""
echo ">>> [3/4] P9 - Ex1 SE p=4 ..."
rm -f "$REF_SRC.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS_10X" \
    -d "$REF_SRC" \
    -o "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tee "$RESULTS_DIR/rust/p4/ex1_se_rust.log"
echo "  完成"

# 4. P9 Rust Ex2 PE p=4
echo ""
echo ">>> [4/4] P9 - Ex2 PE p=4 ..."
rm -f "$REF_SRC.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1_10X" -b "$EX2_READ2_10X" \
    -d "$REF_SRC" \
    -o "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tee "$RESULTS_DIR/rust/p4/ex2_pe_rust.log"
echo "  完成"

# 三路对比
echo ""
echo "============================================"
echo "三路对比分析"
echo "============================================"

mkdir -p "$RESULTS_DIR/comparison"

compare_three_way() {
    local LABEL="$1"
    local CPP_SAM="$2"
    local P8_SAM="$3"
    local P9_SAM="$4"
    local OUT_DIR="$5"

    mkdir -p "$OUT_DIR"

    local CPP_LINES=$(grep -cv '^@' "$CPP_SAM" 2>/dev/null || echo "N/A")
    local P8_LINES=$(grep -cv '^@' "$P8_SAM" 2>/dev/null || echo "N/A")
    local P9_LINES=$(grep -cv '^@' "$P9_SAM" 2>/dev/null || echo "N/A")

    grep -v '^@' "$CPP_SAM" 2>/dev/null | sort > "$OUT_DIR/cpp_sorted.sam" || true
    grep -v '^@' "$P8_SAM" 2>/dev/null | sort > "$OUT_DIR/p8_sorted.sam" || true
    grep -v '^@' "$P9_SAM" 2>/dev/null | sort > "$OUT_DIR/p9_sorted.sam" || true

    diff "$OUT_DIR/cpp_sorted.sam" "$OUT_DIR/p8_sorted.sam" > "$OUT_DIR/cpp_vs_p8.diff" 2>&1 || true
    diff "$OUT_DIR/cpp_sorted.sam" "$OUT_DIR/p9_sorted.sam" > "$OUT_DIR/cpp_vs_p9.diff" 2>&1 || true
    diff "$OUT_DIR/p8_sorted.sam" "$OUT_DIR/p9_sorted.sam" > "$OUT_DIR/p8_vs_p9.diff" 2>&1 || true

    local CPP_P8_DIFF=$(wc -l < "$OUT_DIR/cpp_vs_p8.diff")
    local CPP_P9_DIFF=$(wc -l < "$OUT_DIR/cpp_vs_p9.diff")
    local P8_P9_DIFF=$(wc -l < "$OUT_DIR/p8_vs_p9.diff")

    echo "=== $LABEL ===" | tee "$OUT_DIR/summary.txt"
    echo "C++: $CPP_LINES  P8: $P8_LINES  P9: $P9_LINES" | tee -a "$OUT_DIR/summary.txt"
    echo "C++ vs P8 diff: $CPP_P8_DIFF   C++ vs P9 diff: $CPP_P9_DIFF   P8 vs P9 diff: $P8_P9_DIFF" | tee -a "$OUT_DIR/summary.txt"
    if [ "$P8_P9_DIFF" -gt 0 ]; then
        echo "P8 vs P9 差异 (前20行):" | tee -a "$OUT_DIR/summary.txt"
        head -20 "$OUT_DIR/p8_vs_p9.diff" | tee -a "$OUT_DIR/summary.txt"
    else
        echo "P8 vs P9: 0 diff" | tee -a "$OUT_DIR/summary.txt"
    fi
    cat "$OUT_DIR/summary.txt"
}

echo ""
echo "--- Ex1 SE p=1 ---"
compare_three_way "Ex1_SE_p1" \
    "$P7_RESULTS/cpp/p1/ex1_se_cpp.sam" \
    "$P8_RESULTS/rust/p1/ex1_se_rust.sam" \
    "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    "$RESULTS_DIR/comparison/ex1_se_p1"

echo ""
echo "--- Ex1 SE p=4 ---"
compare_three_way "Ex1_SE_p4" \
    "$P7_RESULTS/cpp/p4/ex1_se_cpp.sam" \
    "$P8_RESULTS/rust/p4/ex1_se_rust.sam" \
    "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    "$RESULTS_DIR/comparison/ex1_se_p4"

echo ""
echo "--- Ex2 PE p=1 ---"
compare_three_way "Ex2_PE_p1" \
    "$P7_RESULTS/cpp/p1/ex2_pe_cpp.sam" \
    "$P8_RESULTS/rust/p1/ex2_pe_rust.sam" \
    "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    "$RESULTS_DIR/comparison/ex2_pe_p1"

echo ""
echo "--- Ex2 PE p=4 ---"
compare_three_way "Ex2_PE_p4" \
    "$P7_RESULTS/cpp/p4/ex2_pe_cpp.sam" \
    "$P8_RESULTS/rust/p4/ex2_pe_rust.sam" \
    "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    "$RESULTS_DIR/comparison/ex2_pe_p4"

# FLAG 分布
echo ""
echo "=== FLAG 分布 (Ex1 SE p=1) ==="
echo "C++:"
grep -v '^@' "$P7_RESULTS/cpp/p1/ex1_se_cpp.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10
echo ""
echo "P8:"
grep -v '^@' "$P8_RESULTS/rust/p1/ex1_se_rust.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10
echo ""
echo "P9:"
grep -v '^@' "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

# 内存
echo ""
echo "=== 内存 (Max RSS KB) ==="
for label in ex1_se_p1 ex1_se_p4 ex2_pe_p1 ex2_pe_p4; do
    p7_mem=$(grep "Maximum resident" "$P7_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p8_mem=$(grep "Maximum resident" "$P8_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p9_mem=$(grep "Maximum resident" "$RESULTS_DIR/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$label: P7=$p7_mem  P8=$p8_mem  P9=$p9_mem"
done

# 耗时
echo ""
echo "=== 耗时 (Elapsed) ==="
for label in ex1_se_p1 ex1_se_p4 ex2_pe_p1 ex2_pe_p4; do
    p7_time=$(grep "Elapsed" "$P7_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p8_time=$(grep "Elapsed" "$P8_RESULTS/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    p9_time=$(grep "Elapsed" "$RESULTS_DIR/rust/${label}.time" 2>/dev/null | awk '{print $NF}' || echo "N/A")
    echo "$label: P7=$p7_time  P8=$p8_time  P9=$p9_time"
done

echo ""
echo "============================================"
echo "P9 基准测试完成: $(date)"
echo "结果目录: $RESULTS_DIR"
echo "============================================"
