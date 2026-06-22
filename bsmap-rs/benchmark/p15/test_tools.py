#!/usr/bin/env python3

import json
import os
import struct
import tempfile
import threading
import unittest
from pathlib import Path

from index_sections import HEADER_SIZE, inspect_index
from metrics import parse_elapsed, parse_gnu_time
from slow_sink import consume
from stream_fastq import FastqError, iter_fastq, parse_size, stream_fastq
from summarize_scale import summarize


FASTQ_1 = b"@read1\nACGT\n+\nIIII\n"
FASTQ_2 = b"@read2\nTGCA\n+\nJJJJ\n"


class MetricsTest(unittest.TestCase):
    def test_parses_time_and_signal(self):
        text = """
        Command terminated by signal 6
        User time (seconds): 2.50
        System time (seconds): 0.25
        Percent of CPU this job got: 110%
        Elapsed (wall clock) time (h:mm:ss or m:ss): 0:02.75
        Maximum resident set size (kbytes): 1024
        Major (requiring I/O) page faults: 7
        Exit status: 0
        """
        result = parse_gnu_time(text, 134)
        self.assertEqual(result["elapsed_seconds"], 2.75)
        self.assertEqual(result["signal"], 6)
        self.assertEqual(result["effective_exit_code"], 134)
        self.assertFalse(result["successful"])

    def test_elapsed_formats(self):
        self.assertEqual(parse_elapsed("1:02.5"), 62.5)
        self.assertEqual(parse_elapsed("1:01:02"), 3662.0)


class IndexSectionsTest(unittest.TestCase):
    def make_index(
        self,
        root,
        *,
        version=8,
        mode=0,
        refcat_count=0,
        crefcat_count=1,
    ):
        header = bytearray(HEADER_SIZE)
        header[:8] = b"BSMAPIDX"
        struct.pack_into("<I", header, 8, version)
        struct.pack_into("<I", header, 12, 16)
        struct.pack_into("<I", header, 16, mode)
        struct.pack_into("<I", header, 20, 43_046_721)
        struct.pack_into("<I", header, 28, 4)
        struct.pack_into("<Q", header, 48, refcat_count)
        struct.pack_into("<Q", header, 56, crefcat_count)
        cursor = HEADER_SIZE
        sizes = (8, 4, 8, 4, 12, 4, 8, 8, 8)
        for index, size in enumerate(sizes):
            count = refcat_count if index == 7 else crefcat_count if index == 8 else 0
            struct.pack_into("<QQ", header, 100 + index * 16, cursor, count)
            cursor += count * size
        header[248:256] = b"RAWSECT2"
        path = root / "test.bsi"
        path.write_bytes(header + bytearray(cursor - HEADER_SIZE))
        return path

    def test_valid_v8_layout(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            report = inspect_index(self.make_index(Path(temp_dir)))
        self.assertTrue(report["valid"])
        self.assertEqual(report["mode"], "WGBS")
        self.assertEqual(report["sections"][-1]["byte_length"], 8)

    def test_detects_section_beyond_eof(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            path = self.make_index(Path(temp_dir), crefcat_count=2)
            path.write_bytes(path.read_bytes()[:-1])
            report = inspect_index(path)
        self.assertFalse(report["valid"])
        self.assertTrue(any("文件末尾" in error for error in report["errors"]))

    def test_valid_v9_rrbs_omits_crefcat(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            report = inspect_index(
                self.make_index(
                    Path(temp_dir),
                    version=9,
                    mode=1,
                    refcat_count=1,
                    crefcat_count=0,
                )
            )
        self.assertTrue(report["valid"])
        self.assertEqual(report["mode"], "RRBS")
        self.assertEqual(report["crefcat_words"], 0)
        self.assertEqual(report["sections"][-1]["byte_length"], 0)

    def test_rejects_v9_wgbs(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            report = inspect_index(
                self.make_index(Path(temp_dir), version=9, mode=0, crefcat_count=1)
            )
        self.assertFalse(report["valid"])
        self.assertTrue(any("WGBS raw index 必须为 v8" in error for error in report["errors"]))

    def test_rejects_v9_rrbs_with_materialized_crefcat(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            report = inspect_index(
                self.make_index(Path(temp_dir), version=9, mode=1, crefcat_count=1)
            )
        self.assertFalse(report["valid"])
        self.assertTrue(any("省略 materialized crefcat" in error for error in report["errors"]))


class StreamFastqTest(unittest.TestCase):
    def test_size_units(self):
        self.assertEqual(parse_size("90G"), 90_000_000_000)
        self.assertEqual(parse_size("1GiB"), 1_073_741_824)

    def test_repeats_se_with_constant_record_boundaries(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "reads.fq"
            output = root / "out.fq"
            summary_path = root / "summary.json"
            source.write_bytes(FASTQ_1 + FASTQ_2)
            summary = stream_fastq(source, output, summary_path, repeats=3)
            self.assertEqual(output.read_bytes(), (FASTQ_1 + FASTQ_2) * 3)
            self.assertEqual(summary["records_or_pairs"], 6)
            self.assertEqual(summary["cycles"], 3)
            self.assertEqual(json.loads(summary_path.read_text())["status"], "complete")

    def test_target_emitted_bytes_stops_after_complete_record(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "reads.fq"
            output = root / "out.fq"
            source.write_bytes(FASTQ_1)
            summary = stream_fastq(
                source,
                output,
                root / "summary.json",
                target_emitted_bytes=len(FASTQ_1) + 1,
            )
            self.assertEqual(summary["records_or_pairs"], 2)
            self.assertEqual(list(iter_fastq(output)), [FASTQ_1, FASTQ_1])

    def test_rejects_truncated_record(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "bad.fq"
            path.write_bytes(b"@read\nACGT\n+\n")
            with self.assertRaises(FastqError):
                list(iter_fastq(path))

    def test_paired_records_stay_synchronized(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            r1, r2 = root / "r1.fq", root / "r2.fq"
            o1, o2 = root / "o1.fq", root / "o2.fq"
            r1.write_bytes(FASTQ_1)
            r2.write_bytes(FASTQ_2)
            summary = stream_fastq(
                r1, o1, root / "summary.json", read_2=r2, output_2=o2, repeats=2
            )
            self.assertEqual(summary["records_or_pairs"], 2)
            self.assertEqual(o1.read_bytes(), FASTQ_1 * 2)
            self.assertEqual(o2.read_bytes(), FASTQ_2 * 2)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "requires POSIX FIFO")
    def test_paired_fifo_allows_mates_to_be_consumed_sequentially(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            r1, r2 = root / "r1.fq", root / "r2.fq"
            fifo1, fifo2 = root / "r1.pipe", root / "r2.pipe"
            r1.write_bytes(FASTQ_1)
            r2.write_bytes(FASTQ_2)
            os.mkfifo(fifo1)
            os.mkfifo(fifo2)
            consumed = {}

            def sequential_reader():
                with fifo1.open("rb", buffering=0) as first:
                    consumed["r1"] = first.read()
                with fifo2.open("rb", buffering=0) as second:
                    consumed["r2"] = second.read()

            reader = threading.Thread(target=sequential_reader)
            reader.start()
            stream_fastq(
                r1,
                fifo1,
                root / "summary.json",
                read_2=r2,
                output_2=fifo2,
                repeats=16,
            )
            reader.join(timeout=2)
            self.assertFalse(reader.is_alive())
            self.assertEqual(consumed["r1"], FASTQ_1 * 16)
            self.assertEqual(consumed["r2"], FASTQ_2 * 16)


class SinkAndSummaryTest(unittest.TestCase):
    def test_sink_and_combined_summary(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "sam"
            source.write_bytes(b"@HD\tVN:1.0\nread1\t4\t*\nread2\t0\tchr1")
            sink = consume(source, root / "sink.json")
            self.assertEqual(sink["line_count"], 3)
            self.assertEqual(sink["sam_header_lines"], 1)
            self.assertEqual(sink["sam_record_lines"], 2)
            timing = {
                "successful": True,
                "elapsed_seconds": 2.0,
                "max_rss_kib": 1_048_576,
            }
            result = summarize(
                {"threads": "8"},
                {"emitted_bytes": 2 * 1024 * 1024},
                sink,
                timing,
                {"align": 0, "producer": 0, "sink": 0},
            )
            self.assertTrue(result["successful"])
            self.assertEqual(result["input_throughput_mib_per_sec"], 1.0)
            self.assertEqual(result["max_rss_gib"], 1.0)


if __name__ == "__main__":
    unittest.main()
