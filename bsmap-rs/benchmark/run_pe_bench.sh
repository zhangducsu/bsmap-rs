#!/bin/bash
BENCH=/home/zhang_i5edc0/bsmap_benchmark
D=$BENCH/data
REF=$D/chr22_tail_1M.fa
CPP=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
RUST=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
OUT=$BENCH/p11_15_20_bench
mkdir -p $OUT

echo "=== C++ PE p=1 (known to crash) ==="
time $CPP -a $D/wgbs/ex2_pe150_10x/simulated_1.fastq.gz -b $D/wgbs/ex2_pe150_10x/simulated_2.fastq.gz -d $REF -o $OUT/cpp_pe_p1.sam -s 16 -v 0.08 -I 4 -p 1 2>&1

echo "=== Rust PE p=1 ==="
time $RUST align -a $D/wgbs/ex2_pe150_10x/simulated_1.fastq.gz -b $D/wgbs/ex2_pe150_10x/simulated_2.fastq.gz -d $REF -o $OUT/rust_pe_p1.sam -s 16 -v 0.08 -I 4 -p 1 2>&1

echo "=== Rust PE p=4 ==="
time $RUST align -a $D/wgbs/ex2_pe150_10x/simulated_1.fastq.gz -b $D/wgbs/ex2_pe150_10x/simulated_2.fastq.gz -d $REF -o $OUT/rust_pe_p4.sam -s 16 -v 0.08 -I 4 -p 4 2>&1

echo "=== PE DONE ==="
