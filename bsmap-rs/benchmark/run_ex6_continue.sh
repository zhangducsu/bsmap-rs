#!/bin/bash
# Continue benchmarks for ex6 and handle C++ PE crashes gracefully

DATA=/home/zhang_i5edc0/bsmap_benchmark/data
REF=$DATA/ref/chr22_tail_1M.fa
RUST_BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
CPP_BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
OUTDIR=/tmp/bsmap_bench_ex3_ex4_ex6

RRBS_DATA=$DATA/rrbs/rrbssim
EX6_FQ1=$RRBS_DATA/ex6_pe150_20x.1.fq.gz
EX6_FQ2=$RRBS_DATA/ex6_pe150_20x.2.fq.gz

echo "=========================================="
echo "= Continue: ex6 (RRBS PE 150bp, 29k pairs)"
echo "=========================================="

echo ""
echo "=== Rust PE p=1 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX6_FQ1 -b $EX6_FQ2 -d $REF -o $OUTDIR/ex6_pe_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== Rust PE p=4 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX6_FQ1 -b $EX6_FQ2 -d $REF -o $OUTDIR/ex6_pe_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=== C++ PE p=1 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX6_FQ1 -b $EX6_FQ2 -d $REF -o $OUTDIR/ex6_cpp_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 || echo "(C++ PE p=1 crashed as expected - buffer overflow in original BSMAP with this data)"

echo ""
echo "=== C++ PE p=4 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX6_FQ1 -b $EX6_FQ2 -d $REF -o $OUTDIR/ex6_cpp_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1 || echo "(C++ PE p=4 crashed as expected)"

echo ""
echo "=========================================="
echo "= SAM Stats & Comparison"
echo "=========================================="

echo "--- SAM file sizes ---"
ls -la $OUTDIR/*.sam 2>/dev/null

echo ""
echo "--- ex3 SAM stats ---"
for f in $OUTDIR/ex3_*.sam; do
  name=$(basename $f)
  total=$(grep -v '^@' "$f" | wc -l)
  echo "  $name: $total alignments"
done

echo ""
echo "--- ex4 SAM stats ---"
for f in $OUTDIR/ex4_*.sam; do
  name=$(basename $f)
  total=$(grep -v '^@' "$f" | wc -l)
  echo "  $name: $total alignments"
done

echo ""
echo "--- ex6 SAM stats ---"
for f in $OUTDIR/ex6_*.sam; do
  name=$(basename $f)
  if [ ! -s "$f" ]; then
    echo "  $name: EMPTY (crashed)"
  else
    total=$(grep -v '^@' "$f" | wc -l)
    echo "  $name: $total alignments"
  fi
done

echo ""
echo "--- Cross-version diff ---"
echo "ex3: Rust p=1 vs C++ p=1:"
diff <(grep -v '^@' $OUTDIR/ex3_se_p1.sam | sort) \
     <(grep -v '^@' $OUTDIR/ex3_cpp_p1.sam | sort) | wc -l

echo "ex3: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' $OUTDIR/ex3_se_p1.sam | sort) \
     <(grep -v '^@' $OUTDIR/ex3_se_p4.sam | sort) | wc -l

echo "ex4: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' $OUTDIR/ex4_pe_p1.sam | sort) \
     <(grep -v '^@' $OUTDIR/ex4_pe_p4.sam | sort) | wc -l

echo "ex6: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' $OUTDIR/ex6_pe_p1.sam | sort) \
     <(grep -v '^@' $OUTDIR/ex6_pe_p4.sam | sort) | wc -l

echo ""
echo "=== Done ==="
