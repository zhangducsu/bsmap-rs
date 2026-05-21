#!/bin/bash
set -e

BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
OUT=/tmp/p9_debug_4reads

echo "=== Build ==="
cd /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs
/home/zhang_i5edc0/.cargo/bin/cargo build --release -p bsmap 2>&1 | tail -5

echo ""
echo "=== Delete old .bsi and run ==="
rm -f /tmp/p9_ref.fa.bsi
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d /tmp/p9_ref.fa \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_v4.sam" 2>/tmp/index_debug_v4.txt

echo ""
echo "=== INDEX_DEBUG lines ==="
grep 'INDEX_DEBUG' /tmp/index_debug_v4.txt || echo "(none)"

echo ""
echo "=== SAM output ==="
cat "$OUT/all4_out_v4.sam"

echo ""
echo "=== Read counts ==="
for rid in 446 51599 677 58512; do
    count=$(grep -c "^${rid}_" "$OUT/all4_out_v4.sam" 2>/dev/null || echo 0)
    echo "Read $rid: $count lines"
done

echo ""
echo "=== C++ comparison ==="
echo "C++ reads 58512:"
grep "^58512_" "$OUT/all4_cpp.sam" 2>/dev/null || echo "(none)"
