#!/bin/bash
set -e

DATA=/home/zhang_i5edc0/bsmap_benchmark/data
REF=$DATA/ref/chr22_tail_1M.fa
BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
OUTDIR=/tmp/bsmap_bench_cpp
mkdir -p $OUTDIR

# Decompress reads if needed (C++ BSMAP may not support .gz)
if [ ! -f /tmp/se75_10x.fastq ]; then
  echo "Decompressing example data..."
  gunzip -c $DATA/wgbs/ex1_se75_10x/simulated.fastq.gz > /tmp/se75_10x.fastq
  gunzip -c $DATA/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > /tmp/pe150_10x_1.fastq
  gunzip -c $DATA/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > /tmp/pe150_10x_2.fastq
fi

echo "=== C++ SE p=1 ==="
/usr/bin/time -v $BIN \
  -a /tmp/se75_10x.fastq \
  -d $REF \
  -o $OUTDIR/se_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1

echo ""
echo "=== C++ SE p=4 ==="
/usr/bin/time -v $BIN \
  -a /tmp/se75_10x.fastq \
  -d $REF \
  -o $OUTDIR/se_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4

echo ""
echo "=== C++ PE p=1 ==="
/usr/bin/time -v $BIN \
  -a /tmp/pe150_10x_1.fastq \
  -b /tmp/pe150_10x_2.fastq \
  -d $REF \
  -o $OUTDIR/pe_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 || echo "(Expected: C++ PE crashes on this data)"

echo ""
echo "=== SAM stats ==="
echo "SE p=1 lines: $(grep -v '^@' $OUTDIR/se_p1.sam 2>/dev/null | wc -l)"
echo "SE p=4 lines: $(grep -v '^@' $OUTDIR/se_p4.sam 2>/dev/null | wc -l)"
echo "PE p=1 lines: $(grep -v '^@' $OUTDIR/pe_p1.sam 2>/dev/null | wc -l)"

echo ""
echo "=== Done ==="
