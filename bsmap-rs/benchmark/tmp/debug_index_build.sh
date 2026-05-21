#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
OUT=/tmp/p9_debug_4reads

cd "$PROJECT/bsmap-rs"

echo "=== Building ==="
$HOME/.cargo/bin/cargo build --release -p bsmap 2>&1 | tail -3

echo ""
echo "=== Rebuilding .bsi ==="
rm -f /tmp/p9_ref.fa.bsi

"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d /tmp/p9_ref.fa \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_v3.sam" 2>/tmp/index_debug_stderr.txt

echo ""
echo "=== INDEX_DEBUG lines ==="
grep 'INDEX_DEBUG' /tmp/index_debug_stderr.txt || echo "(none found)"

echo ""
echo "=== First 5 non-INDEX_DEBUG lines ==="
grep -v 'INDEX_DEBUG' /tmp/index_debug_stderr.txt | head -5
