#!/usr/bin/env python3
"""Repeat FASTQ records into files or FIFOs without materializing a large input."""

import argparse
import gzip
import json
import math
import threading
import time
from pathlib import Path


DECIMAL_UNITS = {"K": 10**3, "M": 10**6, "G": 10**9, "T": 10**12}
BINARY_UNITS = {"KI": 2**10, "MI": 2**20, "GI": 2**30, "TI": 2**40}


class FastqError(ValueError):
    pass


def parse_size(value):
    normalized = value.strip().upper()
    if normalized.endswith("B"):
        normalized = normalized[:-1]
    for suffix, multiplier in (*BINARY_UNITS.items(), *DECIMAL_UNITS.items()):
        if normalized.endswith(suffix):
            number = normalized[: -len(suffix)]
            return int(float(number) * multiplier)
    return int(normalized)


def open_fastq(path):
    path = Path(path)
    if path.name.lower().endswith(".gz"):
        return gzip.open(path, "rb")
    return path.open("rb")


def iter_fastq(path):
    with open_fastq(path) as handle:
        record_index = 0
        while True:
            lines = [handle.readline() for _ in range(4)]
            if not lines[0]:
                if any(lines[1:]):
                    raise FastqError(f"{path}: 文件末尾存在不完整 FASTQ 记录")
                return
            record_index += 1
            if any(not line for line in lines[1:]):
                raise FastqError(f"{path}: 第 {record_index} 条记录不完整")
            if not lines[0].startswith(b"@"):
                raise FastqError(f"{path}: 第 {record_index} 条记录缺少 @ header")
            if not lines[2].startswith(b"+"):
                raise FastqError(f"{path}: 第 {record_index} 条记录缺少 + 分隔行")
            sequence = lines[1].rstrip(b"\r\n")
            quality = lines[3].rstrip(b"\r\n")
            if len(sequence) != len(quality):
                raise FastqError(f"{path}: 第 {record_index} 条记录序列与质量长度不一致")
            yield b"".join(lines)


def write_summary(path, summary):
    target = Path(path)
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def stream_one_input(input_path, output_path, repeat_limit, result, errors):
    try:
        emitted_bytes = 0
        records = 0
        cycles = 0
        with Path(output_path).open("wb", buffering=1024 * 1024) as output:
            for _ in range(repeat_limit):
                cycle_records = 0
                for record in iter_fastq(input_path):
                    output.write(record)
                    emitted_bytes += len(record)
                    records += 1
                    cycle_records += 1
                if cycle_records == 0:
                    raise FastqError(f"{input_path}: 没有完整 FASTQ 记录")
                cycles += 1
        result.update(bytes=emitted_bytes, records=records, cycles=cycles)
    except BaseException as error:
        errors.append(error)


def stream_fastq(read_1, output_1, summary_path, read_2=None, output_2=None,
                 repeats=None, target_source_bytes=None, target_emitted_bytes=None):
    source_cycle_bytes = Path(read_1).stat().st_size
    if read_2 is not None:
        source_cycle_bytes += Path(read_2).stat().st_size
    if source_cycle_bytes == 0:
        raise FastqError("输入 FASTQ 为空")

    if target_source_bytes is not None:
        repeat_limit = max(1, math.ceil(target_source_bytes / source_cycle_bytes))
    else:
        repeat_limit = repeats

    summary = {
        "schema_version": 1,
        "status": "running",
        "read_1": str(read_1),
        "read_2": str(read_2) if read_2 is not None else None,
        "source_cycle_bytes": source_cycle_bytes,
        "requested_repeats": repeats,
        "target_source_bytes": target_source_bytes,
        "target_emitted_bytes": target_emitted_bytes,
        "cycles": 0,
        "records_or_pairs": 0,
        "read_1_emitted_bytes": 0,
        "read_2_emitted_bytes": 0,
        "emitted_bytes": 0,
    }
    started = time.monotonic()

    try:
        if read_2 is not None:
            if target_emitted_bytes is not None:
                raise FastqError("PE 流式输入不支持 --target-emitted-bytes；请使用源字节或重复次数")
            results = ({}, {})
            errors = []
            threads = [
                threading.Thread(
                    target=stream_one_input,
                    args=(input_path, output_path, repeat_limit, result, errors),
                    daemon=True,
                )
                for input_path, output_path, result in zip(
                    (read_1, read_2), (output_1, output_2), results
                )
            ]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join()
            if errors:
                raise errors[0]
            if results[0]["records"] != results[1]["records"]:
                raise FastqError(
                    f"PE 输入记录数不一致：R1={results[0]['records']}，R2={results[1]['records']}"
                )
            summary["cycles"] = results[0]["cycles"]
            summary["records_or_pairs"] = results[0]["records"]
            summary["read_1_emitted_bytes"] = results[0]["bytes"]
            summary["read_2_emitted_bytes"] = results[1]["bytes"]
            summary["emitted_bytes"] = results[0]["bytes"] + results[1]["bytes"]
        else:
            with Path(output_1).open("wb", buffering=1024 * 1024) as out_1:
                while repeat_limit is None or summary["cycles"] < repeat_limit:
                    cycle_records = 0
                    for record_1 in iter_fastq(read_1):
                        out_1.write(record_1)
                        summary["read_1_emitted_bytes"] += len(record_1)
                        summary["records_or_pairs"] += 1
                        cycle_records += 1
                        summary["emitted_bytes"] = summary["read_1_emitted_bytes"]
                        if (
                            target_emitted_bytes is not None
                            and summary["emitted_bytes"] >= target_emitted_bytes
                        ):
                            break
                    if cycle_records == 0:
                        raise FastqError("输入 FASTQ 没有完整记录")
                    summary["cycles"] += 1
                    if (
                        target_emitted_bytes is not None
                        and summary["emitted_bytes"] >= target_emitted_bytes
                    ):
                        break
        summary["status"] = "complete"
        return summary
    except BaseException as error:
        summary["status"] = "error"
        summary["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        summary["elapsed_seconds"] = time.monotonic() - started
        write_summary(summary_path, summary)


def main():
    parser = argparse.ArgumentParser(description="以常数内存重复输出 FASTQ 到文件或 FIFO。")
    parser.add_argument("--input-r1", required=True)
    parser.add_argument("--input-r2")
    parser.add_argument("--output-r1", required=True)
    parser.add_argument("--output-r2")
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument("--repeats", type=int)
    target.add_argument("--target-source-bytes", type=parse_size)
    target.add_argument("--target-emitted-bytes", type=parse_size)
    parser.add_argument("--summary", required=True)
    args = parser.parse_args()

    if (args.input_r2 is None) != (args.output_r2 is None):
        parser.error("--input-r2 与 --output-r2 必须同时提供")
    if args.repeats is not None and args.repeats < 1:
        parser.error("--repeats 必须大于 0")

    stream_fastq(
        args.input_r1,
        args.output_r1,
        args.summary,
        read_2=args.input_r2,
        output_2=args.output_r2,
        repeats=args.repeats,
        target_source_bytes=args.target_source_bytes,
        target_emitted_bytes=args.target_emitted_bytes,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
