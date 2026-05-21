#!/bin/bash
set -e

C_SAM="$1"
RUST_SAM="$2"
OUTDIR="$3"

mkdir -p "$OUTDIR"

echo "=== 提取 FLAG 分类 ==="

# C++ unique reads (flag 0 or 16)
grep -v '^@' "$C_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > "$OUTDIR/c_unique.txt"

# Rust unique reads
grep -v '^@' "$RUST_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > "$OUTDIR/rust_unique.txt"

# Rust multiple reads
grep -v '^@' "$RUST_SAM" | awk '{flag=$2; if(flag==256||flag==272) print $1}' | sort > "$OUTDIR/rust_multi.txt"

echo "C++ unique: $(wc -l < "$OUTDIR/c_unique.txt")"
echo "Rust unique: $(wc -l < "$OUTDIR/rust_unique.txt")"
echo "Rust multiple: $(wc -l < "$OUTDIR/rust_multi.txt")"

# Reads in C++ unique but NOT in Rust unique = misclassified
comm -23 "$OUTDIR/c_unique.txt" "$OUTDIR/rust_unique.txt" > "$OUTDIR/diff_reads.txt"
echo "C++ unique but Rust NOT unique: $(wc -l < "$OUTDIR/diff_reads.txt")"

# Of those, which are Rust multiple (confirming unique→multiple reclassification)
comm -12 "$OUTDIR/diff_reads.txt" "$OUTDIR/rust_multi.txt" > "$OUTDIR/c_unique_rust_multi.txt"
echo "Confirmed C++ unique → Rust multiple: $(wc -l < "$OUTDIR/c_unique_rust_multi.txt")"

echo ""
echo "=== 前 10 条 unique-in-C++ multi-in-Rust 样本 ==="
head -10 "$OUTDIR/c_unique_rust_multi.txt"

echo ""
echo "=== 样本 read 在 C++ vs Rust SAM 中的行 ==="
for read in $(head -5 "$OUTDIR/c_unique_rust_multi.txt"); do
    echo ""
    echo "--- $read (C++) ---"
    grep "^$read[[:space:]]" "$C_SAM" | head -1
    echo "--- $read (Rust) ---"
    grep "^$read[[:space:]]" "$RUST_SAM" | head -1
done
