#!/bin/bash
set -e
PATH="/home/zhang_i5edc0/.cargo/bin:$PATH"
PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
REF="/home/zhang_i5edc0/bsmap_benchmark/data/chr22_tail_1M.fa"
OUT=/tmp/p9_debug_4reads

mkdir -p "$OUT"

for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398" "58512_chr22_tail_1M:772128-772202"; do
    base="${rid%%_*}"
    grep -A3 "^@$rid" "$FASTQ" > "$OUT/${base}.fastq" 2>/dev/null
done
cat "$OUT"/446.fastq "$OUT"/51599.fastq "$OUT"/677.fastq "$OUT"/58512.fastq > "$OUT/all4.fastq"

rm -f "$REF.bsi"
echo "=== Running P9 ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d "$REF" \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_p9.sam"

echo ""
echo "=== SAM ==="
cat "$OUT/all4_out_p9.sam"

echo ""
echo "=== Counts ==="
for rid in 446 51599 677 58512; do
    count=$(grep -c "^${rid}_" "$OUT/all4_out_p9.sam" 2>/dev/null || echo 0)
    echo "Read $rid: $count"
done
