#!/usr/bin/env python3
"""Collect SSH1 case outputs into one summary JSON."""

import json
from pathlib import Path
import sys


def read_text(path):
    path = Path(path)
    return path.read_text(encoding="utf-8", errors="replace").strip() if path.exists() else ""


def read_json(path):
    path = Path(path)
    if not path.exists():
        return None
    return json.loads(path.read_text(encoding="utf-8", errors="replace"))


def read_tsv(path):
    result = {}
    path = Path(path)
    if not path.exists():
        return result
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        key, sep, value = line.partition("\t")
        if sep:
            result[key] = value
    return result


def parse_rrbs_profile(text):
    profile = {}
    for line in text.splitlines():
        if not line.startswith("BSMAP_PROFILE_RRBS "):
            continue
        item = line.removeprefix("BSMAP_PROFILE_RRBS ").strip()
        key, sep, value = item.partition("=")
        if not sep:
            continue
        try:
            profile[key] = int(value)
        except ValueError:
            try:
                profile[key] = float(value)
            except ValueError:
                profile[key] = value
    return profile


def parse_case(case_dir):
    case_dir = Path(case_dir)
    sha_text = read_text(case_dir / "output.sam.sha256")
    stderr_text = read_text(case_dir / "stderr.txt")
    return {
        "command": read_text(case_dir / "command.txt"),
        "exit_code": int(read_text(case_dir / "exit_code.txt") or "-1"),
        "time": read_json(case_dir / "time.json"),
        "sam_stats": read_json(case_dir / "sam_stats.json"),
        "sam_sha256": sha_text.split(" ")[0] if sha_text else "",
        "rrbs_profile": parse_rrbs_profile(stderr_text),
        "stderr_tail": "\n".join(stderr_text.splitlines()[-12:]),
    }


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: summarize_server_rrbs.py RUN_DIR")
    run_dir = Path(sys.argv[1]).resolve()
    metadata = read_tsv(run_dir / "metadata.tsv")
    cases = {}
    for case_dir in sorted(run_dir.glob("case_*")):
        cases[case_dir.name.removeprefix("case_")] = parse_case(case_dir)

    comparisons = {}
    comparison_dir = run_dir / "comparisons"
    if comparison_dir.exists():
        for path in sorted(comparison_dir.glob("*.json")):
            comparisons[path.stem] = read_json(path)

    summary = {
        "schema_version": 1,
        "metadata": metadata,
        "cases": cases,
        "comparisons": comparisons,
    }
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
