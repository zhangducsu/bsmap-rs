#!/bin/bash
set -e
export PATH="$HOME/.cargo/bin:$PATH"

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BENCH="$PROJECT/bsmap-rs/benchmark"
RESULTS_DIR="$BENCH/results_p7_$(date +%Y%m%d_%H%M%S)"
BSMAP_RS="$PROJECT/bsmap-rs/target/release/bsmap"
BSMAP_CPP="$PROJECT/bsmap-original/bsmap-2.90/bsmap"
REF="$BENCH/data/chr22_tail_1M.fa"
EX1_READS="$BENCH/tmp/ex1_se75_10x.fastq"
EX2_READ1="$BENCH/tmp/ex2_pe150_10x_1.fastq"
EX2_READ2="$BENCH/tmp/ex2_pe150_10x_2.fastq"
NPROC=$(nproc)

mkdir -p "$RESULTS_DIR"/{cpp,rust}/{p1,p4}

echo "============================================"
echo "BSMAP P7 基准测试"
echo "============================================"
echo "开始时间: $(date)"
echo "CPU 核心: $NPROC"
echo "结果目录: $RESULTS_DIR"
echo ""
echo "数据集:"
echo "  Ex1: WGBS SE 75bp 10x (chr22 1Mbp)"
echo "  Ex2: WGBS PE 150bp 10x (chr22 1Mbp)"
echo "============================================"
echo ""

# ============================
# 1. C++ BSMAP Ex1 SE p=1
# ============================
echo ">>> [1/8] C++ BSMAP - Ex1 SE p=1 ..."
/usr/bin/time -v -o "$RESULTS_DIR/cpp/ex1_se_p1.time" \
    "$BSMAP_CPP" \
    -a "$EX1_READS" \
    -d "$REF" \
    -o "$RESULTS_DIR/cpp/p1/ex1_se_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee "$RESULTS_DIR/cpp/p1/ex1_se_cpp.log"
echo "  完成"

# ============================
# 2. C++ BSMAP Ex2 PE p=1
# ============================
echo ">>> [2/8] C++ BSMAP - Ex2 PE p=1 ..."
/usr/bin/time -v -o "$RESULTS_DIR/cpp/ex2_pe_p1.time" \
    "$BSMAP_CPP" \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$REF" \
    -o "$RESULTS_DIR/cpp/p1/ex2_pe_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 1 \
    2>&1 | tee "$RESULTS_DIR/cpp/p1/ex2_pe_cpp.log"
echo "  完成"

# ============================
# 3. C++ BSMAP Ex1 SE p=4
# ============================
echo ">>> [3/8] C++ BSMAP - Ex1 SE p=4 ..."
/usr/bin/time -v -o "$RESULTS_DIR/cpp/ex1_se_p4.time" \
    "$BSMAP_CPP" \
    -a "$EX1_READS" \
    -d "$REF" \
    -o "$RESULTS_DIR/cpp/p4/ex1_se_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 4 \
    2>&1 | tee "$RESULTS_DIR/cpp/p4/ex1_se_cpp.log"
echo "  完成"

# ============================
# 4. C++ BSMAP Ex2 PE p=4
# ============================
echo ">>> [4/8] C++ BSMAP - Ex2 PE p=4 ..."
/usr/bin/time -v -o "$RESULTS_DIR/cpp/ex2_pe_p4.time" \
    "$BSMAP_CPP" \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$REF" \
    -o "$RESULTS_DIR/cpp/p4/ex2_pe_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 4 \
    2>&1 | tee "$RESULTS_DIR/cpp/p4/ex2_pe_cpp.log"
echo "  完成"

# ============================
# 5. bsmap-rs Ex1 SE p=1
# ============================
echo ">>> [5/8] bsmap-rs - Ex1 SE p=1 ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS" \
    -d "$REF" \
    -o "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p1/ex1_se_rust.log"
echo "  完成"

# ============================
# 6. bsmap-rs Ex2 PE p=1
# ============================
echo ">>> [6/8] bsmap-rs - Ex2 PE p=1 ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$REF" \
    -o "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p1/ex2_pe_rust.log"
echo "  完成"

# ============================
# 7. bsmap-rs Ex1 SE p=4
# ============================
echo ">>> [7/8] bsmap-rs - Ex1 SE p=4 ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS" \
    -d "$REF" \
    -o "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p4/ex1_se_rust.log"
echo "  完成"

# ============================
# 8. bsmap-rs Ex2 PE p=4
# ============================
echo ">>> [8/8] bsmap-rs - Ex2 PE p=4 ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$REF" \
    -o "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p4/ex2_pe_rust.log"
echo "  完成"

echo ""
echo "============================================"
echo "所有比对完成！进行SAM对比分析..."
echo "============================================"

# ============================
# SAM 对比函数
# ============================
mkdir -p "$RESULTS_DIR/sam_comparison"

compare_sams() {
    local LABEL="$1"
    local CPP_SAM="$2"
    local RUST_SAM="$3"
    local OUT_DIR="$4"

    mkdir -p "$OUT_DIR"

    grep -v "^@" "$CPP_SAM" | sort > "$OUT_DIR/cpp_sorted.sam"
    grep -v "^@" "$RUST_SAM" | sort > "$OUT_DIR/rust_sorted.sam"

    local CPP_LINES=$(wc -l < "$OUT_DIR/cpp_sorted.sam")
    local RUST_LINES=$(wc -l < "$OUT_DIR/rust_sorted.sam")

    diff "$OUT_DIR/cpp_sorted.sam" "$OUT_DIR/rust_sorted.sam" > "$OUT_DIR/diff.txt" 2>&1 || true
    local DIFF_LINES=$(wc -l < "$OUT_DIR/diff.txt")

    echo "=== $LABEL SAM 对比 ===" > "$OUT_DIR/summary.txt"
    echo "C++ BSMAP 比对行数: $CPP_LINES" >> "$OUT_DIR/summary.txt"
    echo "bsmap-rs 比对行数: $RUST_LINES" >> "$OUT_DIR/summary.txt"
    echo "差异行数: $DIFF_LINES" >> "$OUT_DIR/summary.txt"
    echo "" >> "$OUT_DIR/summary.txt"

    if [ "$DIFF_LINES" -eq 0 ]; then
        echo "结论: 完全一致 (0 差异)" >> "$OUT_DIR/summary.txt"
    else
        echo "结论: 存在 $DIFF_LINES 行差异" >> "$OUT_DIR/summary.txt"
        echo "差异样本 (前50行):" >> "$OUT_DIR/summary.txt"
        head -50 "$OUT_DIR/diff.txt" >> "$OUT_DIR/summary.txt"
    fi

    cat "$OUT_DIR/summary.txt"
}

echo ""
echo "--- Ex1 SE 单线程 ---"
compare_sams "Ex1_SE_p1" \
    "$RESULTS_DIR/cpp/p1/ex1_se_cpp.sam" \
    "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    "$RESULTS_DIR/sam_comparison/ex1_se_p1"

echo ""
echo "--- Ex1 SE 4线程 ---"
compare_sams "Ex1_SE_p4" \
    "$RESULTS_DIR/cpp/p4/ex1_se_cpp.sam" \
    "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    "$RESULTS_DIR/sam_comparison/ex1_se_p4"

echo ""
echo "--- Ex2 PE 单线程 ---"
compare_sams "Ex2_PE_p1" \
    "$RESULTS_DIR/cpp/p1/ex2_pe_cpp.sam" \
    "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    "$RESULTS_DIR/sam_comparison/ex2_pe_p1"

echo ""
echo "--- Ex2 PE 4线程 ---"
compare_sams "Ex2_PE_p4" \
    "$RESULTS_DIR/cpp/p4/ex2_pe_cpp.sam" \
    "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    "$RESULTS_DIR/sam_comparison/ex2_pe_p4"

echo ""
echo "============================================"
echo "测试完成: $(date)"
echo "结果目录: $RESULTS_DIR"
echo "============================================"

# 打印性能汇总
echo ""
echo "=== GNU time 性能数据汇总 ==="
for f in "$RESULTS_DIR"/cpp/*.time "$RESULTS_DIR"/rust/*.time; do
    echo "--- $(basename "$f") ---"
    grep -E "User time|System time|Elapsed|Maximum resident|Minor page faults|Major page faults|Voluntary|Involuntary" "$f"
    echo ""
done
