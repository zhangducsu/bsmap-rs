#!/bin/bash
set -e
BPATH="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap"
BENCH="/home/zhang_i5edc0/bsmap_benchmark"
RESULTS_DIR="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_cpp_$(date +%Y%m%d_%H%M%S)"
REF="$BENCH/data/chr22_tail_1M.fa"
EX1_10X="$BENCH/tmp/ex1_se75_10x.fastq"
EX2_R1="$BENCH/tmp/ex2_pe150_10x_1.fastq"
EX2_R2="$BENCH/tmp/ex2_pe150_10x_2.fastq"

mkdir -p "$RESULTS_DIR"/{p1,p4}

echo "=== C++ BSMAP 2.90 基准测试 ==="
echo "开始: $(date)"
echo "结果目录: $RESULTS_DIR"
echo ""

echo ">>> [1/4] C++ Ex1 SE p=1 ..."
rm -f "$REF.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/ex1_se_p1.time" \
    "$BPATH" \
    -a "$EX1_10X" -d "$REF" -o "$RESULTS_DIR/p1/ex1_se_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee "$RESULTS_DIR/p1/ex1_se_cpp.log"
echo "  完成"

echo ""
echo ">>> [2/4] C++ Ex2 PE p=1 ..."
rm -f "$REF.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/ex2_pe_p1.time" \
    "$BPATH" \
    -a "$EX2_R1" -b "$EX2_R2" -d "$REF" -o "$RESULTS_DIR/p1/ex2_pe_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee "$RESULTS_DIR/p1/ex2_pe_cpp.log"
echo "  完成"

echo ""
echo ">>> [3/4] C++ Ex1 SE p=4 ..."
rm -f "$REF.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/ex1_se_p4.time" \
    "$BPATH" \
    -a "$EX1_10X" -d "$REF" -o "$RESULTS_DIR/p4/ex1_se_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tee "$RESULTS_DIR/p4/ex1_se_cpp.log"
echo "  完成"

echo ""
echo ">>> [4/4] C++ Ex2 PE p=4 ..."
rm -f "$REF.bsi"
/usr/bin/time -v -o "$RESULTS_DIR/ex2_pe_p4.time" \
    "$BPATH" \
    -a "$EX2_R1" -b "$EX2_R2" -d "$REF" -o "$RESULTS_DIR/p4/ex2_pe_cpp.sam" \
    -s 16 -v 0.08 -I 4 -p 4 2>&1 | tee "$RESULTS_DIR/p4/ex2_pe_cpp.log"
echo "  完成"

echo ""
echo "=== FLAG 分布 (Ex1 SE p=1) ==="
grep -v '^@' "$RESULTS_DIR/p1/ex1_se_cpp.sam" | cut -f2 | sort | uniq -c | sort -rn | head -10
echo ""
echo "=== SAM 行数 ==="
for cfg in p1/ex1_se_cpp p1/ex2_pe_cpp p4/ex1_se_cpp p4/ex2_pe_cpp; do
    lines=$(grep -cv '^@' "$RESULTS_DIR/$cfg.sam" 2>/dev/null || echo "N/A")
    echo "$cfg: $lines"
done
echo ""
echo "=== C++ 基准测试完成: $(date) ==="
echo "结果目录: $RESULTS_DIR"
