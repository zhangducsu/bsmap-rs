#!/bin/bash
# BSMAP-rs P6 基准测试 - WSL2 直接执行版本

set -e

# 配置
PROJECT_ROOT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs"
DATA_DIR="$PROJECT_ROOT/benchmark/data"
RESULTS_DIR="$PROJECT_ROOT/benchmark/results_p6_final"
BSMAP_RS="$PROJECT_ROOT/target/release/bsmap"
BSMAP_CPP="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap"

# 创建结果目录
mkdir -p "$RESULTS_DIR"/{single,multi,sam_compare,profile}

cd "$PROJECT_ROOT"

echo "=========================================="
echo "BSMAP-rs P6 基准测试开始"
echo "=========================================="
echo ""

# 1. 检查环境
echo "[1/7] 检查环境..."

if [ -f "$BSMAP_RS" ]; then
    echo "✓ BSMAP-rs 二进制文件存在"
else
    echo "✗ BSMAP-rs 不存在，开始编译..."
    RUSTFLAGS='-C target-cpu=native' cargo build --release
fi

echo ""

# 2. Ex1 SE 单线程测试
echo "[2/7] Ex1 SE 单线程测试..."
EX1_START=$(date +%s.%N)
timeout 600 "$BSMAP_RS" \
    -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 1 \
    -o "$RESULTS_DIR/single/ex1_se_rust.sam" > "$RESULTS_DIR/single/ex1_se_rust.log" 2>&1 || true
EX1_END=$(date +%s.%N)
EX1_TIME=$(echo "$EX1_END - $EX1_START" | bc)
echo "✓ Ex1 SE 完成: ${EX1_TIME}s"

if [ -f "$RESULTS_DIR/single/ex1_se_rust.sam" ]; then
    EX1_TOTAL=$(grep -v "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" | wc -l)
    EX1_UNIQUE=$(grep -v "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" | grep -c "XS:i:0")
    echo "  总读段: $EX1_TOTAL, 唯一比对: $EX1_UNIQUE"
fi

echo ""

# 3. Ex2 PE 单线程测试
echo "[3/7] Ex2 PE 单线程测试..."
EX2_START=$(date +%s.%N)
timeout 600 "$BSMAP_RS" \
    -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
    -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 1 \
    -o "$RESULTS_DIR/single/ex2_pe_rust.sam" > "$RESULTS_DIR/single/ex2_pe_rust.log" 2>&1 || true
EX2_END=$(date +%s.%N)
EX2_TIME=$(echo "$EX2_END - $EX2_START" | bc)
echo "✓ Ex2 PE 完成: ${EX2_TIME}s"

if [ -f "$RESULTS_DIR/single/ex2_pe_rust.sam" ]; then
    EX2_TOTAL=$(grep -v "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" | wc -l)
    EX2_UNIQUE=$(grep -v "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" | grep -c "XS:i:0")
    echo "  总读段: $EX2_TOTAL, 唯一比对: $EX2_UNIQUE"
fi

echo ""

# 4. Ex1 SE 4线程测试
echo "[4/7] Ex1 SE 4线程测试..."
EX1_4T_START=$(date +%s.%N)
timeout 600 "$BSMAP_RS" \
    -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 4 \
    -o "$RESULTS_DIR/multi/ex1_se_rust_4t.sam" > "$RESULTS_DIR/multi/ex1_se_rust_4t.log" 2>&1 || true
EX1_4T_END=$(date +%s.%N)
EX1_4T_TIME=$(echo "$EX1_4T_END - $EX1_4T_START" | bc)
echo "✓ Ex1 SE 4线程完成: ${EX1_4T_TIME}s"

echo ""

# 5. Ex2 PE 4线程测试
echo "[5/7] Ex2 PE 4线程测试..."
EX2_4T_START=$(date +%s.%N)
timeout 600 "$BSMAP_RS" \
    -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
    -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 4 \
    -o "$RESULTS_DIR/multi/ex2_pe_rust_4t.sam" > "$RESULTS_DIR/multi/ex2_pe_rust_4t.log" 2>&1 || true
EX2_4T_END=$(date +%s.%N)
EX2_4T_TIME=$(echo "$EX2_4T_END - $EX2_4T_START" | bc)
echo "✓ Ex2 PE 4线程完成: ${EX2_4T_TIME}s"

echo ""

# 6. C++ BSMAP对比（如果可用）
echo "[6/7] C++ BSMAP对比测试..."
if [ -f "$BSMAP_CPP" ]; then
    echo "  C++ BSMAP 存在，执行对比测试..."

    # Ex1 SE C++
    EX1_CPP_START=$(date +%s.%N)
    timeout 600 "$BSMAP_CPP" \
        -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
        -d "$DATA_DIR/chr22_tail_1M.fa" \
        -p 1 \
        -o "$RESULTS_DIR/single/ex1_se_cpp.sam" > "$RESULTS_DIR/single/ex1_se_cpp.log" 2>&1 || true
    EX1_CPP_END=$(date +%s.%N)
    EX1_CPP_TIME=$(echo "$EX1_CPP_END - $EX1_CPP_START" | bc)
    echo "  ✓ Ex1 SE C++ 完成: ${EX1_CPP_TIME}s"

    # Ex2 PE C++
    EX2_CPP_START=$(date +%s.%N)
    timeout 600 "$BSMAP_CPP" \
        -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
        -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
        -d "$DATA_DIR/chr22_tail_1M.fa" \
        -p 1 \
        -o "$RESULTS_DIR/single/ex2_pe_cpp.sam" > "$RESULTS_DIR/single/ex2_pe_cpp.log" 2>&1 || true
    EX2_CPP_END=$(date +%s.%N)
    EX2_CPP_TIME=$(echo "$EX2_CPP_END - $EX2_CPP_START" | bc)
    echo "  ✓ Ex2 PE C++ 完成: ${EX2_CPP_TIME}s"

else
    echo "  未找到C++ BSMAP，跳过对比测试"
fi

echo ""

# 7. 生成测试报告
echo "[7/7] 生成测试报告..."

cat > "$RESULTS_DIR/P6_BENCHMARK_SUMMARY.md" << EOF
# BSMAP-rs P6 基准测试结果

**测试日期**: $(date '+%Y-%m-%d %H:%M:%S')
**测试环境**: WSL2 Ubuntu
**测试版本**: P6 (P0-P6完整优化)

---

## 一、性能测试结果

### 1.1 执行时间

| 测试用例 | 线程数 | 执行时间 |
|---------|--------|---------|
| Ex1 SE 75bp | 1 | ${EX1_TIME}s |
| Ex2 PE 150bp | 1 | ${EX2_TIME}s |
| Ex1 SE 75bp | 4 | ${EX1_4T_TIME}s |
| Ex2 PE 150bp | 4 | ${EX2_4T_TIME}s |
EOF

if [ -f "$RESULTS_DIR/single/ex1_se_cpp.log" ] && [ -f "$RESULTS_DIR/single/ex2_pe_cpp.log" ]; then
cat >> "$RESULTS_DIR/P6_BENCHMARK_SUMMARY.md" << EOF

### 1.2 与C++ BSMAP对比

| 测试用例 | C++ BSMAP | BSMAP-rs | 性能提升 |
|---------|-----------|----------|----------|
| Ex1 SE 75bp | ${EX1_CPP_TIME}s | ${EX1_TIME}s | TBD |
| Ex2 PE 150bp | ${EX2_CPP_TIME}s | ${EX2_TIME}s | TBD |

EOF
fi

cat >> "$RESULTS_DIR/P6_BENCHMARK_SUMMARY.md" << EOF

## 二、比对统计

### 2.1 Ex1 SE 75bp

| 指标 | 数值 |
|------|------|
| 总读段数 | ${EX1_TOTAL} |
| 唯一比对数 | ${EX1_UNIQUE} |
| 比对率 | TBD% |

### 2.2 Ex2 PE 150bp

| 指标 | 数值 |
|------|------|
| 总读段数 | ${EX2_TOTAL} |
| 唯一比对数 | ${EX2_UNIQUE} |
| 比对率 | TBD% |

---

## 三、结果文件

- 单线程结果: $RESULTS_DIR/single/
- 多线程结果: $RESULTS_DIR/multi/
- 详细日志: *.log
- SAM文件: *.sam

EOF

echo "✓ 测试报告已生成: $RESULTS_DIR/P6_BENCHMARK_SUMMARY.md"
echo ""

echo "=========================================="
echo "基准测试完成！"
echo "=========================================="
