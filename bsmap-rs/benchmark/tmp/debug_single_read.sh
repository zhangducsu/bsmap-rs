#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
OUT="/tmp/p9_debug_read"
READS="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"

mkdir -p "$OUT"

# Find read with ID starting with 101272 in FASTQ
echo "=== Finding read 101272 in FASTQ ==="
grep -A3 "^@101272" "$READS" > "$OUT/single.fastq"
head -4 "$OUT/single.fastq"

echo ""
echo "=== Running P9 alignment with -r 2 (report all hits) ==="
TMP_REF="/tmp/p9_ref.fa"
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d "$TMP_REF" \
    -a "$OUT/single.fastq" \
    -o "$OUT/single_out.sam" 2>&1

echo ""
echo "=== SAM output (all lines) ==="
grep -v '^@' "$OUT/single_out.sam"
