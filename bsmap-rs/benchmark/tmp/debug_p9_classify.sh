#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
TMP_REF="/tmp/p9_ref.fa"
FASTQ="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
C_SAM="$PROJECT/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"
OUT="/tmp/p9_classify"

mkdir -p "$OUT"

echo "=== Step 1: Run P9 ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 \
    -d "$TMP_REF" \
    -a "$FASTQ" \
    -o "$OUT/ex1_se_p9.sam" 2>/dev/null

echo "P9 SAM lines: $(grep -cv '^@' "$OUT/ex1_se_p9.sam")"

echo ""
echo "=== Step 2: Find C++ unique -> P9 multi ==="
# C++ unique: flag 0 (++) or 16 (+-)
# P9 multi: flag 256 (+-) or 272 (++)
grep -v '^@' "$C_SAM" | awk '{flag=$2+0; if(flag==0||flag==16) print $1}' | sort -u > "$OUT/c_uniq.txt"
grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{flag=$2+0; if(flag==256||flag==272) print $1}' | sort -u > "$OUT/p9_multi.txt"
comm -12 "$OUT/c_uniq.txt" "$OUT/p9_multi.txt" > "$OUT/uniq_to_multi.txt"

N=$(wc -l < "$OUT/uniq_to_multi.txt")
echo "C++ unique -> P9 multi: $N reads"

if [ "$N" -gt 0 ]; then
    echo ""
    echo "=== Step 3: Detail each read ==="
    while IFS= read -r rid; do
        echo ""
        echo "--- Read: $rid ---"
        echo "C++:"
        grep "^$rid" "$C_SAM" | head -5
        echo "P9:"
        grep "^$rid" "$OUT/ex1_se_p9.sam" | head -5
    done < "$OUT/uniq_to_multi.txt"
fi

echo ""
echo "=== Step 4: Find P9 missing vs C++ ==="
grep -v '^@' "$C_SAM" | awk '{print $1}' | sort -u > "$OUT/c_all.txt"
grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{print $1}' | sort -u > "$OUT/p9_all.txt"
comm -23 "$OUT/c_all.txt" "$OUT/p9_all.txt" > "$OUT/c_only.txt"
echo "C++ only reads: $(wc -l < "$OUT/c_only.txt")"
if [ -s "$OUT/c_only.txt" ]; then
    head -10 "$OUT/c_only.txt"
fi
