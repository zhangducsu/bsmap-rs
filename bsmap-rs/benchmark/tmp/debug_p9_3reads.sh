#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
TMP_REF="/tmp/p9_ref.fa"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
OUT="/tmp/p9_debug_3reads"

mkdir -p "$OUT"

echo "=== Extracting 3 problematic reads ==="
for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398"; do
    base="${rid%%_*}"
    grep -A3 "^@$rid" "$FASTQ" > "$OUT/${base}.fastq" 2>/dev/null || echo "WARN: cannot find $rid"
    if [ -s "$OUT/${base}.fastq" ]; then
        echo "  $base: $(wc -l < "$OUT/${base}.fastq") lines"
    fi
done

echo ""
echo "=== Running P9 with debug (stderr to file) ==="
cat "$OUT"/446.fastq "$OUT"/51599.fastq "$OUT"/677.fastq > "$OUT/all3.fastq"

"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d "$TMP_REF" \
    -a "$OUT/all3.fastq" \
    -o "$OUT/all3_out.sam" 2>"$OUT/debug_stderr.txt" || true

echo ""
echo "=== P9 hits ==="
grep -v '^@' "$OUT/all3_out.sam" | head -30

echo ""
echo "=== Debug stderr ==="
cat "$OUT/debug_stderr.txt"
