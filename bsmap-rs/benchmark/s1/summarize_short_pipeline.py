#!/usr/bin/env python3
import json
import re
import sys
from pathlib import Path


def parse_elapsed(value):
    parts = value.split(":")
    if len(parts) == 2:
        minutes, seconds = parts
        return int(minutes) * 60.0 + float(seconds)
    if len(parts) == 3:
        hours, minutes, seconds = parts
        return int(hours) * 3600.0 + int(minutes) * 60.0 + float(seconds)
    return float(value)


def field(text, label):
    prefix = f"{label}:"
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(prefix):
            return stripped[len(prefix) :].strip()
    return None


def parse_time(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    elapsed = field(text, "Elapsed (wall clock) time (h:mm:ss or m:ss)")
    cpu = field(text, "Percent of CPU this job got")
    rss = field(text, "Maximum resident set size (kbytes)")
    user = field(text, "User time (seconds)")
    sys_time = field(text, "System time (seconds)")
    signal = re.search(r"Command terminated by signal (\d+)", text)
    return {
        "elapsed_seconds": parse_elapsed(elapsed) if elapsed else None,
        "user_seconds": float(user) if user else None,
        "system_seconds": float(sys_time) if sys_time else None,
        "cpu_percent": float(cpu.rstrip("%")) if cpu else None,
        "max_rss_kib": int(rss) if rss else None,
        "signal": int(signal.group(1)) if signal else None,
    }


def read_exit(path):
    return int(path.read_text().strip())


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def collect_align(root):
    result = {}
    for run_dir in sorted((root / "align").iterdir()):
        if not run_dir.is_dir():
            continue
        result[run_dir.name] = {
            "exit_code": read_exit(run_dir / "exit.txt"),
            "time": parse_time(run_dir / "time.txt"),
            "sam": read_json(run_dir / "sam_summary.json"),
        }
    return result


def collect_index(root):
    result = {}
    for time_file in sorted((root / "index").glob("*.time")):
        name = time_file.stem
        result[name] = {
            "exit_code": read_exit(root / "index" / f"{name}.exit"),
            "time": parse_time(time_file),
        }
    return result


def compare_sha(align):
    groups = {
        "wgbs_se": ["baseline_wgbs_se", "current_wgbs_se_d1", "current_wgbs_se_d2"],
        "wgbs_pe": ["baseline_wgbs_pe", "current_wgbs_pe_d1", "current_wgbs_pe_d2"],
        "rrbs_se": ["baseline_rrbs_se", "current_rrbs_se_d1", "current_rrbs_se_d2"],
        "rrbs_pe": ["baseline_rrbs_pe", "current_rrbs_pe_d1", "current_rrbs_pe_d2"],
    }
    checks = {}
    for group, labels in groups.items():
        shas = {label: align[label]["sam"]["sam_sha256"] for label in labels if label in align}
        checks[group] = {
            "sha_by_label": shas,
            "all_equal": len(set(shas.values())) == 1 if shas else False,
        }
    return checks


def main():
    root = Path(sys.argv[1])
    align = collect_align(root)
    output = {
        "run_root": str(root),
        "metadata": (root / "metadata.tsv").read_text(encoding="utf-8", errors="replace"),
        "input_sha256": (root / "input_sha256.txt").read_text(encoding="utf-8", errors="replace"),
        "index": collect_index(root),
        "align": align,
        "sam_sha_checks": compare_sha(align),
    }
    print(json.dumps(output, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
