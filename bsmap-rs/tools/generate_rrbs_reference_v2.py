#!/usr/bin/env python3
"""
生成RRBS标准模拟DNA序列 v2
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
MSP1_SPACING_MIN = 150
MSP1_SPACING_MAX = 300
FRAGMENT_SIZE_MIN = 50
FRAGMENT_SIZE_MAX = 300

# CpG密度定义 (CpG位点 / 100bp)
CPG_ISLAND_DENSITY = 8.0      # CpG岛: 高CpG密度
CPG_MEDIUM_DENSITY = 4.0      # 中等CpG区
CPG_LOW_DENSITY = 1.0         # 低CpG区

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

def generate_balanced_seq(length, target_gc):
    """生成指定长度和GC含量的平衡序列"""
    gc_needed = int(length * target_gc)
    at_needed = length - gc_needed
    
    gc_bases = ['G', 'C'] * (gc_needed // 2 + 1)
    at_bases = ['A', 'T'] * (at_needed // 2 + 1)
    
    base_pool = (gc_bases[:gc_needed] + at_bases[:at_needed])
    random.shuffle(base_pool)
    
    return ''.join(base_pool[:length])

def generate_cpg_rich_block(length, target_cpg_density):
    """生成富含CpG的序列块"""
    seq = []
    gc_target = 0.60 if target_cpg_density > 6 else 0.50
    
    for i in range(length):
        if i > 0 and seq[-1] == 'C' and random.random() < (target_cpg_density / 100):
            seq.append('G')
        else:
            if random.random() < gc_target:
                seq.append(random.choice(['G', 'C']))
            else:
                seq.append(random.choice(['A', 'T']))
    
    return ''.join(seq)

def generate_cpg_poor_block(length):
    """生成低CpG序列块"""
    seq = []
    for i in range(length):
        if i > 0 and seq[-1] == 'C':
            seq.append(random.choice(['A', 'T', 'C']))
        else:
            if random.random() < 0.35:
                seq.append(random.choice(['G', 'C']))
            else:
                seq.append(random.choice(['A', 'T']))
    return ''.join(seq)

def adjust_gc_content(seq, target_gc, tolerance):
    """调整序列GC含量到目标范围"""
    seq = list(seq)
    current_gc = gc_content(seq)
    max_iterations = 5000
    iteration = 0
    
    while abs(current_gc - target_gc) > tolerance and iteration < max_iterations:
        iteration += 1
        if current_gc < target_gc:
            at_positions = [i for i, b in enumerate(seq) if b in ['A', 'T']]
            if not at_positions:
                break
            pos = random.choice(at_positions)
            seq[pos] = random.choice(['G', 'C'])
        else:
            gc_positions = [i for i, b in enumerate(seq) if b in ['G', 'C']]
            if not gc_positions:
                break
            pos = random.choice(gc_positions)
            seq[pos] = random.choice(['A', 'T'])
        current_gc = gc_content(seq)
    
    return ''.join(seq)

def insert_msp1_sites_strict(seq, min_spacing, max_spacing):
    """
    严格插入MspI(CCGG)位点，确保位点间距在指定范围内
    返回插入位点后的序列和位点位置列表
    """
    length = len(seq)
    
    # 计算需要的MspI位点数量
    # 平均间距约225bp，所以大约需要 length/225 个位点
    estimated_sites = length // ((min_spacing + max_spacing) // 2)
    
    # 生成位点位置，确保间距
    positions = []
    current_pos = random.randint(100, 300)  # 第一个位点位置
    
    while current_pos < length - 100:
        positions.append(current_pos)
        spacing = random.randint(min_spacing, max_spacing)
        current_pos += spacing
    
    # 确保最后一个位点不会导致片段太长
    if positions and length - positions[-1] > max_spacing:
        positions.append(length - random.randint(50, 100))
    
    # 严格过滤，确保最小间距
    filtered_positions = []
    last_pos = -min_spacing
    for pos in sorted(positions):
        if pos - last_pos >= min_spacing and pos + 4 <= length:
            filtered_positions.append(pos)
            last_pos = pos
    
    # 从后往前替换为CCGG
    seq_list = list(seq)
    for pos in sorted(filtered_positions, reverse=True):
        seq_list[pos:pos+4] = list('CCGG')
    
    return ''.join(seq_list), filtered_positions

# ==================== 主生成流程 ====================
def generate_rrbs_reference():
    """生成完整的RRBS参考基因组"""
    
    print("=" * 60)
    print("RRBS标准模拟DNA序列生成器 v2")
    print("=" * 60)
    
    # 步骤1: 生成基础序列（先生成稍长的序列，插入MspI后再截断）
    print("\n【步骤1】生成基础序列...")
    
    # 3个特殊区块，每个约3000bp
    block_size = 3000
    
    blocks = [
        ("CpG岛区", CPG_ISLAND_DENSITY),
        ("中等CpG区", CPG_MEDIUM_DENSITY),
        ("低CpG区", CPG_LOW_DENSITY),
    ]
    random.shuffle(blocks)
    
    sequences = []
    for name, density in blocks:
        print(f"  生成 {name}: {block_size}bp, 目标CpG密度={density}/100bp")
        if density >= 6:
            seq = generate_cpg_rich_block(block_size, density)
        elif density >= 3:
            seq = generate_balanced_seq(block_size, TARGET_GC)
        else:
            seq = generate_cpg_poor_block(block_size)
        
        actual_density = cpg_density(seq)
        actual_gc = gc_content(seq)
        print(f"    实际: GC={actual_gc:.2%}, CpG密度={actual_density:.2f}/100bp")
        sequences.append(seq)
    
    # 填充序列，使总长度达到目标
    current_length = len(''.join(sequences))
    padding_needed = TARGET_LENGTH - current_length
    
    if padding_needed > 0:
        print(f"  生成填充序列: {padding_needed}bp")
        padding = generate_balanced_seq(padding_needed, TARGET_GC)
        sequences.append(padding)
    
    raw_seq = ''.join(sequences)
    print(f"  基础序列总长度: {len(raw_seq):,}bp")
    
    # 步骤2: 调整GC含量
    print("\n【步骤2】调整GC含量...")
    seq = adjust_gc_content(raw_seq, TARGET_GC, GC_TOLERANCE)
    final_gc = gc_content(seq)
    print(f"  调整后GC含量: {final_gc:.2%}")
    
    # 步骤3: 插入MspI位点
    print(f"\n【步骤3】插入MspI(CCGG)位点...")
    print(f"  目标间距: {MSP1_SPACING_MIN}-{MSP1_SPACING_MAX}bp")
    seq, msp1_positions = insert_msp1_sites_strict(seq, MSP1_SPACING_MIN, MSP1_SPACING_MAX)
    
    # 步骤4: 最终验证
    print("\n【步骤4】最终验证...")
    final_gc = gc_content(seq)
    msp1_count = count_msp1_sites(seq)
    fragments = get_msp1_fragments(seq)
    valid_fragments = [f for f in fragments if FRAGMENT_SIZE_MIN <= f <= FRAGMENT_SIZE_MAX]
    
    # 计算MspI位点间距
    if len(msp1_positions) > 1:
        spacings = [msp1_positions[i+1] - msp1_positions[i] for i in range(len(msp1_positions)-1)]
        min_spacing = min(spacings)
        max_spacing = max(spacings)
        mean_spacing = sum(spacings) / len(spacings)
    else:
        min_spacing = max_spacing = mean_spacing = 0
    
    print(f"  序列长度: {len(seq):,} bp")
    print(f"  GC含量: {final_gc:.2%}")
    print(f"  MspI(CCGG)位点数: {msp1_count}")
    print(f"  位点间距: min={min_spacing}, max={max_spacing}, mean={mean_spacing:.0f} bp")
    print(f"  酶切片段数: {len(fragments)}")
    print(f"  有效片段(50-300bp): {len(valid_fragments)} ({len(valid_fragments)/len(fragments)*100:.1f}%)")
    
    return seq, blocks, msp1_positions

def write_fasta(seq, filename, seq_name="rrbs_reference_48.5kb"):
    """写入FASTA文件"""
    with open(filename, 'w') as f:
        f.write(f">{seq_name}\n")
        for i in range(0, len(seq), 80):
            f.write(seq[i:i+80] + '\n')
    print(f"\n【输出】FASTA文件已保存: {filename}")

def generate_statistics(seq, msp1_positions, output_file=None):
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
    report.append("【MspI(CCGG)位点统计】")
    report.append(f"位点总数: {len(msp1_positions)}")
    
    if len(msp1_positions) > 1:
        spacings = [msp1_positions[i+1] - msp1_positions[i] for i in range(len(msp1_positions)-1)]
        report.append(f"位点间距: min={min(spacings)}, max={max(spacings)}, mean={sum(spacings)/len(spacings):.0f} bp")
    report.append("")
    
    # 片段长度分布
    fragments = get_msp1_fragments(seq)
    report.append("【酶切片段长度分布】")
    report.append(f"总片段数: {len(fragments)}")
    
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
    seq, blocks, msp1_positions = generate_rrbs_reference()
    
    # 输出FASTA
    fasta_file = f"{output_dir}/reference/random_genome.fa"
    write_fasta(seq, fasta_file)
    
    # 输出统计报告
    stats_file = f"{output_dir}/reference/statistics.txt"
    report = generate_statistics(seq, msp1_positions, stats_file)
    
    # 打印报告
    print("\n" + report)
    
    print("\n" + "=" * 60)
    print("生成完成！")
    print(f"输出目录: {output_dir}")
    print("=" * 60)
