#!/usr/bin/env python3
import argparse
import json
from collections import Counter


def parse_sam(path):
    records = {}
    flags = Counter()
    rnames = Counter()
    total = mapped = unmapped = unique = multiple = 0

    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            if not line or line.startswith("@"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 6:
                continue
            qname = fields[0]
            flag = int(fields[1])
            rname = fields[2]
            pos = int(fields[3])
            mapq = int(fields[4])
            cigar = fields[5]

            total += 1
            flags[str(flag)] += 1
            rnames[rname] += 1
            if flag & 0x4:
                unmapped += 1
            else:
                mapped += 1
                if mapq == 255:
                    unique += 1
                elif mapq > 0:
                    multiple += 1
            records[qname] = {
                "flag": flag,
                "rname": rname,
                "pos": pos,
                "mapq": mapq,
                "cigar": cigar,
            }

    top_rname, top_rname_count = ("NA", 0)
    if rnames:
        top_rname, top_rname_count = rnames.most_common(1)[0]

    return {
        "path": path,
        "total": total,
        "mapped": mapped,
        "unmapped": unmapped,
        "unique_mapq255": unique,
        "multiple_mapq_1_254": multiple,
        "top_rname": top_rname,
        "top_rname_count": top_rname_count,
        "top_rname_pct": round((top_rname_count / mapped * 100.0), 2) if mapped else 0.0,
        "flag_distribution": dict(flags.most_common(20)),
        "rname_distribution": dict(rnames.most_common(30)),
        "records": records,
    }


def compare(cpp, rust):
    cpp_records = cpp["records"]
    rust_records = rust["records"]
    cpp_names = set(cpp_records)
    rust_names = set(rust_records)
    common = cpp_names & rust_names
    exact = 0
    same_rname_pos = 0
    same_strand = 0
    for qname in common:
        a = cpp_records[qname]
        b = rust_records[qname]
        if a == b:
            exact += 1
        if a["rname"] == b["rname"] and a["pos"] == b["pos"]:
            same_rname_pos += 1
        if (a["flag"] & 0x10) == (b["flag"] & 0x10):
            same_strand += 1

    return {
        "common_qname": len(common),
        "cpp_only_qname": len(cpp_names - rust_names),
        "rust_only_qname": len(rust_names - cpp_names),
        "exact_record_match": exact,
        "same_rname_pos": same_rname_pos,
        "same_strand": same_strand,
        "exact_match_pct_of_common": round(exact / len(common) * 100.0, 2) if common else 0.0,
        "same_rname_pos_pct_of_common": round(same_rname_pos / len(common) * 100.0, 2) if common else 0.0,
    }


def strip_records(stats):
    stats = dict(stats)
    stats.pop("records", None)
    return stats


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpp", required=True)
    parser.add_argument("--rust", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    cpp = parse_sam(args.cpp)
    rust = parse_sam(args.rust)
    result = {
        "cpp": strip_records(cpp),
        "rust": strip_records(rust),
        "compare": compare(cpp, rust),
    }
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(result, fh, indent=2, ensure_ascii=False)


if __name__ == "__main__":
    main()
