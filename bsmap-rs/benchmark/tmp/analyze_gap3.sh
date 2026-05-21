#!/bin/bash
set -e

C_SAM="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"
P9_SAM="/tmp/p9_test_ex1/ex1_se_p9.sam"
OUT="/tmp/gap_analysis"
mkdir -p "$OUT"

# Extract unique reads from C++ (flag 0 or 16)
grep -v '^@' "$C_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > "$OUT/c_unique.txt"
# Extract multiple reads from P9 (flag 256 or 272)
grep -v '^@' "$P9_SAM" | awk '{flag=$2; if(flag==256||flag==272) print $1}' | sort > "$OUT/p9_multi.txt"
# Find C++ unique that became P9 multiple
comm -12 "$OUT/c_unique.txt" "$OUT/p9_multi.txt" > "$OUT/uniq_to_multi.txt"

echo "=== C++ unique -> P9 multiple: $(wc -l < "$OUT/uniq_to_multi.txt") reads ==="
for read_id in $(cat "$OUT/uniq_to_multi.txt"); do
    echo ""
    echo "--- Read: $read_id ---"
    echo "C++ output:"
    grep "^$read_id" "$C_SAM" | head -5
    echo "P9 output:"
    grep "^$read_id" "$P9_SAM" | head -5
done
