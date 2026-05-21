#!/bin/bash
BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
TMP_REF="/tmp/p9_ref.fa"
FASTQ="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"

"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -d "$TMP_REF" -a "$FASTQ" -o /dev/null 2>/tmp/debug_stderr.txt
echo "Exit: $?"
echo "Stderr lines:"
wc -l /tmp/debug_stderr.txt
echo "--- First 10 DEBUG lines ---"
grep "^DEBUG" /tmp/debug_stderr.txt | head -10
echo "--- All stderr ---"
head -30 /tmp/debug_stderr.txt
