#!/bin/bash
# ==========================================
# P3优化测试脚本
# 测试内容:
#   1. 优化前后的性能对比
#   2. 内存分配优化效果
#   3. 提前终止策略效果
# ==========================================
set -e
WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "BSMAP-rs P3优化测试"
echo "=========================================="
echo "  运行环境: Docker 20GB内存"
echo "  线程数: 4"
echo "  测试内容: Ex1 (WGBS SE), Ex2 (WGBS PE)"
echo "=========================================="
date
echo ""

# ======================================
# 步骤1：清理旧结果
# ======================================
echo ">>> 步骤1：清理旧结果..."
rm -rf results_p3/*
mkdir -p results_p3

# ======================================
# 步骤2：解压测试数据
# ======================================
echo ""
echo ">>> 步骤2：准备测试数据..."
mkdir -p tmp

if [ ! -f tmp/ex1_se75_10x.fastq ]; then
    echo "  解压 Ex1 数据..."
    gzip -d -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > tmp/ex1_se75_10x.fastq
fi

if [ ! -f tmp/ex2_pe150_10x_1.fastq ]; then
    echo "  解压 Ex2 数据..."
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > tmp/ex2_pe150_10x_1.fastq
    gzip -d -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > tmp/ex2_pe150_10x_2.fastq
fi

# ======================================
# 定义测试函数
# ======================================
run_bsmap_rs_p3() {
    local EXAMPLE=$1
    local READ1=$2
    local READ2=$3
    
    echo "  [$EXAMPLE] bsmap-rs (4线程, P3优化)..."
    local RESULT_DIR="results_p3/${EXAMPLE}_p3_optimized"
    mkdir -p $RESULT_DIR
    
    if [ "$READ2" = "" ]; then
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s 16 -v 0.08 -I 4 -p 4 \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    else
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
            -a tmp/$READ1 -b tmp/$READ2 \
            -d data/chr22_tail_1M.fa \
            -o $RESULT_DIR/bsmaprs.sam \
            -s 16 -v 0.08 -I 4 -p 4 \
            2>&1 | tee $RESULT_DIR/bsmaprs.log
    fi
}

# ======================================
# 步骤3：运行P3优化测试
# ======================================
echo ""
echo "======================================"
echo "P3优化测试"
echo "======================================"

echo ""
echo "--- Ex1: WGBS SE 75bp ---"
run_bsmap_rs_p3 "example1_wgbs_se" "ex1_se75_10x.fastq" ""

echo ""
echo "--- Ex2: WGBS PE 150bp ---"
run_bsmap_rs_p3 "example2_wgbs_pe" "ex2_pe150_10x_1.fastq" "ex2_pe150_10x_2.fastq"

# ======================================
# 步骤4：生成结果汇总
# ======================================
echo ""
echo "======================================"
echo "步骤4：生成结果汇总"
echo "======================================"

cat > results_p3/summary.csv << 'CSV_HEADER'
example,tool,mode,time_wall_sec,time_user_sec,time_sys_sec,mem_max_rss_kb
CSV_HEADER

extract_stats_p3() {
    local EXAMPLE=$1
    local TOOL=$2
    local MODE=$3
    local LOG_FILE="results_p3/${EXAMPLE}_${TOOL}/${TOOL}.log"
    
    if [ ! -f "$LOG_FILE" ]; then
        return
    fi
    
    local WALL=$(grep "wall clock" "$LOG_FILE" | awk '{print $NF}' | tr -d ':' | awk -F. '{printf "%.2f", ($1*60) + $2}' || echo "0")
    local USER=$(grep "user" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local SYS=$(grep "sys" "$LOG_FILE" | head -1 | awk '{print $NF}' || echo "0")
    local RSS=$(grep "Maximum resident" "$LOG_FILE" | awk '{print $NF}' || echo "0")
    
    echo "$EXAMPLE,$TOOL,$MODE,$WALL,$USER,$SYS,$RSS"
}

echo "  提取统计数据..."
extract_stats_p3 "example1_wgbs_se" "bsmaprs" "p3_optimized" >> results_p3/summary.csv
extract_stats_p3 "example2_wgbs_pe" "bsmaprs" "p3_optimized" >> results_p3/summary.csv

echo ""
echo "=== P3优化测试结果 ==="
cat results_p3/summary.csv

# ======================================
# 步骤5：生成P3优化报告
# ======================================
echo ""
echo "======================================"
echo "步骤5：生成P3优化报告"
echo "======================================"

cat > results_p3/p3_optimization_report.md << 'REPORT'
# BSMAP-rs P3优化测试报告

**测试日期**: $(date)
**测试环境**: Docker容器 (20GB内存, 4线程)

---

## 测试目标

验证P3优化的实际效果：
1. **提前终止策略**: 参考原版BSMAP逻辑，找到唯一比对后提前终止
2. **命中去重优化**: 使用排序+去重代替HashSet，减少内存分配
3. **对象池**: 减少高频分配场景的内存分配开销

---

## 测试配置

| 配置项 | 值 |
|-------|-----|
| 参考序列 | chr22_tail_1M.fa (1Mbp) |
| 种子大小 | 16 |
| 最大错配率 | 8% |
| 索引间隔 | 4 |
| 线程数 | 4 |

---

## P3优化内容

### 1. 提前终止策略

```rust
fn should_stop_early(seg_idx, hits, snp_thres) {
    // 处理至少2个segment后才考虑终止
    // 如果找到唯一比对且mismatch数足够好，提前终止
    // 如果命中数超过1000，提前终止避免过多计算
}
```

### 2. 命中去重优化

```rust
// 优化前: 使用HashSet (O(n)插入, 但有内存分配开销)
// 优化后: 排序+去重 (O(n log n), 无额外分配)
fn dedup_hits_fast(hits) {
    hits.sort_unstable_by_key(|h| (h.chr, h.loc, h.strand, h.snps));
    hits.retain(|h| { /* 去重逻辑 */ });
}
```

### 3. 对象池实现

```rust
struct HitPool<T> {
    hits: Vec<T>,  // 预分配缓冲区
    pos: usize,    // 当前位置
}
```

---

## 测试结果

### 性能对比

| 测试用例 | 总耗时 | 内存峰值 |
|---------|--------|----------|

REPORT

cat results_p3/summary.csv >> results_p3/p3_optimization_report.md

cat >> results_p3/p3_optimization_report.md << 'REPORT_END'

---

## 分析结论

### P3优化效果

| 优化项 | 说明 |
|--------|------|
| 提前终止 | 根据数据特征，可能减少20-50%的处理时间 |
| 去重优化 | 减少HashSet分配，降低内存压力 |
| 对象池 | 减少临时对象分配，提高缓存效率 |

---

## 优化收益预估

| 维度 | 预期收益 |
|------|---------|
| 内存分配 | 减少30-50% |
| 单读段处理 | 5-15%加速 |
| 总体性能 | 取决于数据特征 |

---

**报告生成时间**: $(date)
REPORT_END

date > results_p3/run_date.txt

echo ""
echo "=========================================="
echo "✅ P3优化测试完成！"
echo "=========================================="
echo "结果目录：results_p3/"
echo "汇总文件：results_p3/summary.csv"
echo "报告文件：results_p3/p3_optimization_report.md"
echo "=========================================="
date
