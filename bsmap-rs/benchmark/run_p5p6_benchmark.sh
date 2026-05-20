#!/bin/bash
# BSMAP-rs P5/P6 优化基准测试脚本
# 用于验证所有P5和P6优化是否正常工作

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DATA_DIR="$PROJECT_ROOT/../data"
RESULTS_DIR="$PROJECT_ROOT/results_p5p6"

echo "======================================"
echo "BSMAP-rs P5/P6 优化基准测试"
echo "======================================"
echo ""

# 创建结果目录
mkdir -p "$RESULTS_DIR"

# 设置超时（秒）
TIMEOUT=300

# 测试函数
run_test() {
    local name="$1"
    local cmd="$2"
    local output="$3"

    echo "[$name] 开始测试..."
    echo "命令: $cmd"

    # 检查命令是否存在
    if ! command -v timeout &> /dev/null; then
        # Windows环境下使用替代方法
        start_time=$(date +%s)
        eval "$cmd" > "$output" 2>&1 || true
        end_time=$(date +%s)
        elapsed=$((end_time - start_time))
    else
        # Unix环境
        timeout $TIMEOUT bash -c "$cmd" > "$output" 2>&1 || {
            echo "[$name] 超时或失败"
            return 1
        }
        elapsed=$(grep "Elapsed:" "$output" 2>/dev/null | awk '{print $2}' || echo "N/A")
    fi

    echo "[$name] 完成 (耗时: ${elapsed}s)"
    echo ""
}

# 检查编译
echo "======================================"
echo "1. 检查编译状态"
echo "======================================"
if [ -f "$PROJECT_ROOT/target/release/bsmap" ]; then
    echo "✓ bsmap 二进制已编译"
    BSMAP_RS="$PROJECT_ROOT/target/release/bsmap"
elif [ -f "$PROJECT_ROOT/target/debug/bsmap" ]; then
    echo "✓ bsmap 二进制已编译 (debug)"
    BSMAP_RS="$PROJECT_ROOT/target/debug/bsmap"
else
    echo "✗ bsmap 二进制未找到，尝试编译..."
    cd "$PROJECT_ROOT"
    cargo build --release 2>&1 | tail -5
    BSMAP_RS="$PROJECT_ROOT/target/release/bsmap"
fi

# 显示版本信息
echo ""
echo "======================================"
echo "2. 版本信息"
echo "======================================"
"$BSMAP_RS" --version 2>/dev/null || "$BSMAP_RS" --help 2>&1 | head -10

# 检查测试数据
echo ""
echo "======================================"
echo "3. 检查测试数据"
echo "======================================"
if [ -d "$DATA_DIR/wgbs/ex1_se75_10x" ]; then
    echo "✓ Ex1 SE 测试数据存在"
else
    echo "✗ Ex1 SE 测试数据不存在"
fi

if [ -d "$DATA_DIR/wgbs/ex2_pe150_10x" ]; then
    echo "✓ Ex2 PE 测试数据存在"
else
    echo "✗ Ex2 PE 测试数据不存在"
fi

if [ -f "$DATA_DIR/chr22_tail_1M.fa" ]; then
    echo "✓ 参考序列存在"
else
    echo "✗ 参考序列不存在"
fi

# 运行Ex1测试
echo ""
echo "======================================"
echo "4. Ex1 SE 75bp 单线程测试"
echo "======================================"
if [ -f "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" ]; then
    EX1_CMD="$BSMAP_RS -a $DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz \
        -d $DATA_DIR/chr22_tail_1M.fa \
        -p 1 \
        -o $RESULTS_DIR/ex1_se.sam 2>&1"

    if [ -f "$RESULTS_DIR/ex1_se.sam" ]; then
        echo "Ex1 SE 结果已存在，跳过"
    else
        run_test "Ex1 SE" "$EX1_CMD" "$RESULTS_DIR/ex1_se.log"
    fi

    # 统计结果
    if [ -f "$RESULTS_DIR/ex1_se.sam" ]; then
        TOTAL=$(grep -c "^@" "$RESULTS_DIR/ex1_se.sam" 2>/dev/null || echo 0)
        MAPPED=$(grep -v "^@" "$RESULTS_DIR/ex1_se.sam" | grep -c "XS:i:" 2>/dev/null || echo 0)
        UNIQUE=$(grep -v "^@" "$RESULTS_DIR/ex1_se.sam" | grep -vc "XS:i:" 2>/dev/null || echo 0)
        echo "  总读段数: $TOTAL"
        echo "  唯一比对: $UNIQUE"
    fi
fi

# 运行Ex2测试
echo ""
echo "======================================"
echo "5. Ex2 PE 150bp 单线程测试"
echo "======================================"
if [ -f "$DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" ]; then
    EX2_CMD="$BSMAP_RS -a $DATA_DIR/wgbs/ex2_pe150_10x/simulated_1.fastq.gz \
        -b $DATA_DIR/wgbs/ex2_pe150_10x/simulated_2.fastq.gz \
        -d $DATA_DIR/chr22_tail_1M.fa \
        -p 1 \
        -o $RESULTS_DIR/ex2_pe.sam 2>&1"

    if [ -f "$RESULTS_DIR/ex2_pe.sam" ]; then
        echo "Ex2 PE 结果已存在，跳过"
    else
        run_test "Ex2 PE" "$EX2_CMD" "$RESULTS_DIR/ex2_pe.log"
    fi

    # 统计结果
    if [ -f "$RESULTS_DIR/ex2_pe.sam" ]; then
        TOTAL=$(grep -c "^@" "$RESULTS_DIR/ex2_pe.sam" 2>/dev/null || echo 0)
        MAPPED=$(grep -v "^@" "$RESULTS_DIR/ex2_pe.sam" | grep -c "XS:i:" 2>/dev/null || echo 0)
        UNIQUE=$(grep -v "^@" "$RESULTS_DIR/ex2_pe.sam" | grep -vc "XS:i:" 2>/dev/null || echo 0)
        echo "  总读段对数: $TOTAL"
        echo "  唯一比对: $UNIQUE"
    fi
fi

# 运行4线程测试
echo ""
echo "======================================"
echo "6. Ex1 SE 75bp 4线程测试"
echo "======================================"
if [ -f "$DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz" ]; then
    EX1_4T_CMD="$BSMAP_RS -a $DATA_DIR/wgbs/ex1_se75_10x/simulated.fastq.gz \
        -d $DATA_DIR/chr22_tail_1M.fa \
        -p 4 \
        -o $RESULTS_DIR/ex1_se_4t.sam 2>&1"

    if [ -f "$RESULTS_DIR/ex1_se_4t.sam" ]; then
        echo "Ex1 SE 4线程结果已存在，跳过"
    else
        run_test "Ex1 SE 4线程" "$EX1_4T_CMD" "$RESULTS_DIR/ex1_se_4t.log"
    fi
fi

# 生成测试总结
echo ""
echo "======================================"
echo "测试总结"
echo "======================================"
echo "结果目录: $RESULTS_DIR"
echo ""

if [ -f "$RESULTS_DIR/ex1_se.log" ]; then
    echo "Ex1 SE 单线程:"
    grep -E "^(总读段|唯一比对|Elapsed)" "$RESULTS_DIR/ex1_se.log" 2>/dev/null || echo "  完成"
fi

if [ -f "$RESULTS_DIR/ex2_pe.log" ]; then
    echo "Ex2 PE 单线程:"
    grep -E "^(总读段|唯一比对|Elapsed)" "$RESULTS_DIR/ex2_pe.log" 2>/dev/null || echo "  完成"
fi

if [ -f "$RESULTS_DIR/ex1_se_4t.log" ]; then
    echo "Ex1 SE 4线程:"
    grep -E "^(总读段|唯一比对|Elapsed)" "$RESULTS_DIR/ex1_se_4t.log" 2>/dev/null || echo "  完成"
fi

echo ""
echo "测试完成！"
