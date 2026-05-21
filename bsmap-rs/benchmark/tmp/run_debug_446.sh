#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
TMP_REF="/tmp/p9_ref.fa"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
OUT="/tmp/p9_debug_446"

mkdir -p "$OUT"

echo "=== Extracting read 446 ==="
RID="446_chr22_tail_1M:772041-772115"
grep -A3 "^@$RID" "$FASTQ" > "$OUT/446.fastq" 2>/dev/null
if [ -s "$OUT/446.fastq" ]; then
    echo "  446: $(wc -l < "$OUT/446.fastq") lines"
else
    echo "ERROR: cannot find read 446"
    exit 1
fi

echo ""
echo "=== Running P9 with debug for read 446 ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 \
    -d "$TMP_REF" \
    -a "$OUT/446.fastq" \
    -o "$OUT/446_out.sam" 2>"$OUT/debug_stderr.txt" || true

echo ""
echo "=== SAM output ==="
cat "$OUT/446_out.sam"

echo ""
echo "=== Debug stderr ==="
cat "$OUT/debug_stderr.txt"
