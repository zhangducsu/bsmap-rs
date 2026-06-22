#!/usr/bin/env python3
"""Parse GNU time -v output into stable JSON metrics."""

import argparse
import json
import re
from pathlib import Path


FIELD_SPECS = (
    ("User time (seconds)", "user_seconds", float),
    ("System time (seconds)", "system_seconds", float),
    ("Percent of CPU this job got", "cpu_percent", lambda value: float(value.rstrip("%"))),
    ("Maximum resident set size (kbytes)", "max_rss_kib", int),
    ("Major (requiring I/O) page faults", "major_page_faults", int),
    ("Minor (reclaiming a frame) page faults", "minor_page_faults", int),
    ("Voluntary context switches", "voluntary_context_switches", int),
    ("Involuntary context switches", "involuntary_context_switches", int),
    ("File system inputs", "file_system_inputs", int),
    ("File system outputs", "file_system_outputs", int),
    ("Exit status", "reported_exit_status", int),
)


def parse_elapsed(value):
    parts = value.split(":")
    if len(parts) == 2:
        minutes, seconds = parts
        return int(minutes) * 60.0 + float(seconds)
    if len(parts) == 3:
        hours, minutes, seconds = parts
        return int(hours) * 3600.0 + int(minutes) * 60.0 + float(seconds)
    return float(value)


def field_value(text, label):
    prefix = f"{label}:"
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(prefix):
            return stripped[len(prefix) :].strip()
    return None


def parse_gnu_time(text, outer_exit_code=None):
    metrics = {
        "schema_version": 1,
        "elapsed_seconds": None,
        "user_seconds": None,
        "system_seconds": None,
        "cpu_percent": None,
        "max_rss_kib": None,
        "major_page_faults": None,
        "minor_page_faults": None,
        "voluntary_context_switches": None,
        "involuntary_context_switches": None,
        "file_system_inputs": None,
        "file_system_outputs": None,
        "reported_exit_status": None,
        "outer_exit_code": outer_exit_code,
        "signal": None,
    }

    elapsed = field_value(text, "Elapsed (wall clock) time (h:mm:ss or m:ss)")
    if elapsed is not None:
        metrics["elapsed_seconds"] = parse_elapsed(elapsed)

    for label, key, converter in FIELD_SPECS:
        value = field_value(text, label)
        if value is not None:
            metrics[key] = converter(value)

    signal_match = re.search(
        r"^\s*Command terminated by signal (\d+)\s*$", text, re.MULTILINE
    )
    if signal_match:
        metrics["signal"] = int(signal_match.group(1))

    effective_exit = outer_exit_code
    if effective_exit is None:
        effective_exit = metrics["reported_exit_status"]
    metrics["effective_exit_code"] = effective_exit
    metrics["successful"] = metrics["signal"] is None and effective_exit == 0
    return metrics


def main():
    parser = argparse.ArgumentParser(description="解析 GNU /usr/bin/time -v 输出。")
    parser.add_argument("time_file")
    parser.add_argument("--output", required=True)
    parser.add_argument("--outer-exit-code", type=int)
    args = parser.parse_args()

    text = Path(args.time_file).read_text(encoding="utf-8", errors="replace")
    metrics = parse_gnu_time(text, args.outer_exit_code)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(metrics, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(metrics, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
