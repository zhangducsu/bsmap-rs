#!/usr/bin/env python3
"""Compare SAM files between C++ BSMAP and Rust bsmap-rs."""

import sys

def parse_sam(path):
    """Parse SAM file and return dict keyed by QNAME."""
    results = {}
    with open(path) as f:
        for line in f:
            if line.startswith('@'):
                continue
            fields = line.strip().split('\t')
            qname = fields[0]
            flag = int(fields[1])
            rname = fields[2]
            pos = int(fields[3])
            mapq = int(fields[4])
            cigar = fields[5]
            results[qname] = (flag, rname, pos, mapq, cigar)
    return results

def compare_sams(cpp_path, rs_path, test_name):
    cpp_sam = parse_sam(cpp_path)
    rs_sam = parse_sam(rs_path)
    
    print(f"{'=' * 60}")
    print(f"{test_name} 比对结果对比")
    print(f"{'=' * 60}")
    print(f"C++ SAM 记录数: {len(cpp_sam)}")
    print(f"Rust SAM 记录数: {len(rs_sam)}")
    
    cpp_only = set(cpp_sam.keys()) - set(rs_sam.keys())
    rs_only = set(rs_sam.keys()) - set(cpp_sam.keys())
    common = set(cpp_sam.keys()) & set(rs_sam.keys())
    
    print(f"共同记录数: {len(common)}")
    print(f"C++ 独有记录: {len(cpp_only)}")
    print(f"Rust 独有记录: {len(rs_only)}")
    
    match_pos = 0
    mismatch_pos = 0
    strand_match = 0
    strand_mismatch = 0
    
    for qname in common:
        cpp_fields = cpp_sam[qname]
        rs_fields = rs_sam[qname]
        if cpp_fields == rs_fields:
            match_pos += 1
        else:
            mismatch_pos += 1
            # Check strand consistency (bit 4 of FLAG)
            cpp_strand = (cpp_fields[0] & 0x10) != 0
            rs_strand = (rs_fields[0] & 0x10) != 0
            if cpp_strand == rs_strand:
                strand_match += 1
            else:
                strand_mismatch += 1
    
    print(f"完全一致: {match_pos}")
    print(f"位置/FLAG不同: {mismatch_pos}")
    print(f"  - 链方向一致: {strand_match}")
    print(f"  - 链方向不同: {strand_mismatch}")
    
    if len(common) > 0:
        consistency_rate = match_pos / len(common) * 100
        print(f"位置一致率: {consistency_rate:.2f}%")
    
    return {
        'cpp_count': len(cpp_sam),
        'rs_count': len(rs_sam),
        'common': len(common),
        'cpp_only': len(cpp_only),
        'rs_only': len(rs_only),
        'match': match_pos,
        'mismatch': mismatch_pos,
        'strand_match': strand_match,
        'strand_mismatch': strand_mismatch
    }

if __name__ == '__main__':
    print("SAM 一致性对比测试")
    print()
    
    r1 = compare_sams(
        'test_results_example1/bsmap.sam', 
        'test_results_example1/bsmaprs.sam',
        'Ex1 (单端 75bp)'
    )
    
    print()
    
    r2 = compare_sams(
        'test_results_example2/bsmap.sam', 
        'test_results_example2/bsmaprs.sam',
        'Ex2 (双端 150bp)'
    )
    
    print()
    print("=" * 60)
    print("总结")
    print("=" * 60)
    total = r1['common'] + r2['common']
    total_match = r1['match'] + r2['match']
    if total > 0:
        print(f"总一致率: {total_match / total * 100:.2f}%")
