#!/usr/bin/env python3
import json
import pathlib
import re
import sys


CASES = ("rust_se", "cpp_se", "rust_pe", "cpp_pe")


def read_text(path):
    return path.read_text(encoding="utf-8", errors="replace").strip()


def read_tsv(path, columns):
    rows = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        fields = line.split("\t", columns - 1)
        if len(fields) == columns:
            rows.append(fields)
    return rows


def parse_number(value, converter):
    try:
        return converter(value)
    except (TypeError, ValueError):
        return None


def parse_time(path):
    text = read_text(path) if path.is_file() else ""

    def field(label):
        match = re.search(rf"^\s*{re.escape(label)}:\s*(.+)$", text, re.MULTILINE)
        return match.group(1).strip() if match else None

    cpu = field("Percent of CPU this job got")
    return {
        "wall": field("Elapsed (wall clock) time (h:mm:ss or m:ss)"),
        "user_seconds": parse_number(field("User time (seconds)"), float),
        "system_seconds": parse_number(field("System time (seconds)"), float),
        "cpu_percent": parse_number(cpu.rstrip("%") if cpu else None, float),
        "max_rss_kb": parse_number(field("Maximum resident set size (kbytes)"), int),
    }


def parse_case(run_dir, case_name):
    case_dir = run_dir / case_name
    exit_code = read_text(case_dir / "exit_code.txt")
    return {
        "implementation": read_text(case_dir / "implementation.txt"),
        "layout": read_text(case_dir / "layout.txt"),
        "command": read_text(case_dir / "command.txt"),
        "exit_code": parse_number(exit_code, int),
        "time": parse_time(case_dir / "time.txt"),
        "sam": str(case_dir / "output.sam"),
        "stdout": str(case_dir / "stdout.txt"),
        "stderr": str(case_dir / "stderr.txt"),
    }


def parse_hashes(path):
    result = {}
    for category, name, digest, file_path in read_tsv(path, 4):
        result.setdefault(category, {})[name] = {
            "sha256": digest,
            "path": file_path,
        }
    return result


def parse_sam_stats(run_dir, layout):
    stats_dir = run_dir / "comparisons"
    exit_path = stats_dir / f"{layout}.exit_code.txt"
    stats_path = stats_dir / f"{layout}.json"
    exit_code = parse_number(read_text(exit_path), int) if exit_path.is_file() else None
    stats = None
    if stats_path.is_file():
        try:
            stats = json.loads(read_text(stats_path))
        except json.JSONDecodeError:
            stats = None
    return {
        "exit_code": exit_code,
        "result": stats,
        "stderr": str(stats_dir / f"{layout}.stderr.txt"),
    }


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: summarize_mm10_run.py RUN_DIR")

    run_dir = pathlib.Path(sys.argv[1]).resolve()
    metadata = dict(read_tsv(run_dir / "metadata.tsv", 2))
    benchmarks = {name: parse_case(run_dir, name) for name in CASES}
    failed_cases = [
        name for name, result in benchmarks.items() if result["exit_code"] != 0
    ]

    summary = {
        "schema_version": int(metadata["schema_version"]),
        "run": {
            "id": metadata["run_id"],
            "started_at_utc": metadata["started_at_utc"],
            "finished_at_utc": metadata["finished_at_utc"],
            "commit": metadata["commit"],
            "repo_dirty": metadata["repo_dirty"] == "true",
            "repo_root": metadata["repo_root"],
            "run_dir": metadata["run_dir"],
        },
        "fixed_inputs": {
            "reference": metadata["reference"],
            "read_1": metadata["read_1"],
            "read_2": metadata["read_2"],
            "rust_binary": metadata["rust_binary"],
            "cpp_binary": metadata["cpp_binary"],
            "common_parameters": metadata["common_parameters"],
        },
        "benchmarks": benchmarks,
        "sha256": parse_hashes(run_dir / "sha256.tsv"),
        "sam_stats": {
            "se": parse_sam_stats(run_dir, "se"),
            "pe": parse_sam_stats(run_dir, "pe"),
        },
        "failed_cases": failed_cases,
    }
    json.dump(summary, sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
