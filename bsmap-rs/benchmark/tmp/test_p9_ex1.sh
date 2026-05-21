#!/bin/bash
set -e

PROJECT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP"
BIN="$PROJECT/bsmap-rs/target/release/bsmap"
REF_SRC="$PROJECT/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
READS="$PROJECT/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
OUT="/tmp/p9_test_ex1"

# Copy ref to ext4 for speed
TMP_REF="/tmp/p9_ref.fa"
TMP_BSI="/tmp/p9_ref.fa.bsi"

echo "=== Preparing ext4 data ==="
cp "$REF_SRC" "$TMP_REF"
if [ -f "$REF_SRC.bsi" ]; then
    cp "$REF_SRC.bsi" "$TMP_BSI" 2>/dev/null || true
fi

mkdir -p "$OUT"

echo "=== Rebuilding index ==="
rm -f "$TMP_BSI"
"$BIN" index -s 16 -I 4 -d "$TMP_REF" 2>&1

echo "=== Running P9 alignment ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 \
    -d "$TMP_REF" \
    -a "$READS" \
    -o "$OUT/ex1_se_p9.sam" 2>&1

echo ""
echo "=== FLAG distribution ==="
grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{print $2}' | sort | uniq -c | sort -rn

echo ""
TOTAL=$(grep -c -v '^@' "$OUT/ex1_se_p9.sam")
U0=$(grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{if($2==0) print}' | wc -l)
U16=$(grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{if($2==16) print}' | wc -l)
M256=$(grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{if($2==256) print}' | wc -l)
M272=$(grep -v '^@' "$OUT/ex1_se_p9.sam" | awk '{if($2==272) print}' | wc -l)
TOTAL_UNIQUE=$((U0 + U16))
TOTAL_MULTI=$((M256 + M272))

echo "Total aligned:     $TOTAL"
echo "Unique (flag 0):   $U0"
echo "Unique (flag 16):  $U16"
echo "Multi (flag 256):  $M256"
echo "Multi (flag 272):  $M272"
echo "Total unique:      $TOTAL_UNIQUE"
echo "Total multiple:    $TOTAL_MULTI"

# C++ baseline from P8 report
C_UNIQUE=64951
C_MULTI=1169
C_TOTAL=66120
echo ""
echo "=== vs C++ baseline ==="
echo "C++ total:     $C_TOTAL"
echo "C++ unique:    $C_UNIQUE"
echo "C++ multiple:  $C_MULTI"
echo ""
echo "P9 total:      $TOTAL (diff: $((TOTAL - C_TOTAL)))"
echo "P9 unique:     $TOTAL_UNIQUE (diff: $((TOTAL_UNIQUE - C_UNIQUE)))"
echo "P9 multiple:   $TOTAL_MULTI (diff: $((TOTAL_MULTI - C_MULTI)))"

# Also compare against P8 C++ SAM
C_SAM="$PROJECT/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"
if [ -f "$C_SAM" ]; then
    echo ""
    echo "=== Direct SAM comparison with C++ ==="
    P9_SORTED="$OUT/ex1_se_p9_sorted.sam"
    grep '^@' "$OUT/ex1_se_p9.sam" > "$P9_SORTED"
    grep -v '^@' "$OUT/ex1_se_p9.sam" | sort >> "$P9_SORTED"
    DIFF_COUNT=$(diff <(cut -f1-11 "$C_SAM") <(cut -f1-11 "$P9_SORTED") | grep -c '^[<>]' || true)
    echo "Diff lines: $DIFF_COUNT"

    # Count remaining unique→multiple misclassifications
    C_UNIQUE_READS="/tmp/c_unique_reads.txt"
    P9_UNIQUE_READS="/tmp/p9_unique_reads.txt"
    P9_MULTI_READS="/tmp/p9_multi_reads.txt"
    grep -v '^@' "$C_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > "$C_UNIQUE_READS"
    grep -v '^@' "$P9_SORTED" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > "$P9_UNIQUE_READS"
    grep -v '^@' "$P9_SORTED" | awk '{flag=$2; if(flag==256||flag==272) print $1}' | sort > "$P9_MULTI_READS"
    comm -23 "$C_UNIQUE_READS" "$P9_UNIQUE_READS" > /tmp/c_unique_not_p9.txt
    comm -12 /tmp/c_unique_not_p9.txt "$P9_MULTI_READS" > /tmp/c_uniq_p9_multi.txt
    echo "C++ unique → P9 multiple (remaining gap): $(wc -l < /tmp/c_uniq_p9_multi.txt)"
fi
