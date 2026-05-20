#!/bin/bash
# bsmap-rs vs BSMAP C++ 对比测试 - 最终版
# 修复所有参数问题，使用正确的路径

WORK_DIR="/workspace/bsmap-rs/benchmark"
cd "$WORK_DIR"

echo "=========================================="
echo "开始执行 BSMAP vs BSMAP-rs 基准测试"
echo "=========================================="
date
echo ""

# 创建必要目录
mkdir -p index results report tmp

# ========================================
# Step 1: 解压测试数据
# ========================================
echo "[1] 解压测试数据到 tmp/ 目录..."

# WGBS 数据
for fq in tmp/ex1_se75_10x.fastq tmp/ex2_pe150_10x_1.fastq tmp/ex2_pe150_10x_2.fastq tmp/ex5_pe150_20x_1.fastq tmp/ex5_pe150_20x_2.fastq; do
  if [ ! -f "$fq" ]; then
    case "$fq" in
      tmp/ex1_se75_10x.fastq) gunzip -c data/wgbs/ex1_se75_10x/simulated.fastq.gz > "$fq" ;;
      tmp/ex2_pe150_10x_1.fastq) gunzip -c data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz > "$fq" ;;
      tmp/ex2_pe150_10x_2.fastq) gunzip -c data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz > "$fq" ;;
      tmp/ex5_pe150_20x_1.fastq) gunzip -c data/wgbs/ex5_pe150_20x/simulated_1.fastq.gz > "$fq" ;;
      tmp/ex5_pe150_20x_2.fastq) gunzip -c data/wgbs/ex5_pe150_20x/simulated_2.fastq.gz > "$fq" ;;
    esac
    echo "  解压: $fq"
  fi
done

# RRBS 数据
for fq in tmp/ex3_se75_10x.fastq tmp/ex4_pe150_10x_1.fastq tmp/ex4_pe150_10x_2.fastq tmp/ex6_pe150_20x_1.fastq tmp/ex6_pe150_20x_2.fastq; do
  if [ ! -f "$fq" ]; then
    case "$fq" in
      tmp/ex3_se75_10x.fastq) gunzip -c data/rrbs/rrbssim/ex3_se75_10x.1.fq.gz > "$fq" ;;
      tmp/ex4_pe150_10x_1.fastq) gunzip -c data/rrbs/rrbssim/ex4_pe150_10x.1.fq.gz > "$fq" ;;
      tmp/ex4_pe150_10x_2.fastq) gunzip -c data/rrbs/rrbssim/ex4_pe150_10x.2.fq.gz > "$fq" ;;
      tmp/ex6_pe150_20x_1.fastq) gunzip -c data/rrbs/rrbssim/ex6_pe150_20x.1.fq.gz > "$fq" ;;
      tmp/ex6_pe150_20x_2.fastq) gunzip -c data/rrbs/rrbssim/ex6_pe150_20x.2.fq.gz > "$fq" ;;
    esac
    echo "  解压: $fq"
  fi
done

echo "解压完成"
ls -lh tmp/

# ========================================
# Step 2: 创建 SAM 对比脚本
# ========================================
cat > compare_sam.sh << 'EOF'
#!/bin/bash
SAM1=$1
SAM2=$2
OUT=$3
mkdir -p $OUT
grep -v "^@" $SAM1 | sort > $OUT/sam1_sorted.sam
grep -v "^@" $SAM2 | sort > $OUT/sam2_sorted.sam
echo "=== SAM 记录数 ===" > $OUT/diff_report.txt
wc -l $OUT/sam1_sorted.sam >> $OUT/diff_report.txt
wc -l $OUT/sam2_sorted.sam >> $OUT/diff_report.txt
echo "" >> $OUT/diff_report.txt
echo "=== 行差异 ===" >> $OUT/diff_report.txt
diff $OUT/sam1_sorted.sam $OUT/sam2_sorted.sam | grep "^[<>]" | head -50 >> $OUT/diff_report.txt
echo "" >> $OUT/diff_report.txt
DIFF_COUNT=$(diff $OUT/sam1_sorted.sam $OUT/sam2_sorted.sam | wc -l)
echo "总差异行数: $DIFF_COUNT" >> $OUT/diff_report.txt
cat $OUT/diff_report.txt
EOF
chmod +x compare_sam.sh

# ========================================
# Step 3: 比对测试 - 6 Examples
# ========================================
echo ""
echo "=========================================="
echo "开始比对测试 (6 Examples)"
echo "=========================================="

run_alignments() {
  local ex=$1
  local mode=$2
  local reads=$3
  local extra=$4
  
  local dir=""
  case $ex in
    1) dir="example1_wgbs_se";;
    2) dir="example2_wgbs_pe";;
    3) dir="example3_rrbs_se";;
    4) dir="example4_rrbs_pe";;
    5) dir="example5_wgbs_pe_20x";;
    6) dir="example6_rrbs_pe_20x";;
  esac
  
  mkdir -p results/$dir
  
  # 原版 BSMAP
  echo ""
  echo "[$ex] $dir - 原版 BSMAP..."
  if [ ! -f results/$dir/bsmap.sam ]; then
    case $ex in
      1)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex1_se75_10x.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
      2)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
      3)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex3_se75_10x.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
      4)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex4_pe150_10x_1.fastq -b tmp/ex4_pe150_10x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
      5)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex5_pe150_20x_1.fastq -b tmp/ex5_pe150_20x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
      6)
        /usr/bin/time -v /workspace/bsmap-original/bsmap-2.90/bsmap \
          -a tmp/ex6_pe150_20x_1.fastq -b tmp/ex6_pe150_20x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmap.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmap.log
        ;;
    esac
  else
    echo "  已存在，跳过"
  fi
  
  # bsmap-rs
  echo ""
  echo "[$ex] $dir - bsmap-rs..."
  if [ ! -f results/$dir/bsmaprs.sam ]; then
    case $ex in
      1)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex1_se75_10x.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
      2)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex2_pe150_10x_1.fastq -b tmp/ex2_pe150_10x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
      3)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex3_se75_10x.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
      4)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex4_pe150_10x_1.fastq -b tmp/ex4_pe150_10x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
      5)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex5_pe150_20x_1.fastq -b tmp/ex5_pe150_20x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 16 -v 0.08 -I 4 -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
      6)
        /usr/bin/time -v /workspace/bsmap-rs/target/release/bsmap align \
          -a tmp/ex6_pe150_20x_1.fastq -b tmp/ex6_pe150_20x_2.fastq \
          -d data/chr22_tail_1M.fa \
          -o results/$dir/bsmaprs.sam \
          -s 12 -v 0.08 -I 4 -D C-CGG -p 1 2>&1 | tee results/$dir/bsmaprs.log
        ;;
    esac
  else
    echo "  已存在，跳过"
  fi
  
  # SAM 对比
  echo ""
  echo "[$ex] SAM 对比..."
  if [ -f results/$dir/bsmap.sam ] && [ -f results/$dir/bsmaprs.sam ]; then
    bash compare_sam.sh \
      results/$dir/bsmap.sam \
      results/$dir/bsmaprs.sam \
      results/${dir}_diff
  fi
}

# 运行所有 6 个 Examples
for ex in 1 2 3 4 5 6; do
  run_alignments $ex
done

# ========================================
# Step 4: 生成报告
# ========================================
echo ""
echo "=========================================="
echo "生成测试报告"
echo "=========================================="

# 生成 summary.csv
echo "example,tool,mode,time_wall,time_user,time_sys,mem_max_rss_kb" > results/summary.csv

for i in 1 2 3 4 5 6; do
  for tool in bsmap bsmaprs; do
    case $i in
      1) DIR="example1_wgbs_se"; MODE="wgbs";;
      2) DIR="example2_wgbs_pe"; MODE="wgbs";;
      3) DIR="example3_rrbs_se"; MODE="rrbs";;
      4) DIR="example4_rrbs_pe"; MODE="rrbs";;
      5) DIR="example5_wgbs_pe_20x"; MODE="wgbs";;
      6) DIR="example6_rrbs_pe_20x"; MODE="rrbs";;
    esac
    
    LOG="results/${DIR}/${tool}.log"
    if [ -f "$LOG" ]; then
      WALL=$(grep "wall clock" $LOG | awk '{print $NF}')
      USER=$(grep "user" $LOG | head -1 | awk '{print $NF}')
      SYS=$(grep "sys" $LOG | head -1 | awk '{print $NF}')
      RSS=$(grep "Maximum resident" $LOG | awk '{print $NF}')
      echo "example${i},${tool},${MODE},${WALL},${USER},${SYS},${RSS}" >> results/summary.csv
    fi
  done
done

echo ""
echo "=== 测试结果汇总 (summary.csv) ==="
cat results/summary.csv

echo ""
echo "=========================================="
echo "测试完成！"
echo "=========================================="
date
