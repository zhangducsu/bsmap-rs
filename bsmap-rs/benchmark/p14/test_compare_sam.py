#!/usr/bin/env python3

import json
import tempfile
import unittest
from pathlib import Path

from compare_sam import compare_files


def sam_record(qname, flag, rname, pos, mapq="255", tags=()):
    fields = [qname, flag, rname, pos, mapq, "75M", "*", "0", "0", "ACGT", "IIII"]
    return "\t".join([*fields, *tags])


class CompareSamTest(unittest.TestCase):
    def compare(self, expected_bytes, actual_bytes):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            expected = root / "expected.sam"
            actual = root / "actual.sam"
            summary_path = root / "summary.json"
            field_diff_path = root / "field_diff.tsv"
            expected.write_bytes(expected_bytes)
            actual.write_bytes(actual_bytes)
            summary = compare_files(expected, actual, summary_path, field_diff_path)
            persisted = json.loads(summary_path.read_text(encoding="utf-8"))
            field_diff = field_diff_path.read_text(encoding="utf-8")
            self.assertEqual(summary, persisted)
            return summary, field_diff

    def test_ignores_headers_and_normalizes_only_line_endings(self):
        record = sam_record("read1", "0", "chr1", "10", tags=("NM:i:0",))
        summary, field_diff = self.compare(
            f"@HD\tVN:1.6\r\n{record}\r\n".encode(),
            f"@PG\tID:other\n{record}\n".encode(),
        )
        self.assertTrue(summary["exact_match"])
        self.assertEqual(summary["exact_line_matches"], 1)
        self.assertEqual(len(field_diff.splitlines()), 1)

    def test_reports_all_tracked_field_differences(self):
        expected = sam_record(
            "left", "0", "chr1", "10", tags=("NM:i:0", "ZP:i:2", "ZL:Z:A")
        )
        actual = sam_record(
            "right", "16", "chr2", "20", tags=("NM:i:1", "ZP:i:3")
        )
        summary, field_diff = self.compare(
            f"{expected}\n".encode(), f"{actual}\n".encode()
        )
        self.assertFalse(summary["exact_match"])
        self.assertEqual(
            summary["field_difference_counts"],
            {"RNAME": 1, "POS": 1, "FLAG": 1, "NM": 1, "ZP": 1, "ZL": 1},
        )
        self.assertEqual(len(field_diff.splitlines()), 7)
        self.assertIn("\tZL\tZ:A\t<MISSING>\n", field_diff)

    def test_keeps_other_field_changes_as_exact_line_mismatches(self):
        expected = sam_record("read1", "0", "chr1", "10", mapq="255")
        actual = sam_record("read1", "0", "chr1", "10", mapq="42")
        summary, field_diff = self.compare(
            f"{expected}\n".encode(), f"{actual}\n".encode()
        )
        self.assertEqual(summary["exact_line_mismatches"], 1)
        self.assertEqual(summary["field_difference_rows"], 0)
        self.assertEqual(len(field_diff.splitlines()), 1)

    def test_record_order_and_count_are_significant(self):
        first = sam_record("first", "0", "chr1", "10")
        second = sam_record("second", "0", "chr1", "20")
        summary, _ = self.compare(
            f"{first}\n{second}\n".encode(), f"{second}\n".encode()
        )
        self.assertEqual(summary["compared_records"], 1)
        self.assertEqual(summary["exact_line_mismatches"], 1)
        self.assertEqual(summary["expected_only_records"], 1)
        self.assertFalse(summary["exact_match"])


if __name__ == "__main__":
    unittest.main()
