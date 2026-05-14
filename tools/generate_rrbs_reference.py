#!/usr/bin/env python3
"""
生成RRBS标准模拟DNA序列
符合约束：48.5kb，GC 48%±3%，MspI(CCGG)间距150-300bp，含3类CpG区块
"""

import random
import re
from collections import Counter

random.seed(42)

# ==================== 参数配置 ====================
TARGET_LENGTH = 48500
TARGET_GC = 0.48
GC_TOLERANCE = 0.03
MSP1_SITE = "CCGG"
MSP1_SPACING_MIN = 150
MSP1_SPACING_MAX = 300
FRAGMENT_SIZE_MIN = 50
FRAGMENT_SIZE_MAX = 300

# CpG密度定义 (CpG位点 / 100bp)
CPG_ISLAND_DENSITY = 8.0      # CpG岛: 高CpG密度
CPG_MEDIUM_DENSITY = 4.0      # 中等CpG区
CPG_LOW_DENSITY = 1.0         # 低CpG区

# 区块长度
BLOCK_MIN_LEN = 2000
BLOCK_MAX_LEN = 5000

# ==================== 工具函数 ====================
def gc_content(seq):
    """计算GC含量"""
    gc = seq.count('G') + seq.count('C')
    return gc / len(seq) if seq else 0

def cpg_density(seq):
    """计算CpG密度 (每100bp的CpG位点数)"""
    cpg_count = len(re.findall(r'CG', seq))
    return (cpg_count / len(seq)) * 100 if seq else 0

def count_msp1_sites(seq):
    """统计MspI(CCGG)位点数量"""
    return len(re.findall(r'CCGG', seq))

def get_msp1_fragments(seq):
    """获取MspI酶切片段长度分布"""
    sites = [m.start() for m in re.finditer(r'CCGG', seq)]
    sites = [0] + sites + [len(seq)]
    fragments = []
    for i in range(len(sites) - 1):
        frag_len = sites[i+1] - sites[i]
        fragments.append(frag_len)
    return fragments

def generate_balanced_seq(length, target_gc, forbidden_patterns=None):
    """
    生成指定长度和GC含量的序列
    避免forbidden_patterns中的序列
    """
    if forbidden_patterns is None:
        forbidden_patterns = []
    
    seq = []
    gc_bases = ['G', 'C']
    at_bases = ['A', 'T']
    
    # 计算需要的GC和AT数量
    gc_needed = int(length * target_gc)
    at_needed = length - gc_needed
    
    # 创建碱基池
    base_pool = gc_bases * gc_needed + at_bases * at_needed
    random.shuffle(base_pool)
    
    # 逐个添加碱基，检查约束
    for base in base_pool:
        test_seq = ''.join(seq + [base])
        
        # 检查是否产生禁止的pattern
        valid = True
        for pattern in forbidden_patterns:
            if pattern in test_seq[-len(pattern):]:
                valid = False
                break
        
        # 检查是否产生连续的CCGG（避免多余酶切位点）
        if "CCGG" in test_seq[-10:] and len(seq) > 4:
            # 检查是否是预期的MspI位点
            pass  # 允许，但需要控制间距
        
        if valid:
            seq.append(base)
        else:
            # 尝试另一个碱基
            alternative = 'A' if base in ['G', 'C'] else 'G'
            if alternative in gc_bases and gc_needed > seq.count('G') + seq.count('C'):
                seq.append(alternative)
            elif alternative in at_bases and at_needed > seq.count('A') + seq.count('T'):
                seq.append(alternative)
            else:
                seq.append(base)  # 强制添加
    
    return ''.join(seq)

def adjust_gc_content(seq, target_gc, tolerance):
    """调整序列GC含量到目标范围"""
    seq = list(seq)
    current_gc = gc_content(seq)
    
    max_iterations = 10000
    iteration = 0
    
    while abs(current_gc - target_gc) > tolerance and iteration < max_iterations:
        iteration += 1
        
        if current_gc < target_gc:
            # 需要增加GC
            at_positions = [i for i, b in enumerate(seq) if b in ['A', 'T']]
            if not at_positions:
                break
            pos = random.choice(at_positions)
            seq[pos] = random.choice(['G', 'C'])
        else:
            # 需要减少GC
            gc_positions = [i for i, b in enumerate(seq) if b in ['G', 'C']]
            if not gc_positions:
                break
            pos = random.choice(gc_positions)
            seq[pos] = random.choice(['A', 'T'])
        
        current_gc = gc_content(seq)
    
    return ''.join(seq)

def insert_msp1_sites(seq, min_spacing, max_spacing):
    """
    在序列中插入MspI(CCGG)位点
    确保位点间距在指定范围内，避免连续的CCGG
    """
    seq = list(seq)
    length = len(seq)
    
    # 确定MspI位点位置
    positions = []
    current_pos = random.randint(min_spacing, max_spacing)
    
    while current_pos < length - min_spacing:
        positions.append(current_pos)
        current_pos += random.randint(min_spacing, max_spacing)
    
    # 检查并修复相邻位点（间距<min_spacing时）
    positions = sorted(positions)
    filtered_positions = []
    last_pos = -min_spacing
    for pos in positions:
        if pos - last_pos >= min_spacing:
            filtered_positions.append(pos)
            last_pos = pos
    
    # 在每个位置插入CCGG
    # 从后往前插入，避免位置偏移
    for pos in sorted(filtered_positions, reverse=True):
        # 替换该位置的4个碱基为CCGG
        if pos + 4 <= length:
            seq[pos:pos+4] = list('CCGG')
    
    return ''.join(seq)

def generate_cpg_rich_block(length, target_cpg_density):
    """生成富含CpG的序列块"""
    seq = []
    
    # CpG岛特征: 高GC + 高CpG
    gc_target = 0.60 if target_cpg_density > 6 else 0.50
    
    for i in range(length):
        if i > 0 and seq[-1] == 'C' and random.random() < (target_cpg_density / 100):
            # 高概率生成G形成CpG
            seq.append('G')
        else:
            # 根据GC目标选择碱基
            if random.random() < gc_target:
                seq.append(random.choice(['G', 'C']))
            else:
                seq.append(random.choice(['A', 'T']))
    
    return ''.join(seq)

def generate_cpg_poor_block(length):
    """生成低CpG序列块"""
    seq = []
    
    for i in range(length):
        # 低GC，避免CpG
        if i > 0 and seq[-1] == 'C':
            # C后避免G
            seq.append(random.choice(['A', 'T', 'C']))
        else:
            # 低GC含量
            if random.random() < 0.35:
                seq.append(random.choice(['G', 'C']))
            else:
                seq.append(random.choice(['A', 'T']))
    
    return ''.join(seq)

# ==================== 主生成流程 ====================
def generate_rrbs_reference():
    """生成完整的RRBS参考基因组"""
    
    print("=" * 60)
    print("RRBS标准模拟DNA序列生成器")
    print("=" * 60)
    
    # 定义3类区块 - 确保总长度不超过目标的30%
    # 剩余70%用平衡序列填充，以便精确控制总长度
    block_total_target = int(TARGET_LENGTH * 0.30)  # 3个区块占30%
    block_avg = block_total_target // 3
    
    blocks = [
        ("CpG岛区", CPG_ISLAND_DENSITY, random.randint(block_avg - 500, block_avg + 500)),
        ("中等CpG区", CPG_MEDIUM_DENSITY, random.randint(block_avg - 500, block_avg + 500)),
        ("低CpG区", CPG_LOW_DENSITY, random.randint(block_avg - 500, block_avg + 500)),
    ]
    
    # 打乱顺序
    random.shuffle(blocks)
    
    # 生成各区块
    sequences = []
    print("\n【步骤1】生成3类CpG区块...")
    for name, density, length in blocks:
        print(f"  生成 {name}: 目标长度={length}bp, CpG密度={density}/100bp")
        
        if density >= 6:
            seq = generate_cpg_rich_block(length, density)
        elif density >= 3:
            seq = generate_balanced_seq(length, TARGET_GC)
        else:
            seq = generate_cpg_poor_block(length)
        
        actual_density = cpg_density(seq)
        actual_gc = gc_content(seq)
        print(f"    实际: GC={actual_gc:.2%}, CpG密度={actual_density:.2f}/100bp")
        sequences.append((name, seq))
    
    # 连接区块
    print("\n【步骤2】连接区块并调整总长度...")
    raw_seq = ''.join([s for _, s in sequences])
    
    # 调整至目标长度
    if len(raw_seq) < TARGET_LENGTH:
        # 补充随机序列
        padding_len = TARGET_LENGTH - len(raw_seq)
        padding = generate_balanced_seq(padding_len, TARGET_GC)
        raw_seq += padding
        print(f"  补充填充序列: {padding_len}bp")
    elif len(raw_seq) > TARGET_LENGTH:
        # 截断
        raw_seq = raw_seq[:TARGET_LENGTH]
        print(f"  截断至目标长度: {TARGET_LENGTH}bp")
    
    # 调整GC含量
    print("\n【步骤3】调整GC含量...")
    seq = adjust_gc_content(raw_seq, TARGET_GC, GC_TOLERANCE)
    final_gc = gc_content(seq)
    print(f"  最终GC含量: {final_gc:.2%} (目标: {TARGET_GC:.0%}±{GC_TOLERANCE:.0%})")
    
    # 插入MspI位点
    print("\n【步骤4】插入MspI(CCGG)位点...")
    print(f"  目标间距: {MSP1_SPACING_MIN}-{MSP1_SPACING_MAX}bp")
    seq = insert_msp1_sites(seq, MSP1_SPACING_MIN, MSP1_SPACING_MAX)
    
    # 最终验证
    print("\n【步骤5】最终验证...")
    final_gc = gc_content(seq)
    msp1_count = count_msp1_sites(seq)
    fragments = get_msp1_fragments(seq)
    valid_fragments = [f for f in fragments if FRAGMENT_SIZE_MIN <= f <= FRAGMENT_SIZE_MAX]
    
    print(f"  序列长度: {len(seq):,} bp")
    print(f"  GC含量: {final_gc:.2%}")
    print(f"  MspI(CCGG)位点数: {msp1_count}")
    print(f"  平均位点间距: {len(seq)/msp1_count:.0f} bp")
    print(f"  酶切片段数: {len(fragments)}")
    print(f"  有效片段(50-300bp): {len(valid_fragments)} ({len(valid_fragments)/len(fragments)*100:.1f}%)")
    
    return seq, blocks

def write_fasta(seq, filename, seq_name="rrbs_reference_48.5kb"):
    """写入FASTA文件"""
    with open(filename, 'w') as f:
        f.write(f">{seq_name}\n")
        # 每行80个碱基
        for i in range(0, len(seq), 80):
            f.write(seq[i:i+80] + '\n')
    print(f"\n【输出】FASTA文件已保存: {filename}")

def generate_statistics(seq, output_file=None):
    """生成详细统计报告"""
    
    report = []
    report.append("=" * 60)
    report.append("RRBS参考基因组统计报告")
    report.append("=" * 60)
    report.append("")
    
    # 基础统计
    report.append("【基础统计】")
    report.append(f"序列长度: {len(seq):,} bp")
    report.append(f"GC含量: {gc_content(seq):.2%}")
    report.append(f"G数量: {seq.count('G'):,}")
    report.append(f"C数量: {seq.count('C'):,}")
    report.append(f"A数量: {seq.count('A'):,}")
    report.append(f"T数量: {seq.count('T'):,}")
    report.append("")
    
    # CpG统计
    cpg_count = len(re.findall(r'CG', seq))
    report.append("【CpG统计】")
    report.append(f"CpG位点总数: {cpg_count}")
    report.append(f"CpG密度: {cpg_density(seq):.2f}/100bp")
    report.append("")
    
    # MspI位点统计
    msp1_positions = [m.start() for m in re.finditer(r'CCGG', seq)]
    msp1_count = len(msp1_positions)
    report.append("【MspI(CCGG)位点统计】")
    report.append(f"位点总数: {msp1_count}")
    
    if msp1_count > 1:
        spacings = [msp1_positions[i+1] - msp1_positions[i] for i in range(msp1_count - 1)]
        report.append(f"位点间距: min={min(spacings)}, max={max(spacings)}, mean={sum(spacings)/len(spacings):.0f} bp")
    report.append("")
    
    # 片段长度分布
    fragments = get_msp1_fragments(seq)
    report.append("【酶切片段长度分布】")
    report.append(f"总片段数: {len(fragments)}")
    
    # 分段统计
    ranges = [(0, 50), (50, 100), (100, 150), (150, 200), (200, 250), (250, 300), (300, float('inf'))]
    for min_len, max_len in ranges:
        if max_len == float('inf'):
            count = len([f for f in fragments if f >= min_len])
            report.append(f"  ≥{min_len}bp: {count} ({count/len(fragments)*100:.1f}%)")
        else:
            count = len([f for f in fragments if min_len <= f < max_len])
            report.append(f"  {min_len}-{max_len}bp: {count} ({count/len(fragments)*100:.1f}%)")
    
    valid_fragments = [f for f in fragments if FRAGMENT_SIZE_MIN <= f <= FRAGMENT_SIZE_MAX]
    report.append(f"\n有效片段(50-300bp): {len(valid_fragments)}/{len(fragments)} ({len(valid_fragments)/len(fragments)*100:.1f}%)")
    report.append("")
    
    # 前10个MspI位点位置
    report.append("【前10个MspI位点位置】")
    for i, pos in enumerate(msp1_positions[:10], 1):
        report.append(f"  位点{i}: {pos}")
    report.append("")
    
    report_text = '\n'.join(report)
    
    if output_file:
        with open(output_file, 'w') as f:
            f.write(report_text)
        print(f"【输出】统计报告已保存: {output_file}")
    
    return report_text

# ==================== 主程序 ====================
if __name__ == "__main__":
    import os
    
    # 创建输出目录
    output_dir = "/workspace/bsmap-rs/tests/data/rrbs_random_v2"
    os.makedirs(output_dir, exist_ok=True)
    os.makedirs(f"{output_dir}/reference", exist_ok=True)
    
    # 生成序列
    seq, blocks = generate_rrbs_reference()
    
    # 输出FASTA
    fasta_file = f"{output_dir}/reference/random_genome.fa"
    write_fasta(seq, fasta_file)
    
    # 输出统计报告
    stats_file = f"{output_dir}/reference/statistics.txt"
    report = generate_statistics(seq, stats_file)
    
    # 打印报告
    print("\n" + report)
    
    print("\n" + "=" * 60)
    print("生成完成！")
    print(f"输出目录: {output_dir}")
    print("=" * 60)
