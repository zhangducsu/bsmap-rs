#!/usr/bin/env python3
"""Summarize P17 short validation runs.

The script intentionally treats large-sample benefit as an estimate. It only
compares measured short-validation metrics already written by the runner.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


SCENARIOS = ("example1", "example2", "rrbs_se", "rrbs_pe")
METRIC_KEYS = ("rust_time", "cpp_time", "rust_index_time")
CONTROL_DRIFT_THRESHOLD_PCT = 10.0


def parse_elapsed(value: str | None) -> float | None:
    if not value:
        return None
    parts = value.split(":")
    try:
        if len(parts) == 2:
            minutes, seconds = parts
            return int(minutes) * 60 + float(seconds)
        if len(parts) == 3:
            hours, minutes, seconds = parts
            return int(hours) * 3600 + int(minutes) * 60 + float(seconds)
    except ValueError:
        return None
    return None


def parse_number(value: str | None) -> float | None:
    if value is None or value == "":
        return None
    try:
        return float(str(value).rstrip("%"))
    except ValueError:
        return None


def percent_change(old: float | None, new: float | None) -> float | None:
    if old is None or new is None or old == 0:
        return None
    return round((new - old) / old * 100.0, 4)


def load_json(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def extract_metrics(summary: dict[str, Any]) -> dict[str, Any]:
    extracted: dict[str, Any] = {}
    for scenario in SCENARIOS:
        entry = summary.get(scenario, {})
        scenario_metrics: dict[str, Any] = {}
        for key in METRIC_KEYS:
            stats = entry.get(key)
            if not isinstance(stats, dict):
                continue
            scenario_metrics[key] = {
                "wall_sec": parse_elapsed(stats.get("elapsed")),
                "user_sec": parse_number(stats.get("user_sec")),
                "sys_sec": parse_number(stats.get("sys_sec")),
                "cpu_pct": parse_number(stats.get("cpu_pct")),
                "max_rss_kib": parse_number(stats.get("max_rss_kib")),
            }
        if scenario_metrics:
            extracted[scenario] = scenario_metrics
    return extracted


def compare(
    baseline: dict[str, Any] | None, candidate: dict[str, Any]
) -> dict[str, Any]:
    candidate_metrics = extract_metrics(candidate)
    if baseline is None:
        return {
            "scale_tests_enabled": False,
            "large_sample_benefit": "estimated_only",
            "benchmark_stability": {
                "baseline_available": False,
                "control_drift_checked": False,
                "unstable": None,
            },
            "candidate": candidate_metrics,
        }

    baseline_metrics = extract_metrics(baseline)
    deltas: dict[str, Any] = {}
    for scenario in SCENARIOS:
        scenario_delta: dict[str, Any] = {}
        for key in METRIC_KEYS:
            old = baseline_metrics.get(scenario, {}).get(key, {})
            new = candidate_metrics.get(scenario, {}).get(key, {})
            if not old or not new:
                continue
            scenario_delta[key] = {
                "wall_pct": percent_change(old.get("wall_sec"), new.get("wall_sec")),
                "rss_pct": percent_change(old.get("max_rss_kib"), new.get("max_rss_kib")),
                "cpu_pct_points": (
                    None
                    if old.get("cpu_pct") is None or new.get("cpu_pct") is None
                    else round(new["cpu_pct"] - old["cpu_pct"], 4)
                ),
                "baseline": old,
                "candidate": new,
            }
        if scenario_delta:
            deltas[scenario] = scenario_delta

    return {
        "scale_tests_enabled": False,
        "large_sample_benefit": "estimated_only",
        "benchmark_stability": summarize_control_drift(deltas),
        "deltas_vs_baseline": deltas,
    }


def summarize_control_drift(deltas: dict[str, Any]) -> dict[str, Any]:
    controls: dict[str, Any] = {}
    max_abs_wall_pct = 0.0
    for scenario, scenario_delta in deltas.items():
        cpp_delta = scenario_delta.get("cpp_time")
        if not isinstance(cpp_delta, dict):
            continue
        wall_pct = cpp_delta.get("wall_pct")
        if wall_pct is None:
            continue
        abs_wall_pct = abs(wall_pct)
        max_abs_wall_pct = max(max_abs_wall_pct, abs_wall_pct)
        controls[scenario] = {
            "cpp_wall_pct": wall_pct,
            "abs_cpp_wall_pct": round(abs_wall_pct, 4),
        }

    return {
        "baseline_available": True,
        "control_drift_checked": bool(controls),
        "control_metric": "cpp_time.wall_pct",
        "threshold_pct": CONTROL_DRIFT_THRESHOLD_PCT,
        "max_abs_control_wall_pct": round(max_abs_wall_pct, 4),
        "unstable": max_abs_wall_pct > CONTROL_DRIFT_THRESHOLD_PCT,
        "controls": controls,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    result = compare(load_json(args.baseline), load_json(args.candidate))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
