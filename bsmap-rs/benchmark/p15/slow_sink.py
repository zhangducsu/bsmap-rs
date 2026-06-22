#!/usr/bin/env python3
"""Consume a file or FIFO with optional throttling and constant memory."""

import argparse
import hashlib
import json
import time
from pathlib import Path


def consume(path, summary_path, rate_mib_per_sec=0.0, chunk_size=1024 * 1024):
    digest = hashlib.sha256()
    total = 0
    newline_count = 0
    header_lines = 0
    header_complete = False
    header_buffer = b""
    last_byte = None
    started = time.monotonic()
    with Path(path).open("rb", buffering=chunk_size) as handle:
        while True:
            chunk = handle.read(chunk_size)
            if not chunk:
                break
            digest.update(chunk)
            total += len(chunk)
            newline_count += chunk.count(b"\n")
            last_byte = chunk[-1]
            if not header_complete:
                header_buffer += chunk
                while b"\n" in header_buffer:
                    line, header_buffer = header_buffer.split(b"\n", 1)
                    if line.rstrip(b"\r").startswith(b"@"):
                        header_lines += 1
                    else:
                        header_complete = True
                        header_buffer = b""
                        break
            if rate_mib_per_sec > 0:
                expected = total / (rate_mib_per_sec * 1024 * 1024)
                remaining = expected - (time.monotonic() - started)
                if remaining > 0:
                    time.sleep(remaining)

    elapsed = time.monotonic() - started
    line_count = newline_count + (1 if total and last_byte != ord("\n") else 0)
    summary = {
        "schema_version": 1,
        "status": "complete",
        "input": str(path),
        "configured_rate_mib_per_sec": rate_mib_per_sec,
        "bytes": total,
        "line_count": line_count,
        "sam_header_lines": header_lines,
        "sam_record_lines": max(0, line_count - header_lines),
        "sha256": digest.hexdigest(),
        "elapsed_seconds": elapsed,
        "observed_mib_per_sec": total / (1024 * 1024) / elapsed if elapsed else None,
    }
    output = Path(summary_path)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return summary


def main():
    parser = argparse.ArgumentParser(description="消费输出 FIFO，并可模拟有限写出带宽。")
    parser.add_argument("input")
    parser.add_argument("--summary", required=True)
    parser.add_argument("--rate-mib-per-sec", type=float, default=0.0)
    args = parser.parse_args()
    if args.rate_mib_per_sec < 0:
        parser.error("--rate-mib-per-sec 不能为负数")
    summary = consume(args.input, args.summary, args.rate_mib_per_sec)
    print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
