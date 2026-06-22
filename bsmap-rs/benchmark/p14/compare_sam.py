#!/usr/bin/env python3
"""Compare non-header SAM records in their original order."""

import argparse
import csv
import json
from collections import Counter
from pathlib import Path


TRACKED_FIELDS = ("RNAME", "POS", "FLAG", "NM", "ZP", "ZL")
HEADER_PREFIXES = ("@HD\t", "@SQ\t", "@RG\t", "@PG\t", "@CO\t")


def normalize_line_ending(line):
    if line.endswith("\r\n"):
        return line[:-2]
    if line.endswith("\n") or line.endswith("\r"):
        return line[:-1]
    return line


def read_records(path):
    records = []
    with Path(path).open("r", encoding="utf-8", errors="surrogateescape", newline="") as handle:
        for line in handle:
            normalized = normalize_line_ending(line)
            if normalized.startswith(HEADER_PREFIXES):
                continue
            records.append(normalized)
    return records


def optional_tag(fields, name):
    prefix = f"{name}:"
    for field in fields[11:]:
        if field.startswith(prefix):
            parts = field.split(":", 2)
            return ":".join(parts[1:]) if len(parts) == 3 else field
    return None


def extract_fields(record):
    fields = record.split("\t")
    return {
        "QNAME": fields[0] if fields else None,
        "FLAG": fields[1] if len(fields) > 1 else None,
        "RNAME": fields[2] if len(fields) > 2 else None,
        "POS": fields[3] if len(fields) > 3 else None,
        "NM": optional_tag(fields, "NM"),
        "ZP": optional_tag(fields, "ZP"),
        "ZL": optional_tag(fields, "ZL"),
    }


def display_value(value):
    return "<MISSING>" if value is None else value


def compare_records(expected_records, actual_records, field_diff_path):
    compared_count = min(len(expected_records), len(actual_records))
    exact_line_matches = 0
    field_difference_counts = Counter()
    field_difference_rows = 0

    field_diff_path = Path(field_diff_path)
    field_diff_path.parent.mkdir(parents=True, exist_ok=True)
    with field_diff_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(
            ("record_index", "expected_qname", "actual_qname", "field", "expected", "actual")
        )

        for index in range(compared_count):
            expected_record = expected_records[index]
            actual_record = actual_records[index]
            if expected_record == actual_record:
                exact_line_matches += 1
                continue

            expected_fields = extract_fields(expected_record)
            actual_fields = extract_fields(actual_record)
            for field in TRACKED_FIELDS:
                if expected_fields[field] == actual_fields[field]:
                    continue
                field_difference_counts[field] += 1
                field_difference_rows += 1
                writer.writerow(
                    (
                        index + 1,
                        display_value(expected_fields["QNAME"]),
                        display_value(actual_fields["QNAME"]),
                        field,
                        display_value(expected_fields[field]),
                        display_value(actual_fields[field]),
                    )
                )

    exact_line_mismatches = compared_count - exact_line_matches
    return {
        "expected_records": len(expected_records),
        "actual_records": len(actual_records),
        "compared_records": compared_count,
        "exact_line_matches": exact_line_matches,
        "exact_line_mismatches": exact_line_mismatches,
        "expected_only_records": len(expected_records) - compared_count,
        "actual_only_records": len(actual_records) - compared_count,
        "field_difference_rows": field_difference_rows,
        "field_difference_counts": {
            field: field_difference_counts[field] for field in TRACKED_FIELDS
        },
        "exact_match": (
            len(expected_records) == len(actual_records) and exact_line_mismatches == 0
        ),
    }


def compare_files(expected_path, actual_path, summary_path, field_diff_path):
    expected_records = read_records(expected_path)
    actual_records = read_records(actual_path)
    summary = compare_records(expected_records, actual_records, field_diff_path)
    summary = {
        "schema_version": 1,
        "expected_path": str(expected_path),
        "actual_path": str(actual_path),
        "field_diff_path": str(field_diff_path),
        **summary,
    }

    summary_path = Path(summary_path)
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    with summary_path.open("w", encoding="utf-8", newline="\n") as handle:
        json.dump(summary, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    return summary


def parse_args():
    parser = argparse.ArgumentParser(
        description="Compare non-header SAM records in original order."
    )
    parser.add_argument("expected", help="Expected SAM file")
    parser.add_argument("actual", help="Actual SAM file")
    parser.add_argument("--summary", required=True, help="JSON summary output path")
    parser.add_argument(
        "--field-diff", required=True, help="Tracked field differences TSV output path"
    )
    return parser.parse_args()


def main():
    args = parse_args()
    summary = compare_files(args.expected, args.actual, args.summary, args.field_diff)
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0 if summary["exact_match"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
