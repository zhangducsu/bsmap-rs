#!/bin/bash
set -e

DATA=/home/zhang_i5edc0/bsmap_benchmark/data
REF=$DATA/ref/chr22_tail_1M.fa
BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
OUTDIR=/tmp/bsmap_bench_p11_18_19
mkdir -p $OUTDIR

echo "=== Rust SE p=1 ==="
/usr/bin/time -v $BIN align \
  -a $DATA/wgbs/ex1_se75_10x/simulated.fastq.gz \
  -d $REF \
  -o $OUTDIR/se_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1

echo ""
echo "=== Rust SE p=4 ==="
/usr/bin/time -v $BIN align \
  -a $DATA/wgbs/ex1_se75_10x/simulated.fastq.gz \
  -d $REF \
  -o $OUTDIR/se_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4

echo ""
echo "=== Rust PE p=1 ==="
/usr/bin/time -v $BIN align \
  -a $DATA/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
  -b $DATA/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
  -d $REF \
  -o $OUTDIR/pe_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1

echo ""
echo "=== Rust PE p=4 ==="
/usr/bin/time -v $BIN align \
  -a $DATA/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
  -b $DATA/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
  -d $REF \
  -o $OUTDIR/pe_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4

echo ""
echo "=== SAM stats ==="
echo "--- SE p=1 ---"
grep -c '^@' $OUTDIR/se_p1.sam 2>/dev/null || echo "header count unknown"
grep -v '^@' $OUTDIR/se_p1.sam | wc -l

echo "--- SE p=4 ---"
grep -v '^@' $OUTDIR/se_p4.sam | wc -l

echo "--- PE p=1 ---"
grep -v '^@' $OUTDIR/pe_p1.sam | wc -l

echo "--- PE p=4 ---"
grep -v '^@' $OUTDIR/pe_p4.sam | wc -l

echo ""
echo "=== Diff vs P11-12~14 (previous version) ==="
echo "SE p=1 diff:"
# Compare with previous SAM if available
echo "SE p=1 vs SE p=4 diff:"
diff <(grep -v '^@' $OUTDIR/se_p1.sam | sort) <(grep -v '^@' $OUTDIR/se_p4.sam | sort) | wc -l
echo "PE p=1 vs PE p=4 diff:"
diff <(grep -v '^@' $OUTDIR/pe_p1.sam | sort) <(grep -v '^@' $OUTDIR/pe_p4.sam | sort) | wc -l

echo "=== Done ==="
