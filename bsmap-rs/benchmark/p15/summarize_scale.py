#!/usr/bin/env python3
"""Combine one P15 FIFO scale run into a machine-readable summary."""

import argparse
import json
from pathlib import Path


def read_json(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def read_metadata(path):
    result = {}
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if not line:
            continue
        key, value = line.split("\t", 1)
        result[key] = value
    return result


def summarize(metadata, producer, sink, timing, exit_codes):
    elapsed = timing.get("elapsed_seconds")
    emitted = producer.get("emitted_bytes", 0)
    return {
        "schema_version": 1,
        "metadata": metadata,
        "exit_codes": exit_codes,
        "successful": all(code == 0 for code in exit_codes.values()) and timing["successful"],
        "producer": producer,
        "sink": sink,
        "time": timing,
        "input_throughput_mib_per_sec": (
            emitted / (1024 * 1024) / elapsed if elapsed and emitted else None
        ),
        "max_rss_gib": (
            timing["max_rss_kib"] / 1024 / 1024
            if timing.get("max_rss_kib") is not None
            else None
        ),
    }


def main():
    parser = argparse.ArgumentParser(description="汇总 P15 FIFO scale run。")
    parser.add_argument("--metadata", required=True)
    parser.add_argument("--producer", required=True)
    parser.add_argument("--sink", required=True)
    parser.add_argument("--time", required=True)
    parser.add_argument("--align-exit", type=int, required=True)
    parser.add_argument("--producer-exit", type=int, required=True)
    parser.add_argument("--sink-exit", type=int, required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    result = summarize(
        read_metadata(args.metadata),
        read_json(args.producer),
        read_json(args.sink),
        read_json(args.time),
        {
            "align": args.align_exit,
            "producer": args.producer_exit,
            "sink": args.sink_exit,
        },
    )
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if result["successful"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
