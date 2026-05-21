#!/bin/bash
# 找出 C++ unique → P9 multiple 的差异 read
C_SAM="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/cpp/p1/ex1_se_cpp.sam"
P9_SAM="/tmp/ex1_se_p9_test.sam"

echo "=== 提取 FLAG 分类 ==="

# C++ unique reads (flag 0 or 16)
grep -v '^@' "$C_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > /tmp/c_unique.txt

# P9 unique reads
grep -v '^@' "$P9_SAM" | awk '{flag=$2; if(!(flag+0)||flag==16) print $1}' | sort > /tmp/p9_unique.txt

# P9 multiple reads
grep -v '^@' "$P9_SAM" | awk '{flag=$2; if(flag==256||flag==272) print $1}' | sort > /tmp/p9_multi.txt

echo "C++ unique: $(wc -l < /tmp/c_unique.txt)"
echo "P9 unique: $(wc -l < /tmp/p9_unique.txt)"
echo "P9 multiple: $(wc -l < /tmp/p9_multi.txt)"

# Reads in C++ unique but NOT in P9 unique = misclassified
comm -23 /tmp/c_unique.txt /tmp/p9_unique.txt > /tmp/diff_reads.txt
echo "C++ unique but P9 NOT unique: $(wc -l < /tmp/diff_reads.txt)"

# Of those, which are P9 multiple (confirming unique→multiple reclassification)
comm -12 /tmp/diff_reads.txt /tmp/p9_multi.txt > /tmp/c_unique_p9_multi.txt
echo "Confirmed C++ unique → P9 multiple: $(wc -l < /tmp/c_unique_p9_multi.txt)"

echo ""
echo "=== 前5条样本 ==="
head -5 /tmp/c_unique_p9_multi.txt

echo ""
echo "=== 样本 read 在 C++ SAM 中的行 ==="
for read in $(head -3 /tmp/c_unique_p9_multi.txt); do
    echo "--- $read (C++) ---"
    grep "^$read[[:space:]]" "$C_SAM" | head -1
    echo "--- $read (P9) ---"
    grep "^$read[[:space:]]" "$P9_SAM" | head -1
done
