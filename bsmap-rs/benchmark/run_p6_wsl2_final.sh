#!/bin/bash
# BSMAP-rs P6 基准测试 - WSL2 最终版本（使用解压的测试数据）

set -e

# 配置
PROJECT_ROOT="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs"
DATA_DIR="$PROJECT_ROOT/benchmark/tmp"
RESULTS_DIR="$PROJECT_ROOT/benchmark/results_p6_final"
BSMAP_RS="$PROJECT_ROOT/target/release/bsmap"

# 创建结果目录
mkdir -p "$RESULTS_DIR"/{single,4threads}

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
"$BSMAP_RS" align \
    -a "$DATA_DIR/ex1_se75.fq" \
    -d "$PROJECT_ROOT/benchmark/data/chr22_tail_1M.fa" \
    -p 1 \
    -s 16 \
    -v 0.08 \
    -I 4 \
    -o "$RESULTS_DIR/single/ex1_se_rust.sam" \
    --verbose 2 2>&1 | tee "$RESULTS_DIR/single/ex1_se.log"
EX1_END=$(date +%s.%N)
EX1_TIME=$(echo "$EX1_END - $EX1_START" | bc)
echo "✓ Ex1 SE 单线程完成: ${EX1_TIME}s"
echo ""

# 3. Ex2 PE 单线程测试
echo "[3/7] Ex2 PE 单线程测试..."
EX2_START=$(date +%s.%N)
"$BSMAP_RS" align \
    -a "$DATA_DIR/ex2_pe150_1.fq" \
    -b "$DATA_DIR/ex2_pe150_2.fq" \
    -d "$PROJECT_ROOT/benchmark/data/chr22_tail_1M.fa" \
    -p 1 \
    -s 16 \
    -v 0.08 \
    -I 4 \
    -o "$RESULTS_DIR/single/ex2_pe_rust.sam" \
    --verbose 2 2>&1 | tee "$RESULTS_DIR/single/ex2_pe.log"
EX2_END=$(date +%s.%N)
EX2_TIME=$(echo "$EX2_END - $EX2_START" | bc)
echo "✓ Ex2 PE 单线程完成: ${EX2_TIME}s"
echo ""

# 4. Ex1 SE 4线程测试
echo "[4/7] Ex1 SE 4线程测试..."
EX1_4T_START=$(date +%s.%N)
"$BSMAP_RS" align \
    -a "$DATA_DIR/ex1_se75.fq" \
    -d "$PROJECT_ROOT/benchmark/data/chr22_tail_1M.fa" \
    -p 4 \
    -s 16 \
    -v 0.08 \
    -I 4 \
    -o "$RESULTS_DIR/4threads/ex1_se_rust.sam" \
    --verbose 2 2>&1 | tee "$RESULTS_DIR/4threads/ex1_se.log"
EX1_4T_END=$(date +%s.%N)
EX1_4T_TIME=$(echo "$EX1_4T_END - $EX1_4T_START" | bc)
echo "✓ Ex1 SE 4线程完成: ${EX1_4T_TIME}s"
echo ""

# 5. Ex2 PE 4线程测试
echo "[5/7] Ex2 PE 4线程测试..."
EX2_4T_START=$(date +%s.%N)
"$BSMAP_RS" align \
    -a "$DATA_DIR/ex2_pe150_1.fq" \
    -b "$DATA_DIR/ex2_pe150_2.fq" \
    -d "$PROJECT_ROOT/benchmark/data/chr22_tail_1M.fa" \
    -p 4 \
    -s 16 \
    -v 0.08 \
    -I 4 \
    -o "$RESULTS_DIR/4threads/ex2_pe_rust.sam" \
    --verbose 2 2>&1 | tee "$RESULTS_DIR/4threads/ex2_pe.log"
EX2_4T_END=$(date +%s.%N)
EX2_4T_TIME=$(echo "$EX2_4T_END - $EX2_4T_START" | bc)
echo "✓ Ex2 PE 4线程完成: ${EX2_4T_TIME}s"
echo ""

# 6. 收集统计信息
echo "[6/7] 收集统计信息..."

# Ex1 SE统计
if [ -f "$RESULTS_DIR/single/ex1_se_rust.sam" ]; then
    EX1_TOTAL=$(grep -v "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" | wc -l)
    EX1_UNIQUE=$(grep -v "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" | grep -c "XS:i:0")
fi

# Ex2 PE统计
if [ -f "$RESULTS_DIR/single/ex2_pe_rust.sam" ]; then
    EX2_TOTAL=$(grep -v "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" | wc -l)
    EX2_UNIQUE=$(grep -v "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" | grep -c "XS:i:0")
fi
echo ""

# 7. 生成测试报告
echo "[7/7] 生成测试报告..."
REPORT_FILE="$RESULTS_DIR/P6_BENCHMARK_REPORT_$(date '+%Y%m%d_%H%M%S').md"

cat > "$REPORT_FILE" << EOF
# BSMAP-rs P6 基准测试报告

**测试日期**: $(date '+%Y-%m-%d %H:%M:%S')
**测试环境**: WSL2 Ubuntu
**测试版本**: P6 (完整优化链 - SIMD + 索引优化 + 并行优化)

---

## 一、性能测试结果

### 1.1 执行时间

| 测试用例 | 线程数 | 执行时间 |
|---------|--------|---------|
| Ex1 SE 75bp | 1 | ${EX1_TIME}s |
| Ex2 PE 150bp | 1 | ${EX2_TIME}s |
| Ex1 SE 75bp | 4 | ${EX1_4T_TIME}s |
| Ex2 PE 150bp | 4 | ${EX2_4T_TIME}s |

---

## 二、比对统计

### 2.1 Ex1 SE 75bp

| 指标 | 数值 |
|------|------|
| 总比对数 | ${EX1_TOTAL} |
| 唯一比对数 | ${EX1_UNIQUE} |

### 2.2 Ex2 PE 150bp

| 指标 | 数值 |
|------|------|
| 总比对数 | ${EX2_TOTAL} |
| 唯一比对数 | ${EX2_UNIQUE} |

---

## 三、结果文件位置

- 单线程结果: $RESULTS_DIR/single/
- 4线程结果: $RESULTS_DIR/4threads/

EOF

echo "✓ 测试报告已生成: $REPORT_FILE"
echo ""

echo "=========================================="
echo "基准测试完成！"
echo "=========================================="
echo ""
