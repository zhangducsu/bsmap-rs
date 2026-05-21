#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
OUT=/tmp/p9_debug_4reads

rm -f /tmp/p9_ref.fa.bsi

echo "=== Running bsmap to build index ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d /tmp/p9_ref.fa \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_v2.sam" 2>&1

echo ""
echo "=== .bsi file ==="
ls -la /tmp/p9_ref.fa.bsi 2>/dev/null || echo "NOT FOUND"

echo ""
echo "=== SAM output ==="
grep -v '^@' "$OUT/all4_out_v2.sam" 2>/dev/null || echo "no SAM output"
