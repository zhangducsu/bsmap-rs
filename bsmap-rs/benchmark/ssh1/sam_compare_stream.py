#!/usr/bin/env python3
"""Streaming SAM field comparison for large RRBS outputs."""

import argparse
import csv
import json
from collections import Counter
from itertools import zip_longest
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
    with Path(path).open("r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line.strip() or line.startswith("@"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 11:
                continue
            yield {
                "QNAME": normalize_qname(fields[0]),
                "FLAG": fields[1],
                "RNAME": fields[2],
                "POS": fields[3],
                "NM": optional_tag(fields, "NM"),
                "ZP": optional_tag(fields, "ZP"),
                "ZL": optional_tag(fields, "ZL"),
            }


def display(value):
    return "<MISSING>" if value is None else value


def compare(expected_path, actual_path, diff_path, sample_limit):
    compared = expected_only = actual_only = exact_records = 0
    field_counts = Counter()
    sampled = 0

    with Path(diff_path).open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(("record_index", "field", "expected", "actual", "expected_qname", "actual_qname"))
        for index, (expected, actual) in enumerate(
            zip_longest(records(expected_path), records(actual_path)),
            start=1,
        ):
            if expected is None:
                actual_only += 1
                continue
            if actual is None:
                expected_only += 1
                continue

            compared += 1
            record_exact = True
            for field in TRACKED_FIELDS:
                if expected[field] == actual[field]:
                    continue
                field_counts[field] += 1
                record_exact = False
                if sampled < sample_limit:
                    writer.writerow(
                        (
                            index,
                            field,
                            display(expected[field]),
                            display(actual[field]),
                            display(expected["QNAME"]),
                            display(actual["QNAME"]),
                        )
                    )
                    sampled += 1
            if record_exact:
                exact_records += 1

    exact = (
        expected_only == 0
        and actual_only == 0
        and compared == exact_records
        and not field_counts
    )
    return {
        "schema_version": 1,
        "expected_path": str(expected_path),
        "actual_path": str(actual_path),
        "field_diff_path": str(diff_path),
        "stream_order_assumption": True,
        "compared_records": compared,
        "exact_records": exact_records,
        "mismatched_records": compared - exact_records,
        "expected_only_records": expected_only,
        "actual_only_records": actual_only,
        "field_difference_counts": {field: field_counts[field] for field in TRACKED_FIELDS},
        "sampled_field_differences": sampled,
        "exact_match": exact,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("expected")
    parser.add_argument("actual")
    parser.add_argument("--summary", required=True)
    parser.add_argument("--field-diff", required=True)
    parser.add_argument("--sample-limit", type=int, default=10000)
    args = parser.parse_args()

    result = compare(args.expected, args.actual, args.field_diff, args.sample_limit)
    Path(args.summary).write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if result["exact_match"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
