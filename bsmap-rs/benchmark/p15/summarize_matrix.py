#!/usr/bin/env python3
"""Aggregate successful P15 scale runs into a thread-scaling matrix."""

import argparse
import json
import statistics
from collections import defaultdict
from pathlib import Path


def load_runs(root):
    runs = []
    for path in sorted(Path(root).rglob("summary.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        data["summary_path"] = str(path)
        runs.append(data)
    if not runs:
        raise ValueError(f"没有找到 summary.json：{root}")
    return runs


def summarize_matrix(runs):
    failed = [run["summary_path"] for run in runs if not run.get("successful")]
    if failed:
        raise ValueError(f"存在失败 run：{failed}")

    hashes = {run["sink"]["sha256"] for run in runs}
    records = {run["sink"]["sam_record_lines"] for run in runs}
    if len(hashes) != 1 or len(records) != 1:
        raise ValueError("线程矩阵的 SAM SHA256 或记录数不一致")

    grouped = defaultdict(list)
    for run in runs:
        grouped[int(run["metadata"]["threads"])].append(run)
    if 1 not in grouped:
        raise ValueError("线程矩阵缺少 p1 基线")

    p1_wall = statistics.median(run["time"]["elapsed_seconds"] for run in grouped[1])
    rows = []
    for threads in sorted(grouped):
        group = grouped[threads]
        walls = [run["time"]["elapsed_seconds"] for run in group]
        rows.append(
            {
                "threads": threads,
                "runs": len(group),
                "median_wall_seconds": statistics.median(walls),
                "min_wall_seconds": min(walls),
                "max_wall_seconds": max(walls),
                "median_user_seconds": statistics.median(
                    run["time"]["user_seconds"] for run in group
                ),
                "median_system_seconds": statistics.median(
                    run["time"]["system_seconds"] for run in group
                ),
                "median_cpu_percent": statistics.median(
                    run["time"]["cpu_percent"] for run in group
                ),
                "worst_rss_kib": max(run["time"]["max_rss_kib"] for run in group),
                "worst_major_page_faults": max(
                    run["time"]["major_page_faults"] for run in group
                ),
                "speedup_vs_p1": p1_wall / statistics.median(walls),
            }
        )

    return {
        "schema_version": 1,
        "run_count": len(runs),
        "sam_sha256": next(iter(hashes)),
        "sam_record_lines": next(iter(records)),
        "rows": rows,
        "summary_paths": [run["summary_path"] for run in runs],
    }


def main():
    parser = argparse.ArgumentParser(description="汇总 P15 线程扩展矩阵。")
    parser.add_argument("runs_root")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    result = summarize_matrix(load_runs(args.runs_root))
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))


if __name__ == "__main__":
    main()
