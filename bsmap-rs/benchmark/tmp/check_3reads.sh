#!/bin/bash
P9_SAM="/tmp/p9_test_ex1/ex1_se_p9.sam"
C_SAM="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p8_20260521_074711/comparison/ex1_se_p1/cpp_sorted.sam"

for rid in "446_chr22_tail_1M:772041-772115" "51599_chr22_tail_1M:952915-952989" "677_chr22_tail_1M:946324-946398"; do
    echo "=== All P9 hits for: $rid ==="
    grep "^$rid" "$P9_SAM" || echo "(none)"
    echo ""
    echo "=== All C++ hits for: $rid ==="
    grep "^$rid" "$C_SAM" || echo "(none)"
    echo ""
    echo "---"
    echo ""
done
