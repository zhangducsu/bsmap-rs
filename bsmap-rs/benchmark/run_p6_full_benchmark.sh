#!/bin/bash
# BSMAP-rs P6 完整基准测试脚本
# 测试环境: WSL2 Ubuntu
# 测试内容: 单线程/多线程性能测试 + SAM对比

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
DATA_DIR="$PROJECT_ROOT/benchmark/data"
RESULTS_DIR="$PROJECT_ROOT/benchmark/results_p6_final"
BSMAP_RS_BIN="$PROJECT_ROOT/target/release/bsmap"
BSMAP_CPP_BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap"

# 测试参数
THREAD_COUNTS=(1 4)
TIMEOUT=600

# 创建结果目录
mkdir -p "$RESULTS_DIR"/{single,multi,sam_compare,profile}

# 日志函数
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# 记录时间
start_time=$(date +%s)

echo "=========================================="
echo "BSMAP-rs P6 完整基准测试"
echo "=========================================="
echo ""

# 1. 环境检查
log_info "1. 环境检查"
echo "-------------------------------------------"

# 检查编译
if [ -f "$BSMAP_RS_BIN" ]; then
    log_success "BSMAP-rs 二进制文件存在"
else
    log_error "BSMAP-rs 二进制文件不存在，请先编译"
    exit 1
fi

# 检查测试数据
if [ -f "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" ]; then
    log_success "Ex1 SE 测试数据存在"
else
    log_error "Ex1 SE 测试数据不存在"
    exit 1
fi

if [ -f "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" ]; then
    log_success "Ex2 PE 测试数据存在"
else
    log_error "Ex2 PE 测试数据不存在"
    exit 1
fi

if [ -f "$DATA_DIR/chr22_tail_1M.fa" ]; then
    log_success "参考基因组存在"
else
    log_error "参考基因组不存在"
    exit 1
fi

# 检查C++ BSMAP
if [ -f "$BSMAP_CPP_BIN" ]; then
    log_success "C++ BSMAP 存在"
    HAS_CPP=true
else
    log_warn "C++ BSMAP 不存在，跳过对比测试"
    HAS_CPP=false
fi

echo ""

# 2. 单线程性能测试
log_info "2. 单线程性能测试 (1 thread)"
echo "-------------------------------------------"

# Ex1 SE 单线程
log_info "测试 Ex1 SE 75bp..."
START=$(date +%s.%N)
{ timeout $TIMEOUT "$BSMAP_RS_BIN" \
    -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 1 \
    -o "$RESULTS_DIR/single/ex1_se_rust.sam" \
    2>&1 || true; } | tee "$RESULTS_DIR/single/ex1_se_rust.log"
END=$(date +%s.%N)
EX1_SE_TIME=$(echo "$END - $START" | bc)
EX1_SE_MEM=$(grep -oP 'Peak memory: \K\d+' "$RESULTS_DIR/single/ex1_se_rust.log" || echo "N/A")

log_success "Ex1 SE 单线程完成: ${EX1_SE_TIME}s"

# 统计比对结果
if [ -f "$RESULTS_DIR/single/ex1_se_rust.sam" ]; then
    EX1_TOTAL=$(grep -c "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" 2>/dev/null || echo 0)
    EX1_MAPPED=$(grep -v "^@" "$RESULTS_DIR/single/ex1_se_rust.sam" | grep -c "XS:i:" 2>/dev/null || echo 0)
    EX1_UNIQUE=$((EX1_TOTAL - EX1_MAPPED))
    echo "  总读段: $EX1_TOTAL, 唯一比对: $EX1_UNIQUE"
fi

echo ""

# Ex2 PE 单线程
log_info "测试 Ex2 PE 150bp..."
START=$(date +%s.%N)
{ timeout $TIMEOUT "$BSMAP_RS_BIN" \
    -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
    -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 1 \
    -o "$RESULTS_DIR/single/ex2_pe_rust.sam" \
    2>&1 || true; } | tee "$RESULTS_DIR/single/ex2_pe_rust.log"
END=$(date +%s.%N)
EX2_PE_TIME=$(echo "$END - $START" | bc)
EX2_PE_MEM=$(grep -oP 'Peak memory: \K\d+' "$RESULTS_DIR/single/ex2_pe_rust.log" || echo "N/A")

log_success "Ex2 PE 单线程完成: ${EX2_PE_TIME}s"

# 统计比对结果
if [ -f "$RESULTS_DIR/single/ex2_pe_rust.sam" ]; then
    EX2_TOTAL=$(grep -c "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" 2>/dev/null || echo 0)
    EX2_MAPPED=$(grep -v "^@" "$RESULTS_DIR/single/ex2_pe_rust.sam" | grep -c "XS:i:" 2>/dev/null || echo 0)
    EX2_UNIQUE=$((EX2_TOTAL - EX2_MAPPED))
    echo "  总读段: $EX2_TOTAL, 唯一比对: $EX2_UNIQUE"
fi

echo ""

# 3. 多线程性能测试
log_info "3. 多线程性能测试 (4 threads)"
echo "-------------------------------------------"

# Ex1 SE 4线程
log_info "测试 Ex1 SE 75bp (4线程)..."
START=$(date +%s.%N)
{ timeout $TIMEOUT "$BSMAP_RS_BIN" \
    -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 4 \
    -o "$RESULTS_DIR/multi/ex1_se_rust_4t.sam" \
    2>&1 || true; } | tee "$RESULTS_DIR/multi/ex1_se_rust_4t.log"
END=$(date +%s.%N)
EX1_SE_4T_TIME=$(echo "$END - $START" | bc)

log_success "Ex1 SE 4线程完成: ${EX1_SE_4T_TIME}s"

# Ex2 PE 4线程
log_info "测试 Ex2 PE 150bp (4线程)..."
START=$(date +%s.%N)
{ timeout $TIMEOUT "$BSMAP_RS_BIN" \
    -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
    -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
    -d "$DATA_DIR/chr22_tail_1M.fa" \
    -p 4 \
    -o "$RESULTS_DIR/multi/ex2_pe_rust_4t.sam" \
    2>&1 || true; } | tee "$RESULTS_DIR/multi/ex2_pe_4t_rust.log"
END=$(date +%s.%N)
EX2_PE_4T_TIME=$(echo "$END - $START" | bc)

log_success "Ex2 PE 4线程完成: ${EX2_PE_4T_TIME}s"

echo ""

# 4. C++ BSMAP 测试（如果存在）
if [ "$HAS_CPP" = true ]; then
    log_info "4. C++ BSMAP 性能测试"
    echo "-------------------------------------------"

    # Ex1 SE C++
    log_info "测试 Ex1 SE 75bp (C++ BSMAP)..."
    START=$(date +%s.%N)
    { timeout $TIMEOUT "$BSMAP_CPP_BIN" \
        -a "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" \
        -d "$DATA_DIR/chr22_tail_1M.fa" \
        -p 1 \
        -o "$RESULTS_DIR/single/ex1_se_cpp.sam" \
        2>&1 || true; } | tee "$RESULTS_DIR/single/ex1_se_cpp.log"
    END=$(date +%s.%N)
    EX1_SE_CPP_TIME=$(echo "$END - $START" | bc)

    log_success "Ex1 SE C++ 完成: ${EX1_SE_CPP_TIME}s"

    # Ex2 PE C++
    log_info "测试 Ex2 PE 150bp (C++ BSMAP)..."
    START=$(date +%s.%N)
    { timeout $TIMEOUT "$BSMAP_CPP_BIN" \
        -a "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" \
        -b "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" \
        -d "$DATA_DIR/chr22_tail_1M.fa" \
        -p 1 \
        -o "$RESULTS_DIR/single/ex2_pe_cpp.sam" \
        2>&1 || true; } | tee "$RESULTS_DIR/single/ex2_pe_cpp.log"
    END=$(date +%s.%N)
    EX2_PE_CPP_TIME=$(echo "$END - $START" | bc)

    log_success "Ex2 PE C++ 完成: ${EX2_PE_CPP_TIME}s"

    echo ""
fi

# 5. SAM对比分析
log_info "5. SAM比对结果对比"
echo "-------------------------------------------"

if [ "$HAS_CPP" = true ]; then
    # Ex1 SE SAM对比
    if [ -f "$RESULTS_DIR/single/ex1_se_rust.sam" ] && [ -f "$RESULTS_DIR/single/ex1_se_cpp.sam" ]; then
        log_info "Ex1 SE SAM对比..."
        "$SCRIPT_DIR/compare_sam.py" \
            "$RESULTS_DIR/single/ex1_se_cpp.sam" \
            "$RESULTS_DIR/single/ex1_se_rust.sam" \
            "$RESULTS_DIR/sam_compare/ex1_se_compare.txt"
        log_success "Ex1 SE SAM对比完成"
    fi

    # Ex2 PE SAM对比
    if [ -f "$RESULTS_DIR/single/ex2_pe_rust.sam" ] && [ -f "$RESULTS_DIR/single/ex2_pe_cpp.sam" ]; then
        log_info "Ex2 PE SAM对比..."
        "$SCRIPT_DIR/compare_sam.py" \
            "$RESULTS_DIR/single/ex2_pe_cpp.sam" \
            "$RESULTS_DIR/single/ex2_pe_rust.sam" \
            "$RESULTS_DIR/sam_compare/ex2_pe_compare.txt"
        log_success "Ex2 PE SAM对比完成"
    fi
fi

echo ""

# 6. 生成测试总结
log_info "6. 测试总结"
echo "-------------------------------------------"

total_time=$(($(date +%s) - start_time))
echo "总测试时间: ${total_time}s"
echo ""
echo "测试结果目录: $RESULTS_DIR"
echo ""

log_success "基准测试完成！"
