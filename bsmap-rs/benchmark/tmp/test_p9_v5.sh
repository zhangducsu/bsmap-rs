#!/bin/bash
set -e
PATH="/home/zhang_i5edc0/.cargo/bin:$PATH"

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
REF_SRC="$PROJECT/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
TMP_REF="/tmp/p9_ref.fa"
OUT=/tmp/p9_debug_4reads

mkdir -p "$OUT"

echo "=== Step 1: Copy reference ==="
cp "$REF_SRC" "$TMP_REF"

echo "=== Step 2: Extract 4 problem reads ==="
for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398" "58512_chr22_tail_1M:772128-772202"; do
    base="${rid%%_*}"
    grep -A3 "^@$rid" "$FASTQ" > "$OUT/${base}.fastq" 2>/dev/null || echo "WARN: cannot find $rid"
    if [ -s "$OUT/${base}.fastq" ]; then
        echo "  $base: $(wc -l < "$OUT/${base}.fastq") lines"
    fi
done
cat "$OUT"/446.fastq "$OUT"/51599.fastq "$OUT"/677.fastq "$OUT"/58512.fastq > "$OUT/all4.fastq"

echo ""
echo "=== Step 3: Delete old .bsi and run P9 ==="
rm -f "$TMP_REF.bsi"
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d "$TMP_REF" \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_v5.sam" 2>/tmp/index_debug_v5.txt || true

echo ""
echo "=== Step 4: FIND_BEST_OFFSET ==="
grep 'FIND_BEST_OFFSET' /tmp/index_debug_v5.txt || echo "(none)"

echo ""
echo "=== Step 5: INDEX_DEBUG ==="
grep 'INDEX_DEBUG' /tmp/index_debug_v5.txt || echo "(none)"

echo ""
echo "=== Step 6: DEBUG_READ (first 30 lines) ==="
grep 'DEBUG_READ' /tmp/index_debug_v5.txt | head -30 || echo "(none)"

echo ""
echo "=== Step 7: HIT lines ==="
grep 'HIT:' /tmp/index_debug_v5.txt | head -30 || echo "(none)"

echo ""
echo "=== Step 8: DBG_ALL lines (first 40) ==="
grep 'DBG_ALL' /tmp/index_debug_v5.txt | head -40 || echo "(none)"

echo ""
echo "=== Step 9: SAM output ==="
cat "$OUT/all4_out_v5.sam"

echo ""
echo "=== Step 10: Read counts ==="
for rid in 446 51599 677 58512; do
    count=$(grep -c "^${rid}_" "$OUT/all4_out_v5.sam" 2>/dev/null || echo 0)
    echo "Read $rid: $count lines"
done
