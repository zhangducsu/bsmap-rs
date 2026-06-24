#!/usr/bin/env python3
"""Memory-light SAM statistics for SSH1 RRBS benchmarks."""

import argparse
import json
from collections import Counter
from pathlib import Path


ASCII_WHITESPACE = " \t\n\r\v\f"


def optional_tag(fields, name):
    prefix = f"{name}:"
    for field in fields[11:]:
        if field.startswith(prefix):
            parts = field.split(":", 2)
            return parts[2] if len(parts) == 3 else field
    return None


def parse_sam(path):
    total = mapped = unmapped = unique = multiple = 0
    flags = Counter()
    rnames = Counter()
    nm_values = Counter()
    zp_values = Counter()
    zl_values = Counter()

    with Path(path).open("r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line.strip() or line.startswith("@"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 11:
                continue
            flag = int(fields[1])
            rname = fields[2]
            mapq = int(fields[4])
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
            for name, counter in (("NM", nm_values), ("ZP", zp_values), ("ZL", zl_values)):
                value = optional_tag(fields, name)
                if value is not None:
                    counter[value] += 1

    top_rname, top_rname_count = ("NA", 0)
    if rnames:
        top_rname, top_rname_count = rnames.most_common(1)[0]

    return {
        "schema_version": 1,
        "path": str(path),
        "total": total,
        "mapped": mapped,
        "unmapped": unmapped,
        "unique_mapq255": unique,
        "multiple_mapq_1_254": multiple,
        "top_rname": top_rname,
        "top_rname_count": top_rname_count,
        "top_rname_pct_of_mapped": round(top_rname_count / mapped * 100.0, 4) if mapped else 0.0,
        "flag_distribution": dict(flags.most_common()),
        "rname_distribution_top30": dict(rnames.most_common(30)),
        "nm_distribution_top30": dict(nm_values.most_common(30)),
        "zp_distribution_top30": dict(zp_values.most_common(30)),
        "zl_distribution_top30": dict(zl_values.most_common(30)),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("sam")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    stats = parse_sam(args.sam)
    Path(args.output).write_text(
        json.dumps(stats, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(stats, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
