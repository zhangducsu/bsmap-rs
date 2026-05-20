#!/bin/bash
# ========================================
# 完整对比测试：原版BSMAP vs bsmap-rs (Mmap模式)
# ========================================

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "============================================="
echo "BSMAP C++ vs bsmap-rs (Mmap模式) 完整对比测试"
echo "============================================="
date
echo ""

# 准备
mkdir -p tmp results_$(date +%Y%m%d_%H%M%S report
export LC_ALL=C
export RUST_BACKTRACE=1

RESULTS_DIR="results_$(date +%Y%m%d_%H%M%S)"
mkdir -p $RESULTS_DIR

echo "结果将保存到: $RESULTS_DIR"

# ========================================
# Step 1: 准备测试数据
# ========================================
echo "[步骤 1] 准备测试数据..."

# 解压所需的数据准备
for fq in tmp/ex1_se75_10x.fastq tmp/ex2_pe150_10x_1.fastq tmp/ex2_pe150_10x_2.fastq; do
  if [ ! -f "$fq" ]; then
    case "$fq" in
      tmp/ex1_se75_10x.fastq)
        gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
        echo "  解压: $fq"
        ;;
      tmp/ex2_pe150_10x_1.fastq)
        gunzip -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > tmp/ex2_pe150_10x_1.fastq
        echo "  解压: $fq"
        ;;
      tmp/ex2_pe150_10x_2.fastq)
        gunzip -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > tmp/ex2_pe150_10x_2.fastq
        echo "  解压: $fq"
        ;;
    esac
  fi
done

echo "✓ 测试数据准备完成"

# ========================================
# Step 2: 构建 bsmap-rs V3索引
# ========================================
echo ""
echo "[步骤 2] 构建 bsmap-rs V3索引 (Mmap模式)"
cd /workspace/bsmap-rs
rm -f benchmark/data/chr22_tail_1M.fa.bsi 2>/dev/null || true
echo "  正在构建索引..."
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap index \
  -d benchmark/data/chr22_tail_1M.fa \
  -s 16 2>&1 | tee $RESULTS_DIR/index_build_rs.log
cd "$WORK_DIR"
echo "✓ 索引构建完成"

# ========================================
# Step 3: 运行原版C++ BSMAP - Example 1
# ========================================
echo ""
echo "[步骤 3] Example 1 - 运行原版C++ BSMAP (WGBS SE)"
mkdir -p $RESULTS_DIR/example1_bsmap
BSMAP_EXE="/workspace/bsmap-original/bsmap-2.90/bsmap"
if [ -f "$BSMAP_EXE" ]; then
    echo "  BSMAP执行文件存在: $BSMAP_EXE"
else
    echo "  警告: BSMAP执行文件不存在！"
fi

echo "  运行中..."
/usr/bin/time -v $BSMAP_EXE \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o $RESULTS_DIR/example1_bsmap/bsmap.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee $RESULTS_DIR/example1_bsmap/bsmap.log
echo "✓ 原版BSMAP Example 运行完成"

# ========================================
# Step 4: 运行bsmap-rs - Example 1
# ========================================
echo ""
echo "[步骤 4] Example 1 - 运行bsmap-rs (WGBS SE, Mmap模式)"
mkdir -p $RESULTS_DIR/example1_bsmaprs
echo "  运行中..."
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
  -a tmp/ex1_se75_10x.fastq \
  -d data/chr22_tail_1M.fa \
  -o $RESULTS_DIR/example1_bsmaprs/bsmaprs.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee $RESULTS_DIR/example1_bsmaprs/bsmaprs.log
echo "✓ bsmap-rs Example 1运行完成"

# ========================================
# Step 5: Example 1 SAM对比
# ========================================
echo ""
echo "[步骤 5] Example 1 SAM对比"
mkdir -p $RESULTS_DIR/comparison_example1
./compare_sam_detailed.sh \
  $RESULTS_DIR/example1_bsmap/bsmap.sam \
  $RESULTS_DIR/example1_bsmaprs/bsmaprs.sam \
  $RESULTS_DIR/comparison_example1 \
  "example1_wgbs_se"

# ========================================
# Step 6: 运行原版C++ BSMAP - Example 2
# ========================================
echo ""
echo "[步骤 6] Example 2 - 运行原版C++ BSMAP (WGBS PE)"
mkdir -p $RESULTS_DIR/example2_bsmap
/usr/bin/time -v $BSMAP_EXE \
  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
  -d data/chr22_tail_1M.fa \
  -o $RESULTS_DIR/example2_bsmap/bsmap.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee $RESULTS_DIR/example2_bsmap/bsmap.log
echo "✓ 原版BSMAP Example 2运行完成"

# ========================================
# Step 7: 运行bsmap-rs - Example 2
# ========================================
echo ""
echo "[步骤 7] Example 2 - 运行bsmap-rs (WGBS PE, Mmap模式)"
mkdir -p $RESULTS_DIR/example2_bsmaprs
/usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
  -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
  -d data/chr22_tail_1M.fa \
  -o $RESULTS_DIR/example2_bsmaprs/bsmaprs.sam \
  -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee $RESULTS_DIR/example2_bsmaprs/bsmaprs.log
echo "✓ bsmap-rs Example 2运行完成"

# ========================================
# Step 8: Example 2 SAM对比
# ========================================
echo ""
echo "[步骤 8] Example 2 SAM对比"
mkdir -p $RESULTS_DIR/comparison_example2
./compare_sam_detailed.sh \
  $RESULTS_DIR/example2_bsmap/bsmap.sam \
  $RESULTS_DIR/example2_bsmaprs/bsmaprs.sam \
  $RESULTS_DIR/comparison_example2 \
  "example2_wgbs_pe"

# ========================================
# Step 9: 生成最终报告
# ========================================
echo ""
echo "[步骤 9] 生成最终报告"

# 提取性能数据
cat > $RESULTS_DIR/summary.csv << CSV_HEADER
example,tool,mode,time_wall,time_user,time_sys,mem_max_rss_kb
CSV_HEADER

for ex in 1 2; do
    for tool in bsmap bsmaprs; do
        LOG_FILE="$RESULTS_DIR/example${ex}_${tool}/${tool}.log"
        if [ -f "$LOG_FILE" ]; then
            WALL=$(grep "wall clock" $LOG_FILE | awk '{print $NF}' | tr -d '"'
            USER=$(grep "user" $LOG_FILE | head -1 | awk '{print $NF}')
            SYS=$(grep "sys" $LOG_FILE | head -1 | awk '{print $NF}')
            RSS=$(grep "Maximum resident" $LOG_FILE | awk '{print $NF}')
            if [ "$ex" = "1" ]; then
                MODE="wgbs_se"
            else
                MODE="wgbs_pe"
            fi
            echo "example${ex},${tool},${MODE},${WALL},${USER},${SYS},${RSS}" >> $RESULTS_DIR/summary.csv
        fi
    done
done

# 生成最终报告
cat > $RESULTS_DIR/final_comparison_report.md << REPORT
# BSMAP vs bsmap-rs 完整对比测试报告

**测试日期**: $(date)

## 测试环境
- Docker容器：bsmap-rs-test
- 内存限制：20G
- 参考序列：data/chr22_tail_1M.fa

## Example 1: WGBS Single-End (SE)
### 测试参数
- 种子长度：16
- 错配率：8%
- 插入缺失：4
- 线程数：1

### 性能对比
| 指标 | BSMAP C++ | bsmap-rs (Mmap) |
|------|-----------|------------------|
| 运行时间 | $(grep "wall clock" $RESULTS_DIR/example1_bsmap/bsmap.log | awk '{print $NF}' | $(grep "wall clock" $RESULTS_DIR/example1_bsmaprs/bsmaprs.log | awk '{print $NF}'
| 用户CPU | $(grep "user" $RESULTS_DIR/example1_bsmap/bsmap.log | head -1 | awk '{print $NF}') | $(grep "user" $RESULTS_DIR/example1_bsmaprs/bsmaprs.log | head -1 | awk '{print $NF}') |
| 系统CPU | $(grep "sys" $RESULTS_DIR/example1_bsmap/bsmap.log | head -1 | awk '{print $NF}') | $(grep "sys" $RESULTS_DIR/example1_bsmaprs/bsmaprs.log | head -1 | awk '{print $NF}') |
| 最大内存 | $(grep "Maximum resident" $RESULTS_DIR/example1_bsmap/bsmap.log | awk '{print $NF}') KB | $(grep "Maximum resident" $RESULTS_DIR/example1_bsmaprs/bsmaprs.log | awk '{print $NF}') KB |

### SAM一致性
详细报告: comparison_example1/detailed_report.txt

## Example 2: WGBS Paired-End (PE)
### 测试参数
- 种子长度：16
- 错配率：8%
- 插入缺失：4
- 线程数：1

### 性能对比
| 指标 | BSMAP C++ | bsmap-rs (Mmap) |
|------|-----------|------------------|
| 运行时间 | $(grep "wall clock" $RESULTS_DIR/example2_bsmap/bsmap.log | awk '{print $NF}'
| 用户CPU | $(grep "user" $RESULTS_DIR/example2_bsmap/bsmap.log | head -1 | awk '{print $NF}') | $(grep "user" $RESULTS_DIR/example2_bsmaprs/bsmaprs.log | head -1 | awk '{print $NF}') |
| 系统CPU | $(grep "sys" $RESULTS_DIR/example2_bsmap/bsmap.log | head -1 | awk '{print $NF}') | $(grep "sys" $RESULTS_DIR/example2_bsmaprs/bsmaprs.log | head -1 | awk '{print $NF}') |
| 最大内存 | $(grep "Maximum resident" $RESULTS_DIR/example2_bsmap/bsmap.log | awk '{print $NF}') KB | $(grep "Maximum resident" $RESULTS_DIR/example2_bsmaprs/bsmaprs.log | awk '{print $NF}') KB |

### SAM一致性
详细报告: comparison_example2/detailed_report.txt

## 关键代码修改
### bsmap/src/reference/storage.rs
```rust
// 添加了 get_index2_entry() 方法
```
### bsmap/src/reference/index.rs
```rust
// 修改了 lookup_separated() 方法
```

## 结论
TODO

REPORT

echo ""
echo "✓ 完整报告生成完成: $RESULTS_DIR/final_comparison_report.md"
echo "✓ 汇总数据: $RESULTS_DIR/summary.csv"

echo ""
echo "============================================="
echo "完整对比测试完成！"
echo "============================================="
date
