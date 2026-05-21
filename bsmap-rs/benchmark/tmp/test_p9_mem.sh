#!/bin/bash
set -e
PATH="/home/zhang_i5edc0/.cargo/bin:$PATH"
PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
REF="/home/zhang_i5edc0/bsmap_benchmark/data/chr22_tail_1M.fa"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
OUT=/tmp/p9_memtest

mkdir -p "$OUT"
rm -f "$REF.bsi"

echo "=== P9 Memory Test (WSL2 ext4 ref, 10x data) ==="
/usr/bin/time -v "$BIN" align \
    -a "$FASTQ" \
    -d "$REF" \
    -o "$OUT/test.sam" \
    -s 16 -v 0.08 -I 4 -p 1 2>&1 | tail -3

echo ""
echo "=== SAM count ==="
grep -cv '^@' "$OUT/test.sam"

echo ""
echo "=== Memory ==="
echo "Check /usr/bin/time output above for 'Maximum resident set size'"
