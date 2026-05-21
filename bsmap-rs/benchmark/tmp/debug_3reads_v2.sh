#!/bin/bash
set -e

FASTQ="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
C_SAM="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"
TMP_REF="/tmp/p9_ref.fa"
BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"

R1="446_chr22_tail_1M:772041-772115"
R2="51599_chr22_tail_1M:952915-952989"
R3="677_chr22_tail_1M:946324-946398"

for rid in "$R1" "$R2" "$R3"; do
    base="${rid%%_*}"
    echo ""
    echo "========== Read: $rid =========="

    # C++
    echo "--- C++ SAM ---"
    grep "^${rid}" "$C_SAM" 2>/dev/null | head -3

    # Extract FASTQ (1 line header + 1 seq + 1 "+" + 1 qual)
    grep -A3 "^@${rid}" "$FASTQ" > "/tmp/single_${base}.fastq" 2>/dev/null || true

    if [ -s "/tmp/single_${base}.fastq" ]; then
        head -4 "/tmp/single_${base}.fastq"
        "$BIN" align \
            -s 16 -v 0.08 -I 4 -p 1 -r 2 \
            -d "$TMP_REF" \
            -a "/tmp/single_${base}.fastq" \
            -o "/tmp/single_${base}_out.sam" 2>/dev/null

        echo "--- P9 SAM (all hits, -r 2) ---"
        grep -v '^@' "/tmp/single_${base}_out.sam" 2>/dev/null | head -20 || echo "(no hits)"
    else
        echo "ERROR: cannot extract read"
    fi
done
