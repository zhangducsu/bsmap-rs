#!/usr/bin/env python3
"""Order-independent SAM comparison for RRBS subset benchmarks."""

import argparse
from collections import Counter
import json
from pathlib import Path


TRACKED_FIELDS = ("QNAME", "RNAME", "POS", "FLAG", "NM", "ZP", "ZL")
ASCII_WHITESPACE = " \t\n\r\v\f"


def normalize_qname(qname):
    token_end = next(
        (index for index, char in enumerate(qname) if char in ASCII_WHITESPACE),
        len(qname),
    )
    token = qname[:token_end]
    if token.endswith("/1") or token.endswith("/2"):
        token = token[:-2]
    return token


def optional_tag(fields, name):
    prefix = f"{name}:"
    for field in fields[11:]:
        if field.startswith(prefix):
            parts = field.split(":", 2)
            return parts[2] if len(parts) == 3 else field
    return None


def records(path):
    result = []
    with Path(path).open("r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line.strip() or line.startswith("@"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 11:
                continue
            result.append(
                (
                    normalize_qname(fields[0]),
                    fields[2],
                    fields[3],
                    fields[1],
                    optional_tag(fields, "NM"),
                    optional_tag(fields, "ZP"),
                    optional_tag(fields, "ZL"),
                )
            )
    return result


def sample(counter, limit):
    return [list(record) + [count] for record, count in list(counter.items())[:limit]]


def compare(expected_path, actual_path, sample_limit):
    expected_records = records(expected_path)
    actual_records = records(actual_path)
    expected_counter = Counter(expected_records)
    actual_counter = Counter(actual_records)
    expected_only = expected_counter - actual_counter
    actual_only = actual_counter - expected_counter
    expected_qnames = Counter(record[0] for record in expected_records)
    actual_qnames = Counter(record[0] for record in actual_records)
    expected_only_qnames = expected_qnames - actual_qnames
    actual_only_qnames = actual_qnames - expected_qnames
    expected_only_count = sum(expected_only.values())
    actual_only_count = sum(actual_only.values())
    compared = min(len(expected_records), len(actual_records))

    return {
        "schema_version": 1,
        "comparison": "sorted_multiset",
        "fields": TRACKED_FIELDS,
        "expected_path": str(expected_path),
        "actual_path": str(actual_path),
        "expected_records": len(expected_records),
        "actual_records": len(actual_records),
        "compared_records": compared,
        "exact_multiset_records": len(expected_records) - expected_only_count,
        "expected_only_records": expected_only_count,
        "actual_only_records": actual_only_count,
        "expected_only_qname_count": sum(expected_only_qnames.values()),
        "actual_only_qname_count": sum(actual_only_qnames.values()),
        "exact_match": expected_only_count == 0 and actual_only_count == 0,
        "sample_expected_only": sample(expected_only, sample_limit),
        "sample_actual_only": sample(actual_only, sample_limit),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("expected")
    parser.add_argument("actual")
    parser.add_argument("--summary", required=True)
    parser.add_argument("--sample-limit", type=int, default=20)
    args = parser.parse_args()

    summary = compare(Path(args.expected), Path(args.actual), args.sample_limit)
    Path(args.summary).write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    raise SystemExit(0 if summary["exact_match"] else 1)


if __name__ == "__main__":
    main()
