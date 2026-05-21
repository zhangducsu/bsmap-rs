#!/bin/bash
set -e

CPP="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap"
OUT=/tmp/p9_debug_4reads

echo "=== Run C++ bsmap on 4 reads ==="
"$CPP" -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d /tmp/p9_ref.fa \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_cpp.sam" 2>/tmp/cpp_stderr.txt

echo ""
echo "=== C++ stderr ==="
cat /tmp/cpp_stderr.txt

echo ""
echo "=== C++ SAM output ==="
cat "$OUT/all4_cpp.sam"

echo ""
echo "=== grep for each read ==="
for rid in 446 51599 677 58512; do
    count=$(grep -c "^${rid}_" "$OUT/all4_cpp.sam" 2>/dev/null || echo 0)
    echo "Read $rid: $count lines"
done
