#!/bin/bash
echo "=========================================="
echo "索引加载性能测试 (V3 格式)"
echo "=========================================="
echo ""

mkdir -p tmp results
rm -f data/chr22_tail_1M.fa.bsi

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq 2>/dev/null || true
fi

echo "> 构建 V3 格式索引"
start=$(date +%s)
/workspace/bsmap-rs/target/release/bsmap index -d data/chr22_tail_1M.fa -s 16 2>&1 | tee results/index_build_v3.log
end=$(date +%s)
build_time=$((end - start))
echo "构建耗时: $build_time 秒"
ls -lh data/chr22_tail_1M.fa.bsi

echo ""
echo "> 测试索引加载"
start2=$(date +%s)
timeout 300 /workspace/bsmap-rs/target/release/bsmap align -a tmp/ex1_se75_10x.fastq -d data/chr22_tail_1M.fa -o results/test_output.sam -s 16 -v 0.08 -I 4 -p 1 -v 2>&1 | tee results/index_load_test.log
end2=$(date +%s)
load_time=$((end2 - start2))
echo ""
echo "比对耗时: $load_time 秒"

echo ""
echo "> 索引相关日志:"
grep -i "index\|k-mer\|加载" results/index_load_test.log | head -10 || true

echo ""
echo "=========================================="
echo "测试完成！"
echo "=========================================="
