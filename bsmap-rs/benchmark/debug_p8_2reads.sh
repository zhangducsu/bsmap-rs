#!/bin/bash
# 提取两条漏比对的 read 并单独调试
set -e

# 提取两条 read 到单独文件
READS_DIR="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/tmp"
grep -A 3 '^@446_chr22_tail' "$READS_DIR/ex1_se75_10x.fastq" > /tmp/debug_2reads.fastq
echo "" >> /tmp/debug_2reads.fastq
grep -A 3 '^@58512_chr22_tail' "$READS_DIR/ex1_se75_10x.fastq" >> /tmp/debug_2reads.fastq

echo "=== 提取的 read 序列 ==="
cat /tmp/debug_2reads.fastq

echo ""
echo "=== C++ BSMAP 的比对结果 ==="
grep '^446_chr22_tail\|^58512_chr22_tail' /mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/results_p7_20260521_055015/cpp/p1/ex1_se_cpp.sam

echo ""
echo "=== 运行 bsmap-rs (P8) ==="
REF="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
BSMAP_RS="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
$BSMAP_RS align -a /tmp/debug_2reads.fastq -d "$REF" -o /tmp/debug_2reads.sam -s 16 -v 0.08 -I 4 -p 1 --verbose 2 2>&1

echo ""
echo "=== bsmap-rs SAM 输出 ==="
grep -v '^@' /tmp/debug_2reads.sam

echo ""
echo "=== 参考序列在 772041-772202 范围 ==="
python3 -c "
with open('$REF') as f:
    lines = f.readlines()
    seq = ''.join(line.strip() for line in lines if not line.startswith('>'))
    print('chr22_tail_1M:772041-772202:')
    # 显示 read 446 区域 (772041-772115) 的参考序列
    print('  772041-772115:', seq[772041-1:772115])
    # 显示 read 58512 区域 (772128-772202) 的参考序列
    print('  772128-772202:', seq[772128-1:772202])
"
