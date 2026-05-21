#!/bin/bash
set -e

BSMAP_RS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
REF="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
READS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/tmp/ex1_se75_10x.fastq"
OUT="/tmp/ex1_se_p8_verify.sam"

echo "=== P8 修复验证：Ex1 SE p=1 ==="
$BSMAP_RS align -a "$READS" -d "$REF" -o "$OUT" -s 16 -v 0.08 -I 4 -p 1 --verbose 2 2>&1 | tail -10

echo ""
echo "=== 检查之前漏掉的2条read ==="
grep -c '^446_chr22_tail\|^58512_chr22_tail' "$OUT" 2>/dev/null || echo "(未找到)"

echo ""
echo "=== P7 C++ SAM 中这2条read的FLAG ==="
grep '^446_chr22_tail\|^58512_chr22_tail' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/cpp/p1/ex1_se_cpp.sam | cut -f2

echo ""
echo "=== P8 Rust SAM 中这2条read的FLAG ==="
grep '^446_chr22_tail\|^58512_chr22_tail' "$OUT" | cut -f2

echo ""
echo "=== FLAG 分布 ==="
grep -v '^@' "$OUT" | cut -f2 | sort | uniq -c | sort -rn

echo ""
echo "=== 比对行数 ==="
echo "C++ P7:   $(grep -cv '^@' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/cpp/p1/ex1_se_cpp.sam)"
echo "Rust P8:  $(grep -cv '^@' "$OUT")"
echo "Rust P7:  $(grep -cv '^@' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/rust/p1/ex1_se_rust.sam)"

echo ""
echo "=== 多重比对 (FLAG 0x100, 不含 0x800) ==="
P8_MULTI_100=$(grep -v '^@' "$OUT" | awk '$2 & 256 && !($2 & 2048)' | wc -l)
P7_MULTI_100=$(grep -v '^@' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/rust/p1/ex1_se_rust.sam | awk '$2 & 256 && !($2 & 2048)' | wc -l)
CPP_MULTI_100=$(grep -v '^@' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/cpp/p1/ex1_se_cpp.sam | awk '$2 & 256 && !($2 & 2048)' | wc -l)
echo "P8 Rust (仅0x100): $P8_MULTI_100"
echo "P7 Rust (0x900/0x910): $P7_MULTI_100"
echo "C++ P7 (仅0x100): $CPP_MULTI_100"

echo ""
echo "=== 完成 ==="
