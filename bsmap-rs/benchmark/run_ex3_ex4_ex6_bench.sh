#!/bin/bash
set -e

DATA=/home/zhang_i5edc0/bsmap_benchmark/data
REF=$DATA/ref/chr22_tail_1M.fa
RUST_BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap
CPP_BIN=/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-original/bsmap-2.90/bsmap
OUTDIR=/tmp/bsmap_bench_ex3_ex4_ex6
mkdir -p $OUTDIR

RRBS_DATA=$DATA/rrbs/rrbssim
EX3_FQ=$RRBS_DATA/ex3_se75_10x.1.fq.gz
EX4_FQ1=$RRBS_DATA/ex4_pe150_10x.1.fq.gz
EX4_FQ2=$RRBS_DATA/ex4_pe150_10x.2.fq.gz
EX6_FQ1=$RRBS_DATA/ex6_pe150_20x.1.fq.gz
EX6_FQ2=$RRBS_DATA/ex6_pe150_20x.2.fq.gz

# Also find the previous P version binary (P11-12~14 or P11-18~19)
# For now, compare against the current binary (P11-18~19) as both are Rust

echo "=========================================="
echo "= Benchmark: ex3 (RRBS SE 75bp, 13k reads)"
echo "=========================================="

echo ""
echo "=== Rust SE p=1 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX3_FQ -d $REF -o $OUTDIR/ex3_se_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== Rust SE p=4 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX3_FQ -d $REF -o $OUTDIR/ex3_se_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=== C++ SE p=1 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX3_FQ -d $REF -o $OUTDIR/ex3_cpp_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== C++ SE p=4 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX3_FQ -d $REF -o $OUTDIR/ex3_cpp_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=========================================="
echo "= Benchmark: ex4 (RRBS PE 150bp, 14k pairs)"
echo "=========================================="

echo ""
echo "=== Rust PE p=1 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX4_FQ1 -b $EX4_FQ2 -d $REF -o $OUTDIR/ex4_pe_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== Rust PE p=4 ==="
/usr/bin/time -v "$RUST_BIN" align \
  -a $EX4_FQ1 -b $EX4_FQ2 -d $REF -o $OUTDIR/ex4_pe_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=== C++ PE p=1 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX4_FQ1 -b $EX4_FQ2 -d $REF -o $OUTDIR/ex4_cpp_p1.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== C++ PE p=4 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX4_FQ1 -b $EX4_FQ2 -d $REF -o $OUTDIR/ex4_cpp_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=========================================="
echo "= Benchmark: ex6 (RRBS PE 150bp, 29k pairs)"
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
  -s 16 -v 0.08 -I 4 -p 1 2>&1

echo ""
echo "=== C++ PE p=4 ==="
/usr/bin/time -v "$CPP_BIN" \
  -a $EX6_FQ1 -b $EX6_FQ2 -d $REF -o $OUTDIR/ex6_cpp_p4.sam \
  -s 16 -v 0.08 -I 4 -p 4 2>&1

echo ""
echo "=========================================="
echo "= SAM Stats & Comparison"
echo "=========================================="

sam_stat() {
  local label="$1"
  local file="$2"
  if [ ! -f "$file" ]; then
    echo "  $label: FILE NOT FOUND"
    return
  fi
  local total=$(grep -v '^@' "$file" | wc -l)
  local unique=$(awk -F'\t' '! /^@/ {if ($5 == 255) u++; else if ($5 > 0 && $5 < 255) m++} END {print u+0, m+0}' "$file")
  echo "  $label: total=${total} ${unique}"
}

echo "--- SAM stats ---"
echo ""
echo "ex3 (SE 75bp):"
sam_stat "Rust p=1" "$OUTDIR/ex3_se_p1.sam"
sam_stat "Rust p=4" "$OUTDIR/ex3_se_p4.sam"
sam_stat "C++  p=1" "$OUTDIR/ex3_cpp_p1.sam"
sam_stat "C++  p=4" "$OUTDIR/ex3_cpp_p4.sam"

echo ""
echo "ex4 (PE 150bp):"
sam_stat "Rust p=1" "$OUTDIR/ex4_pe_p1.sam"
sam_stat "Rust p=4" "$OUTDIR/ex4_pe_p4.sam"
sam_stat "C++  p=1" "$OUTDIR/ex4_cpp_p1.sam"
sam_stat "C++  p=4" "$OUTDIR/ex4_cpp_p4.sam"

echo ""
echo "ex6 (PE 150bp):"
sam_stat "Rust p=1" "$OUTDIR/ex6_pe_p1.sam"
sam_stat "Rust p=4" "$OUTDIR/ex6_pe_p4.sam"
sam_stat "C++  p=1" "$OUTDIR/ex6_cpp_p1.sam"
sam_stat "C++  p=4" "$OUTDIR/ex6_cpp_p4.sam"

echo ""
echo "--- Cross-version diff (non-header lines) ---"
echo ""
echo "ex3: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' "$OUTDIR/ex3_se_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex3_se_p4.sam" | sort) | wc -l
echo "ex3: Rust p=1 vs C++ p=1:"
diff <(grep -v '^@' "$OUTDIR/ex3_se_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex3_cpp_p1.sam" | sort) | wc -l

echo ""
echo "ex4: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' "$OUTDIR/ex4_pe_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex4_pe_p4.sam" | sort) | wc -l
echo "ex4: Rust p=1 vs C++ p=1:"
diff <(grep -v '^@' "$OUTDIR/ex4_pe_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex4_cpp_p1.sam" | sort) | wc -l

echo ""
echo "ex6: Rust p=1 vs Rust p=4:"
diff <(grep -v '^@' "$OUTDIR/ex6_pe_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex6_pe_p4.sam" | sort) | wc -l
echo "ex6: Rust p=1 vs C++ p=1:"
diff <(grep -v '^@' "$OUTDIR/ex6_pe_p1.sam" | sort) \
     <(grep -v '^@' "$OUTDIR/ex6_cpp_p1.sam" | sort) | wc -l

echo ""
echo "=== Done ==="
