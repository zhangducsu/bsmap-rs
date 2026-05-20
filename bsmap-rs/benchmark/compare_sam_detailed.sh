#!/bin/bash
# ===============================
# 详细SAM比对分析脚本 (纯Shell实现，避免依赖R)
# ===============================

SAM1=$1
SAM2=$2
OUT_DIR=$3
EXAMPLE_NAME=$4

mkdir -p $OUT_DIR

echo "============================================="
echo "分析 $EXAMPLE_NAME"
echo "============================================="
echo "SAM1: $SAM1"
echo "SAM2: $SAM2"
echo "输出目录: $OUT_DIR"
echo ""

# 输出报告文件
REPORT="$OUT_DIR/detailed_report.txt"

echo "=============================================" > $REPORT
echo "详细SAM比对报告 - $EXAMPLE_NAME" >> $REPORT
echo "=============================================" >> $REPORT
echo "" >> $REPORT
echo "生成时间: $(date)" >> $REPORT
echo "" >> $REPORT

# ===============================
# 1. 统计读段总数
# ===============================
echo "步骤 1/5: 统计读段总数..."

TOTAL1=$(grep -v "^@" $SAM1 | wc -l | awk '{print $1}')
TOTAL2=$(grep -v "^@" $SAM2 | wc -l | awk '{print $1}')

echo "SAM1 (BSMAP C++) 读段数: $TOTAL1"
echo "SAM2 (bsmap-rs) 读段数: $TOTAL2"
echo "" >> $REPORT
echo "【读段总数】" >> $REPORT
echo "  BSMAP C++: $TOTAL1" >> $REPORT
echo "  bsmap-rs: $TOTAL2" >> $REPORT
echo "  差异: $(echo $TOTAL1 - $TOTAL2 | bc)" >> $REPORT
echo "" >> $REPORT

# ===============================
# 2. 提取比对信息（按读段名）
# ===============================
echo "步骤 2/5: 提取比对信息..."

grep -v "^@" $SAM1 | awk '{print $1"\t"$2"\t"$3"\t"$4}' > $OUT_DIR/sam1_info.txt
grep -v "^@" $SAM2 | awk '{print $1"\t"$2"\t"$3"\t"$4}' > $OUT_DIR/sam2_info.txt

# 统计比对状态
UNMAPPED1=$(awk '$2 == 4' $OUT_DIR/sam1_info.txt | wc -l | awk '{print $1}')
UNMAPPED2=$(awk '$2 == 4' $OUT_DIR/sam2_info.txt | wc -l | awk '{print $1}')
MAPPED1=$(expr $TOTAL1 - $UNMAPPED1 || 0)
MAPPED2=$(expr $TOTAL2 - $UNMAPPED2 || 0)

echo "BSMAP C++: 比对 $MAPPED1, 未比对 $UNMAPPED1"
echo "bsmap-rs: 比对 $MAPPED2, 未比对 $UNMAPPED2"
echo "" >> $REPORT
echo "【比对状态统计】" >> $REPORT
echo "  BSMAP C++:" >> $REPORT
echo "    比对: $MAPPED1" >> $REPORT
echo "    未比对: $UNMAPPED1" >> $REPORT
echo "  bsmap-rs:" >> $REPORT
echo "    比对: $MAPPED2" >> $REPORT
echo "    未比对: $UNMAPPED2" >> $REPORT
echo "" >> $REPORT

# ===============================
# 3. 唯一比对 vs 多重比对
# ===============================
echo "步骤 3/5: 分析唯一/多重比对..."

# 使用flag 256表示次级比对
UNIQUE1=$(grep -v "^@" $SAM1 | awk '($2 != 4 && !and($2, 256))' | wc -l | awk '{print $1}')
MULTI1=$(grep -v "^@" $SAM1 | awk '($2 != 4 && and($2, 256))' | wc -l | awk '{print $1}')

UNIQUE2=$(grep -v "^@" $SAM2 | awk '($2 != 4 && !and($2, 256))' | wc -l | awk '{print $1}')
MULTI2=$(grep -v "^@" $SAM2 | awk '($2 != 4 && and($2, 256))' | wc -l | awk '{print $1}')

echo "" >> $REPORT
echo "【唯一比对 vs 多重比对】" >> $REPORT
echo "  BSMAP C++:" >> $REPORT
echo "    唯一比对: $UNIQUE1" >> $REPORT
echo "    多重比对: $MULTI1" >> $REPORT
echo "  bsmap-rs:" >> $REPORT
echo "    唯一比对: $UNIQUE2" >> $REPORT
echo "    多重比对: $MULTI2" >> $REPORT
echo "" >> $REPORT

# ===============================
# 4. 按读段名join比较 (简化版，只比较读段名相同的部分)
# ===============================
echo "步骤 4/5: 按读段名比较一致性..."

# 提取读段名
cut -f1 $OUT_DIR/sam1_info.txt | sort > $OUT_DIR/reads1.txt
cut -f1 $OUT_DIR/sam2_info.txt | sort > $OUT_DIR/reads2.txt

# 找到共同的读段
comm -12 $OUT_DIR/reads1.txt $OUT_DIR/reads2.txt > $OUT_DIR/common_reads.txt

TOTAL_COMMON=$(wc -l $OUT_DIR/common_reads.txt | awk '{print $1}')
echo "共同读段数: $TOTAL_COMMON"

# 快速统计：分别统计都比对、都未比对等
BOTH_MAPPED=0
BOTH_UNMAPPED=0
MAPPED1_ONLY=0
MAPPED2_ONLY=0
SAME_POS=0
DIFF_POS=0
DIFF_REF=0

# 建立SAM1的索引（读段名 -> 比对信息）
awk '{map[$1] = $2"\t"$3"\t"$4} END {for (k in map) print k"\t"map[k]}' $OUT_DIR/sam1_info.txt > $OUT_DIR/sam1_map.txt
awk '{map[$1] = $2"\t"$3"\t"$4} END {for (k in map) print k"\t"map[k]}' $OUT_DIR/sam2_info.txt > $OUT_DIR/sam2_map.txt

# 读取到内存
while read line; do
  read_id=$(echo $line | awk '{print $1}')
  info1=$(grep "^$read_id" $OUT_DIR/sam1_map.txt | head -1)
  info2=$(grep "^$read_id" $OUT_DIR/sam2_map.txt | head -1)
  
  if [ -n "$info1" ] && [ -n "$info2" ]; then
    flag1=$(echo $info1 | awk '{print $2}')
    ref1=$(echo $info1 | awk '{print $3}')
    pos1=$(echo $info1 | awk '{print $4}')
    
    flag2=$(echo $info2 | awk '{print $2}')
    ref2=$(echo $info2 | awk '{print $3}')
    pos2=$(echo $info2 | awk '{print $4}')
    
    unmapped1=$( [ "$flag1" -eq 4 ] && echo 1 || echo 0 )
    unmapped2=$( [ "$flag2" -eq 4 ] && echo 1 || echo 0 )
    
    if [ $unmapped1 -eq 1 ] && [ $unmapped2 -eq 1 ]; then
      BOTH_UNMAPPED=$(expr $BOTH_UNMAPPED + 1 || 0)
    elif [ $unmapped1 -eq 0 ] && [ $unmapped2 -eq 0 ]; then
      BOTH_MAPPED=$(expr $BOTH_MAPPED + 1 || 0)
      if [ "$ref1" = "$ref2" ] && [ "$pos1" = "$pos2" ]; then
        SAME_POS=$(expr $SAME_POS + 1 || 0)
      elif [ "$ref1" != "$ref2" ]; then
        DIFF_REF=$(expr $DIFF_REF + 1 || 0)
      else
        DIFF_POS=$(expr $DIFF_POS + 1 || 0)
      fi
    elif [ $unmapped1 -eq 0 ]; then
      MAPPED1_ONLY=$(expr $MAPPED1_ONLY + 1 || 0)
    else
      MAPPED2_ONLY=$(expr $MAPPED2_ONLY + 1 || 0)
    fi
  fi
done < $OUT_DIR/common_reads.txt

echo "" >> $REPORT
echo "【比对一致性分析】" >> $REPORT
echo "  共同读段数: $TOTAL_COMMON" >> $REPORT
echo "  都未比对: $BOTH_UNMAPPED" >> $REPORT
echo "  都比对: $BOTH_MAPPED" >> $REPORT
echo "    都比对且位置一致: $SAME_POS" >> $REPORT
echo "    都比对但位置不同: $DIFF_POS" >> $REPORT
echo "    都比对但参考序列不同: $DIFF_REF" >> $REPORT
echo "  BSMAP C++比对但bsmap-rs未比对: $MAPPED1_ONLY" >> $REPORT
echo "  bsmap-rs比对但BSMAP C++未比对: $MAPPED2_ONLY" >> $REPORT

# 计算百分比
if [ $TOTAL_COMMON -gt 0 ]; then
  echo "" >> $REPORT
  echo "  百分比:" >> $REPORT
  echo "    都未比对: $(echo "scale=2; $BOTH_UNMAPPED * 100 / $TOTAL_COMMON" | bc)%" >> $REPORT
  echo "    都比对: $(echo "scale=2; $BOTH_MAPPED * 100 / $TOTAL_COMMON" | bc)%" >> $REPORT
  if [ $BOTH_MAPPED -gt 0 ]; then
    echo "      都比对且位置一致: $(echo "scale=2; $SAME_POS * 100 / $BOTH_MAPPED" | bc)%" >> $REPORT
  fi
fi

echo "" >> $REPORT

# ===============================
# 5. 写CSV汇总
# ===============================
echo "步骤 5/5: 生成汇总..."
cat > $OUT_DIR/comparison_summary.csv << CSV
example,total_reads,both_unmapped,both_mapped_same,both_mapped_different_pos,both_mapped_different_ref,mapped1_unmapped2,mapped2_unmapped1
$EXAMPLE_NAME,$TOTAL_COMMON,$BOTH_UNMAPPED,$SAME_POS,$DIFF_POS,$DIFF_REF,$MAPPED1_ONLY,$MAPPED2_ONLY
CSV

echo ""
echo "============================================="
echo "分析完成!"
echo "报告文件: $REPORT"
echo "============================================="
cat $REPORT
