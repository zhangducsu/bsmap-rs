#!/bin/bash
set -e
export PATH="$HOME/.cargo/bin:$PATH"

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BENCH="$PROJECT/bsmap-rs/benchmark"
RESULTS_DIR="$BENCH/results_p8_$(date +%Y%m%d_%H%M%S)"
BSMAP_RS="$PROJECT/bsmap-rs/target/release/bsmap"
REF_SRC="$BENCH/data/chr22_tail_1M.fa"
EX1_READS="$BENCH/tmp/ex1_se75_10x.fastq"
EX2_READ1="$BENCH/tmp/ex2_pe150_10x_1.fastq"
EX2_READ2="$BENCH/tmp/ex2_pe150_10x_2.fastq"
NPROC=$(nproc)
P7_RESULTS="$BENCH/results_p7_20260521_055015"

# P8-3: 复制参考序列和 .bsi 到 WSL2 ext4 加速加载
TMP_REF="/tmp/p8_ref.fa"
TMP_BSI="/tmp/p8_ref.fa.bsi"

mkdir -p "$RESULTS_DIR"/rust/{p1,p4}

echo "============================================"
echo "BSMAP P8 基准测试"
echo "============================================"
echo "开始时间: $(date)"
echo "CPU 核心: $NPROC"
echo "结果目录: $RESULTS_DIR"
echo "P7 对比基准: $P7_RESULTS"
echo ""

# ============================
# P8-3: 索引加载优化
# ============================
echo ">>> P8-3: 准备 ext4 索引..."
if [ ! -f "$TMP_BSI" ]; then
    echo "  复制参考序列到 /tmp ..."
    cp "$REF_SRC" "$TMP_REF"
    echo "  复制 .bsi 索引到 /tmp (519MB, 可能需要几秒) ..."
    cp "$PROJECT/bsmap-rs/benchmark/data/chr22_tail_1M.fa.bsi" "$TMP_BSI"
    echo "  完成"
else
    echo "  ext4 索引已存在，跳过复制"
fi

# ============================
# 1. P8 Rust Ex1 SE p=1
# ============================
echo ""
echo ">>> [1/4] P8 bsmap-rs - Ex1 SE p=1 (ext4索引) ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS" \
    -d "$TMP_REF" \
    -o "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p1/ex1_se_rust.log"
echo "  完成"

# ============================
# 2. P8 Rust Ex2 PE p=1
# ============================
echo ""
echo ">>> [2/4] P8 bsmap-rs - Ex2 PE p=1 (ext4索引) ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p1.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$TMP_REF" \
    -o "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 1 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p1/ex2_pe_rust.log"
echo "  完成"

# ============================
# 3. P8 Rust Ex1 SE p=4
# ============================
echo ""
echo ">>> [3/4] P8 bsmap-rs - Ex1 SE p=4 (ext4索引) ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex1_se_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX1_READS" \
    -d "$TMP_REF" \
    -o "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p4/ex1_se_rust.log"
echo "  完成"

# ============================
# 4. P8 Rust Ex2 PE p=4
# ============================
echo ""
echo ">>> [4/4] P8 bsmap-rs - Ex2 PE p=4 (ext4索引) ..."
/usr/bin/time -v -o "$RESULTS_DIR/rust/ex2_pe_p4.time" \
    "$BSMAP_RS" align \
    -a "$EX2_READ1" -b "$EX2_READ2" \
    -d "$TMP_REF" \
    -o "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    -s 16 -v 0.08 -I 4 -p 4 --verbose 2 \
    2>&1 | tee "$RESULTS_DIR/rust/p4/ex2_pe_rust.log"
echo "  完成"

echo ""
echo "============================================"
echo "P8 比对完成！生成三路对比分析..."
echo "============================================"

# ============================
# SAM 三路对比
# ============================
mkdir -p "$RESULTS_DIR/comparison"

compare_three() {
    local LABEL="$1"
    local CPP_SAM="$2"   # P7 C++ baseline
    local P7_SAM="$3"    # P7 Rust
    local P8_SAM="$4"    # P8 Rust
    local OUT_DIR="$5"

    mkdir -p "$OUT_DIR"

    local CPP_LINES=$(grep -cv '^@' "$CPP_SAM" 2>/dev/null || echo "N/A")
    local P7_LINES=$(grep -cv '^@' "$P7_SAM" 2>/dev/null || echo "N/A")
    local P8_LINES=$(grep -cv '^@' "$P8_SAM" 2>/dev/null || echo "N/A")

    # 排序后对比
    grep -v '^@' "$CPP_SAM" 2>/dev/null | sort > "$OUT_DIR/cpp_sorted.sam" || true
    grep -v '^@' "$P7_SAM" 2>/dev/null | sort > "$OUT_DIR/p7_sorted.sam" || true
    grep -v '^@' "$P8_SAM" 2>/dev/null | sort > "$OUT_DIR/p8_sorted.sam" || true

    diff "$OUT_DIR/cpp_sorted.sam" "$OUT_DIR/p7_sorted.sam" > "$OUT_DIR/cpp_vs_p7.diff" 2>&1 || true
    diff "$OUT_DIR/cpp_sorted.sam" "$OUT_DIR/p8_sorted.sam" > "$OUT_DIR/cpp_vs_p8.diff" 2>&1 || true
    diff "$OUT_DIR/p7_sorted.sam" "$OUT_DIR/p8_sorted.sam" > "$OUT_DIR/p7_vs_p8.diff" 2>&1 || true

    local CPP_P7_DIFF=$(wc -l < "$OUT_DIR/cpp_vs_p7.diff")
    local CPP_P8_DIFF=$(wc -l < "$OUT_DIR/cpp_vs_p8.diff")
    local P7_P8_DIFF=$(wc -l < "$OUT_DIR/p7_vs_p8.diff")

    echo "=== $LABEL 三路对比 ===" | tee "$OUT_DIR/summary.txt"
    echo "C++ (P7基线):  $CPP_LINES 行" | tee -a "$OUT_DIR/summary.txt"
    echo "P7 Rust:       $P7_LINES 行 (diff vs C++: $CPP_P7_DIFF)" | tee -a "$OUT_DIR/summary.txt"
    echo "P8 Rust:       $P8_LINES 行 (diff vs C++: $CPP_P8_DIFF)" | tee -a "$OUT_DIR/summary.txt"
    echo "" | tee -a "$OUT_DIR/summary.txt"
    echo "P7→P8 改善: diff $P7_P8_DIFF 行" | tee -a "$OUT_DIR/summary.txt"
    if [ "$P7_P8_DIFF" -gt 0 ]; then
        echo "P7 vs P8 差异样本 (前20行):" | tee -a "$OUT_DIR/summary.txt"
        head -20 "$OUT_DIR/p7_vs_p8.diff" | tee -a "$OUT_DIR/summary.txt"
    fi
    echo "" | tee -a "$OUT_DIR/summary.txt"
    cat "$OUT_DIR/summary.txt"
}

echo ""
echo "--- Ex1 SE p=1 ---"
compare_three "Ex1_SE_p1" \
    "$P7_RESULTS/cpp/p1/ex1_se_cpp.sam" \
    "$P7_RESULTS/rust/p1/ex1_se_rust.sam" \
    "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" \
    "$RESULTS_DIR/comparison/ex1_se_p1"

echo ""
echo "--- Ex1 SE p=4 ---"
compare_three "Ex1_SE_p4" \
    "$P7_RESULTS/cpp/p4/ex1_se_cpp.sam" \
    "$P7_RESULTS/rust/p4/ex1_se_rust.sam" \
    "$RESULTS_DIR/rust/p4/ex1_se_rust.sam" \
    "$RESULTS_DIR/comparison/ex1_se_p4"

echo ""
echo "--- Ex2 PE p=1 ---"
compare_three "Ex2_PE_p1" \
    "$P7_RESULTS/cpp/p1/ex2_pe_cpp.sam" \
    "$P7_RESULTS/rust/p1/ex2_pe_rust.sam" \
    "$RESULTS_DIR/rust/p1/ex2_pe_rust.sam" \
    "$RESULTS_DIR/comparison/ex2_pe_p1"

echo ""
echo "--- Ex2 PE p=4 ---"
compare_three "Ex2_PE_p4" \
    "$P7_RESULTS/cpp/p4/ex2_pe_cpp.sam" \
    "$P7_RESULTS/rust/p4/ex2_pe_rust.sam" \
    "$RESULTS_DIR/rust/p4/ex2_pe_rust.sam" \
    "$RESULTS_DIR/comparison/ex2_pe_p4"

# ============================
# FLAG 分布对比
# ============================
echo ""
echo "=== FLAG 分布对比 (Ex1 SE p=1) ==="
echo "C++ P7:"
grep -v '^@' "$P7_RESULTS/cpp/p1/ex1_se_cpp.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10
echo ""
echo "P7 Rust:"
grep -v '^@' "$P7_RESULTS/rust/p1/ex1_se_rust.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10
echo ""
echo "P8 Rust:"
grep -v '^@' "$RESULTS_DIR/rust/p1/ex1_se_rust.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10

# ============================
# 统计对比 (unique/multiple)
# ============================
echo ""
echo "=== Unique/Multiple 统计对比 (Ex1 SE p=1) ==="
echo "P8 Rust stats:"
grep -E "比对读段数|唯一比对|多重比对" "$RESULTS_DIR/rust/p1/ex1_se_rust.log"

echo ""
echo "============================================"
echo "P8 基准测试完成: $(date)"
echo "结果目录: $RESULTS_DIR"
echo "============================================"

# 性能数据汇总
echo ""
echo "=== GNU time 性能数据汇总 ==="
for f in "$RESULTS_DIR"/rust/*.time; do
    echo "--- $(basename "$f") ---"
    grep -E "User time|System time|Elapsed|Maximum resident" "$f"
    echo ""
done

echo ""
echo "=== P7 vs P8 性能对比 ==="
for label in ex1_se_p1 ex1_se_p4 ex2_pe_p1 ex2_pe_p4; do
    p7f="$P7_RESULTS/rust/${label}.time"
    p8f="$RESULTS_DIR/rust/${label}.time"
    if [ -f "$p7f" ] && [ -f "$p8f" ]; then
        p7_elapsed=$(grep "Elapsed" "$p7f" | awk '{print $8}')
        p8_elapsed=$(grep "Elapsed" "$p8f" | awk '{print $8}')
        echo "$label: P7=$p7_elapsed  P8=$p8_elapsed"
    fi
done
