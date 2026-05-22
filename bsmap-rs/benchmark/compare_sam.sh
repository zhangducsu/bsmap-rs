#!/bin/bash
OUT=/home/zhang_i5edc0/bsmap_benchmark/p11_15_20_bench

echo "=== SE wc -l ==="
wc -l "$OUT/cpp_se_p1.sam" "$OUT/rust_se_p1.sam" "$OUT/rust_se_p4.sam"

echo ""
echo "=== SE: Rust p=1 vs C++ p=1 diff ==="
diff <(grep -v "^@" "$OUT/cpp_se_p1.sam" | sort) <(grep -v "^@" "$OUT/rust_se_p1.sam" | sort) | wc -l

echo ""
echo "=== SE: Rust p=1 vs Rust p=4 diff ==="
diff <(grep -v "^@" "$OUT/rust_se_p1.sam" | sort) <(grep -v "^@" "$OUT/rust_se_p4.sam" | sort) | wc -l

echo ""
echo "=== PE wc -l ==="
wc -l "$OUT/rust_pe_p1.sam" "$OUT/rust_pe_p4.sam"

echo ""
echo "=== PE: Rust p=1 vs p=4 diff ==="
diff <(grep -v "^@" "$OUT/rust_pe_p1.sam" | sort) <(grep -v "^@" "$OUT/rust_pe_p4.sam" | sort) | wc -l
