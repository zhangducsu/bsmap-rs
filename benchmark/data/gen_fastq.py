#!/usr/bin/env python3
"""生成模拟 WGBS/RRBS FASTQ 数据"""
import random
import sys
import argparse


def load_fasta(ref_fasta):
    """加载 FASTA 文件，返回序列字符串"""
    seq = []
    with open(ref_fasta) as f:
        for line in f:
            line = line.strip()
            if line.startswith(">"):
                continue
            seq.append(line)
    return "".join(seq)


def generate_wgbs_fastq(output, ref_seq, n_reads, read_length, seed=42):
    """生成 WGBS FASTQ（随机位置）"""
    random.seed(seed)
    for i in range(n_reads):
        pos = random.randint(0, len(ref_seq) - read_length)
        read_seq = ref_seq[pos:pos+read_length]
        qual = "I" * read_length
        output.write(f"@read_{i+1}\n")
        output.write(read_seq + "\n")
        output.write("+\n")
        output.write(qual + "\n")


def generate_rrbs_fastq(output, ref_seq, n_reads, read_length, seed=42):
    """生成 RRBS FASTQ（MspI 酶切位点 CCGG 附近）"""
    # 找所有 CCGG 位点
    cut_sites = [i for i in range(len(ref_seq)-3) if ref_seq[i:i+4] == "CCGG"]
    if not cut_sites:
        raise ValueError("No CCGG sites found in reference sequence")
    
    random.seed(seed)
    for i in range(n_reads):
        pos = random.choice(cut_sites)
        # 从 CCGG 位点开始，向后取 read_length
        start = pos
        end = min(len(ref_seq), start + read_length)
        read_seq = ref_seq[start:end]
        # 如果序列太短，前面补 N
        if len(read_seq) < read_length:
            read_seq = "N" * (read_length - len(read_seq)) + read_seq
        qual = "I" * read_length
        output.write(f"@read_{i+1}\n")
        output.write(read_seq + "\n")
        output.write("+\n")
        output.write(qual + "\n")


def main():
    parser = argparse.ArgumentParser(description="Generate simulated FASTQ data")
    parser.add_argument("-r", "--ref", required=True, help="Reference FASTA file")
    parser.add_argument("-n", "--reads", type=int, required=True, help="Number of reads")
    parser.add_argument("-l", "--length", type=int, required=True, help="Read length")
    parser.add_argument("-o", "--output", required=True, help="Output FASTQ file")
    parser.add_argument("--mode", choices=["wgbs", "rrbs"], default="wgbs", help="Sequencing mode")
    parser.add_argument("--seed", type=int, default=42, help="Random seed")
    args = parser.parse_args()

    ref_seq = load_fasta(args.ref)
    print(f"Loaded reference: {len(ref_seq)} bp", file=sys.stderr)

    with open(args.output, "w") as f:
        if args.mode == "wgbs":
            generate_wgbs_fastq(f, ref_seq, args.reads, args.length, args.seed)
        else:
            generate_rrbs_fastq(f, ref_seq, args.reads, args.length, args.seed)

    print(f"Generated {args.reads} reads to {args.output}", file=sys.stderr)


if __name__ == "__main__":
    main()
