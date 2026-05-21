#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
OUT="/tmp/p9_debug_3reads"
READS="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"

mkdir -p "$OUT"

# Extract the 3 problematic reads
for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398"; do
    echo "=== Extracting $rid ==="
    grep -A3 "^@$rid" "$READS" > "$OUT/${rid%%_*}.fastq"
done

# Run with -r 2 to report all hits for each read
for f in "$OUT"/*.fastq; do
    name=$(basename "$f" .fastq)
    echo ""
    echo "=== All hits for read $name ==="
    TMP_REF="/tmp/p9_ref.fa"
    "$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
        -d "$TMP_REF" \
        -a "$f" \
        -o "$OUT/${name}_allhits.sam" 2>/dev/null

    grep -v '^@' "$OUT/${name}_allhits.sam" || echo "(no hits)"
done

echo ""
echo "=== C++ SAM for comparison ==="
C_SAM="$PROJECT/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"
for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398"; do
    echo "--- C++: $rid ---"
    grep "^$rid" "$C_SAM" | head -5
done
