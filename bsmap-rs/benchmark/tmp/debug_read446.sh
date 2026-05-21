#!/bin/bash
set -e
FASTQ="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
TMP_REF="/tmp/p9_ref.fa"
C_SAM="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"

RID="446_chr22_tail_1M:772041-772115"
BASE="446"

echo "=== C++ hits ==="
grep "^${RID}" "${C_SAM}" | head -5

echo ""
echo "=== Extracting FASTQ ==="
grep -A3 "^@${RID}" "${FASTQ}" > "/tmp/single_${BASE}.fastq"
wc -l "/tmp/single_${BASE}.fastq"
head -4 "/tmp/single_${BASE}.fastq"

echo ""
echo "=== Running P9 ==="
"${BIN}" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d "${TMP_REF}" \
    -a "/tmp/single_${BASE}.fastq" \
    -o "/tmp/single_${BASE}_out.sam" 2>/dev/null

echo ""
echo "=== P9 hits ==="
grep -v '^@' "/tmp/single_${BASE}_out.sam"
