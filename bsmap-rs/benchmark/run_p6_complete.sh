#!/bin/bash
# P6 优化完整基准测试脚本 (WSL2环境)
set -e

# 配置路径
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCHMARK_DIR="${PROJECT_ROOT}/benchmark"
RESULTS_DIR="${BENCHMARK_DIR}/results_p6_final"
DATA_DIR="${BENCHMARK_DIR}/data"
BSMAP_RS="${PROJECT_ROOT}/target/release/bsmap"

# 参考基因组和测试数据
REF_FA="${DATA_DIR}/chr22_tail_1M.fa"
EX1_SE="${BENCHMARK_DIR}/tmp/ex1_se75_10x.fastq"
EX2_PE1="${BENCHMARK_DIR}/tmp/ex2_pe150_10x_1.fastq"
EX2_PE2="${BENCHMARK_DIR}/tmp/ex2_pe150_10x_2.fastq"

# 创建结果目录
mkdir -p "${RESULTS_DIR}/single"
mkdir -p "${RESULTS_DIR}/4threads"

echo "=========================================="
echo "BSMAP-rs P6 优化基准测试 (WSL2环境)"
echo "=========================================="

# 检查二进制文件
if [ ! -x "${BSMAP_RS}" ]; then
    echo "错误: 找不到可执行文件 ${BSMAP_RS}"
    echo "请先编译: cd ${PROJECT_ROOT} && cargo build --release"
    exit 1
fi

# 检查数据文件
for file in "${REF_FA}" "${EX1_SE}" "${EX2_PE1}" "${EX2_PE2}"; do
    if [ ! -f "${file}" ]; then
        echo "错误: 找不到数据文件 ${file}"
        exit 1
    fi
done

echo "✓ 环境检查通过"
echo ""

# 记录开始时间
TEST_START=$(date +%s)

# ======================================
# 1. 单线程测试
# ======================================
echo "===== [1/4] 单线程性能测试 ====="

# Ex1 SE (单端75bp)
echo "  测试 Ex1 SE (单线程)..."
START_TIME=$(date +%s.%N)
${BSMAP_RS} align \
        -a "${EX1_SE}" \
        -d "${REF_FA}" \
        -p 1 \
        -s 16 \
        -v 0.08 \
        -I 4 \
        -o "${RESULTS_DIR}/single/ex1_se_rust.sam" \
        --verbose 2 2>&1 | tee "${RESULTS_DIR}/single/ex1_se.log"
END_TIME=$(date +%s.%N)
ELAPSED=$(echo "$END_TIME - $START_TIME" | bc)
EX1_SINGLE_TIME="${ELAPSED}"
echo "  ✓ Ex1 SE 单线程: ${ELAPSED}s"
echo "  ✓ SAM文件行数: $(wc -l < "${RESULTS_DIR}/single/ex1_se_rust.sam")"

# Ex2 PE (双端150bp)
echo "  测试 Ex2 PE (单线程)..."
START_TIME=$(date +%s.%N)
${BSMAP_RS} align \
        -a "${EX2_PE1}" \
        -b "${EX2_PE2}" \
        -d "${REF_FA}" \
        -p 1 \
        -s 16 \
        -v 0.08 \
        -I 4 \
        -o "${RESULTS_DIR}/single/ex2_pe_rust.sam" \
        --verbose 2 2>&1 | tee "${RESULTS_DIR}/single/ex2_pe.log"
END_TIME=$(date +%s.%N)
ELAPSED=$(echo "$END_TIME - $START_TIME" | bc)
EX2_SINGLE_TIME="${ELAPSED}"
echo "  ✓ Ex2 PE 单线程: ${ELAPSED}s"
echo "  ✓ SAM文件行数: $(wc -l < "${RESULTS_DIR}/single/ex2_pe_rust.sam")"

# ======================================
# 2. 4线程测试
# ======================================
echo ""
echo "===== [2/4] 4线程性能测试 ====="

# Ex1 SE (4线程)
echo "  测试 Ex1 SE (4线程)..."
START_TIME=$(date +%s.%N)
${BSMAP_RS} align \
        -a "${EX1_SE}" \
        -d "${REF_FA}" \
        -p 4 \
        -s 16 \
        -v 0.08 \
        -I 4 \
        -o "${RESULTS_DIR}/4threads/ex1_se_rust.sam" \
        --verbose 2 2>&1 | tee "${RESULTS_DIR}/4threads/ex1_se.log"
END_TIME=$(date +%s.%N)
ELAPSED=$(echo "$END_TIME - $START_TIME" | bc)
EX1_4THREAD_TIME="${ELAPSED}"
echo "  ✓ Ex1 SE 4线程: ${ELAPSED}s"
echo "  ✓ SAM文件行数: $(wc -l < "${RESULTS_DIR}/4threads/ex1_se_rust.sam")"

# Ex2 PE (4线程)
echo "  测试 Ex2 PE (4线程)..."
START_TIME=$(date +%s.%N)
${BSMAP_RS} align \
        -a "${EX2_PE1}" \
        -b "${EX2_PE2}" \
        -d "${REF_FA}" \
        -p 4 \
        -s 16 \
        -v 0.08 \
        -I 4 \
        -o "${RESULTS_DIR}/4threads/ex2_pe_rust.sam" \
        --verbose 2 2>&1 | tee "${RESULTS_DIR}/4threads/ex2_pe.log"
END_TIME=$(date +%s.%N)
ELAPSED=$(echo "$END_TIME - $START_TIME" | bc)
EX2_4THREAD_TIME="${ELAPSED}"
echo "  ✓ Ex2 PE 4线程: ${ELAPSED}s"
echo "  ✓ SAM文件行数: $(wc -l < "${RESULTS_DIR}/4threads/ex2_pe_rust.sam")"

# ======================================
# 3. 计算加速比
# ======================================
echo ""
echo "===== [3/4] 性能分析 ====="

EX1_SPEEDUP=$(echo "scale=2; ${EX1_SINGLE_TIME} / ${EX1_4THREAD_TIME}" | bc)
EX2_SPEEDUP=$(echo "scale=2; ${EX2_SINGLE_TIME} / ${EX2_4THREAD_TIME}" | bc)

echo "Ex1 SE (单端75bp):"
echo "  - 单线程: ${EX1_SINGLE_TIME}s"
echo "  - 4线程:  ${EX1_4THREAD_TIME}s"
echo "  - 加速比:  ${EX1_SPEEDUP}x"

echo ""
echo "Ex2 PE (双端150bp):"
echo "  - 单线程: ${EX2_SINGLE_TIME}s"
echo "  - 4线程:  ${EX2_4THREAD_TIME}s"
echo "  - 加速比:  ${EX2_SPEEDUP}x"

# ======================================
# 4. 生成报告
# ======================================
echo ""
echo "===== [4/4] 生成测试报告 ====="

REPORT_FILE="${RESULTS_DIR}/p6_benchmark_report_$(date +%Y%m%d_%H%M%S).md"

cat > "${REPORT_FILE}" << EOF
# BSMAP-rs P6 优化基准测试报告

**测试时间**: $(date)
**测试环境**: WSL2 (Ubuntu)
**测试版本**: P6 最终优化版本

## 测试数据集

| 数据集 | 类型 | 读长 | 覆盖度 |
|--------|------|------|--------|
| Ex1 SE | 单端 | 75bp | 10x |
| Ex2 PE | 双端 | 150bp | 10x |

## 性能测试结果

### Ex1 SE (单端75bp)
- 单线程: ${EX1_SINGLE_TIME}s
- 4线程:  ${EX1_4THREAD_TIME}s
- 加速比:  ${EX1_SPEEDUP}x

### Ex2 PE (双端150bp)
- 单线程: ${EX2_SINGLE_TIME}s
- 4线程:  ${EX2_4THREAD_TIME}s
- 加速比:  ${EX2_SPEEDUP}x

## 比对结果统计

### Ex1 SE
- SAM行数: $(wc -l < "${RESULTS_DIR}/single/ex1_se_rust.sam")

### Ex2 PE
- SAM行数: $(wc -l < "${RESULTS_DIR}/single/ex2_pe_rust.sam")

## 测试文件位置

- 单线程结果: ${RESULTS_DIR}/single/
- 4线程结果: ${RESULTS_DIR}/4threads/
- 详细日志: *.log 文件

EOF

echo "✓ 报告已生成: ${REPORT_FILE}"
echo ""

# 统计总耗时
TEST_END=$(date +%s)
TOTAL_ELAPSED=$((TEST_END - TEST_START))
echo "=========================================="
echo "测试完成! 总耗时: ${TOTAL_ELAPSED}s"
echo "=========================================="
echo ""
echo "查看报告: cat ${REPORT_FILE}"
