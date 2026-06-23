#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
  printf 'Usage: run_short_validation.sh REPO_ROOT RUNS_ROOT\n' >&2
  exit 2
fi

readonly REPO_ROOT="$(cd "$1" && pwd)"
readonly RUNS_ROOT="$2"
readonly BENCH="$REPO_ROOT/bsmap-rs/benchmark"
readonly RUST_BINARY="${RUST_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly CPP_BINARY="${CPP_BINARY:-$REPO_ROOT/bsmap-original/bsmap-2.90/bsmap}"
readonly WGBS_REF="$BENCH/data/chr22_tail_1M.fa"
readonly MM10_DATA="${MM10_DATA:-/mnt/d/BSMAP/benchmark-data/mm10}"
readonly MM10_REF="$MM10_DATA/mm10.fa"
readonly MM10_R1="$MM10_DATA/Ctrl_10K_R1.fq"
readonly MM10_R2="$MM10_DATA/Ctrl_10K_R2.fq"
readonly THREADS="${THREADS:-8}"
readonly RANDOM_SEED="${RANDOM_SEED:-1}"

mkdir -p "$RUNS_ROOT"/{tmp,example1,example2,rrbs_index,rrbs_se,rrbs_pe}

sha256sum "$RUST_BINARY" "$CPP_BINARY" "$WGBS_REF" "$MM10_REF" "$MM10_R1" "$MM10_R2" \
  > "$RUNS_ROOT/input_sha256.txt"

cat > "$RUNS_ROOT/metadata.txt" <<META
repo_root=$REPO_ROOT
rust_binary=$RUST_BINARY
cpp_binary=$CPP_BINARY
wgbs_reference=$WGBS_REF
mm10_reference=$MM10_REF
mm10_read_1=$MM10_R1
mm10_read_2=$MM10_R2
threads=$THREADS
random_seed=$RANDOM_SEED
example1_params=-s 16 -v 0.08 -I 4 -p 1 -S $RANDOM_SEED
example2_params=-s 16 -v 0.08 -I 4 -p 1 -S $RANDOM_SEED
rrbs_params=-s 12 -v 0.08 -I 4 -D C-CGG -p $THREADS -S $RANDOM_SEED
standalone_index_included=false
META

gzip -dc "$BENCH/data/wgbs/ex1_se75_10x/simulated.fastq.gz" > "$RUNS_ROOT/tmp/ex1.fastq"
gzip -dc "$BENCH/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" > "$RUNS_ROOT/tmp/ex2_1.fastq"
gzip -dc "$BENCH/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" > "$RUNS_ROOT/tmp/ex2_2.fastq"
ln -sf "$MM10_REF" "$RUNS_ROOT/tmp/mm10.fa"

run_timed() {
  local time_file="$1"
  local stdout_file="$2"
  local stderr_file="$3"
  local exit_file="$4"
  shift 4
  set +e
  /usr/bin/time -v -o "$time_file" "$@" > "$stdout_file" 2> "$stderr_file"
  local status=$?
  set -e
  printf '%s\n' "$status" > "$exit_file"
  return 0
}

rm -f "$WGBS_REF.bsi"
run_timed "$RUNS_ROOT/example1/cpp.time" "$RUNS_ROOT/example1/cpp.stdout" "$RUNS_ROOT/example1/cpp.stderr" "$RUNS_ROOT/example1/cpp.exit" \
  "$CPP_BINARY" -a "$RUNS_ROOT/tmp/ex1.fastq" -d "$WGBS_REF" -o "$RUNS_ROOT/example1/cpp.sam" \
  -s 16 -v 0.08 -I 4 -p 1 -S "$RANDOM_SEED"

rm -f "$WGBS_REF.bsi"
run_timed "$RUNS_ROOT/example1/rust_index.time" "$RUNS_ROOT/example1/rust_index.stdout" "$RUNS_ROOT/example1/rust_index.stderr" "$RUNS_ROOT/example1/rust_index.exit" \
  "$RUST_BINARY" index -d "$WGBS_REF" -s 16 -I 4

run_timed "$RUNS_ROOT/example1/rust.time" "$RUNS_ROOT/example1/rust.stdout" "$RUNS_ROOT/example1/rust.stderr" "$RUNS_ROOT/example1/rust.exit" \
  "$RUST_BINARY" align -a "$RUNS_ROOT/tmp/ex1.fastq" -d "$WGBS_REF" -o "$RUNS_ROOT/example1/rust.sam" \
  -s 16 -v 0.08 -I 4 -p 1 -S "$RANDOM_SEED"

python3 "$BENCH/p14/compare_sam.py" \
  "$RUNS_ROOT/example1/cpp.sam" "$RUNS_ROOT/example1/rust.sam" \
  --summary "$RUNS_ROOT/example1/compare_sam.json" \
  --field-diff "$RUNS_ROOT/example1/field_diff.tsv" || true

rm -f "$WGBS_REF.bsi"
run_timed "$RUNS_ROOT/example2/cpp.time" "$RUNS_ROOT/example2/cpp.stdout" "$RUNS_ROOT/example2/cpp.stderr" "$RUNS_ROOT/example2/cpp.exit" \
  "$CPP_BINARY" -a "$RUNS_ROOT/tmp/ex2_1.fastq" -b "$RUNS_ROOT/tmp/ex2_2.fastq" -d "$WGBS_REF" -o "$RUNS_ROOT/example2/cpp.sam" \
  -s 16 -v 0.08 -I 4 -p 1 -S "$RANDOM_SEED"

if [[ ! -f "$WGBS_REF.bsi" ]]; then
  run_timed "$RUNS_ROOT/example2/rust_index.time" "$RUNS_ROOT/example2/rust_index.stdout" "$RUNS_ROOT/example2/rust_index.stderr" "$RUNS_ROOT/example2/rust_index.exit" \
    "$RUST_BINARY" index -d "$WGBS_REF" -s 16 -I 4
fi

run_timed "$RUNS_ROOT/example2/rust.time" "$RUNS_ROOT/example2/rust.stdout" "$RUNS_ROOT/example2/rust.stderr" "$RUNS_ROOT/example2/rust.exit" \
  "$RUST_BINARY" align -a "$RUNS_ROOT/tmp/ex2_1.fastq" -b "$RUNS_ROOT/tmp/ex2_2.fastq" -d "$WGBS_REF" -o "$RUNS_ROOT/example2/rust.sam" \
  -s 16 -v 0.08 -I 4 -p 1 -S "$RANDOM_SEED"

python3 "$BENCH/p13/sam_stats.py" \
  --cpp "$RUNS_ROOT/example2/cpp.sam" \
  --rust "$RUNS_ROOT/example2/rust.sam" \
  --out "$RUNS_ROOT/example2/sam_stats.json"

rm -f "$RUNS_ROOT/tmp/mm10.fa.bsi"
run_timed "$RUNS_ROOT/rrbs_index/rust.time" "$RUNS_ROOT/rrbs_index/rust.stdout" "$RUNS_ROOT/rrbs_index/rust.stderr" "$RUNS_ROOT/rrbs_index/rust.exit" \
  "$RUST_BINARY" index -d "$RUNS_ROOT/tmp/mm10.fa" -s 12 -I 4 -D C-CGG

run_timed "$RUNS_ROOT/rrbs_se/rust.time" "$RUNS_ROOT/rrbs_se/rust.stdout" "$RUNS_ROOT/rrbs_se/rust.stderr" "$RUNS_ROOT/rrbs_se/rust.exit" \
  "$RUST_BINARY" align -a "$MM10_R1" -d "$RUNS_ROOT/tmp/mm10.fa" -o "$RUNS_ROOT/rrbs_se/rust.sam" \
  -s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED"

run_timed "$RUNS_ROOT/rrbs_se/cpp.time" "$RUNS_ROOT/rrbs_se/cpp.stdout" "$RUNS_ROOT/rrbs_se/cpp.stderr" "$RUNS_ROOT/rrbs_se/cpp.exit" \
  "$CPP_BINARY" -a "$MM10_R1" -d "$RUNS_ROOT/tmp/mm10.fa" -o "$RUNS_ROOT/rrbs_se/cpp.sam" \
  -s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED"

python3 "$BENCH/p14/compare_sam.py" \
  "$RUNS_ROOT/rrbs_se/cpp.sam" "$RUNS_ROOT/rrbs_se/rust.sam" \
  --summary "$RUNS_ROOT/rrbs_se/compare_sam.json" \
  --field-diff "$RUNS_ROOT/rrbs_se/field_diff.tsv" || true

run_timed "$RUNS_ROOT/rrbs_pe/rust.time" "$RUNS_ROOT/rrbs_pe/rust.stdout" "$RUNS_ROOT/rrbs_pe/rust.stderr" "$RUNS_ROOT/rrbs_pe/rust.exit" \
  "$RUST_BINARY" align -a "$MM10_R1" -b "$MM10_R2" -d "$RUNS_ROOT/tmp/mm10.fa" -o "$RUNS_ROOT/rrbs_pe/rust.sam" \
  -s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED"

python3 - "$RUNS_ROOT" <<'PY' > "$RUNS_ROOT/summary.json"
import json
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])

def read_json(path):
    return json.loads(path.read_text()) if path.exists() else None

def read_exit(path):
    return int(path.read_text().strip()) if path.exists() else None

def time_stats(path):
    text = path.read_text(errors="replace") if path.exists() else ""
    def grab(label):
        m = re.search(rf"{re.escape(label)}: *(.+)", text)
        return m.group(1).strip() if m else ""
    return {
        "elapsed": grab("Elapsed (wall clock) time (h:mm:ss or m:ss)"),
        "user_sec": grab("User time (seconds)"),
        "sys_sec": grab("System time (seconds)"),
        "cpu_pct": grab("Percent of CPU this job got"),
        "max_rss_kib": grab("Maximum resident set size (kbytes)"),
    }

def sam_stats(path):
    total = mapped = unmapped = 0
    flags = {}
    rnames = {}
    if not path.exists():
        return None
    with path.open("r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            if not line.strip() or line.startswith("@"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 11:
                continue
            total += 1
            flag = int(fields[1])
            rname = fields[2]
            flags[str(flag)] = flags.get(str(flag), 0) + 1
            rnames[rname] = rnames.get(rname, 0) + 1
            if flag & 0x4:
                unmapped += 1
            else:
                mapped += 1
    top_rname, top_count = ("NA", 0)
    if rnames:
        top_rname, top_count = max(rnames.items(), key=lambda item: item[1])
    return {
        "total": total,
        "mapped": mapped,
        "unmapped": unmapped,
        "flag_distribution": flags,
        "rname_distribution": rnames,
        "top_rname": top_rname,
        "top_rname_count": top_count,
        "top_rname_pct": round((top_count / total * 100) if total else 0.0, 4),
    }

summary = {
    "example1": {
        "cpp_exit": read_exit(root / "example1/cpp.exit"),
        "rust_index_exit": read_exit(root / "example1/rust_index.exit"),
        "rust_exit": read_exit(root / "example1/rust.exit"),
        "compare": read_json(root / "example1/compare_sam.json"),
        "rust_index_time": time_stats(root / "example1/rust_index.time"),
        "cpp_time": time_stats(root / "example1/cpp.time"),
        "rust_time": time_stats(root / "example1/rust.time"),
    },
    "example2": {
        "cpp_exit": read_exit(root / "example2/cpp.exit"),
        "rust_exit": read_exit(root / "example2/rust.exit"),
        "stats": read_json(root / "example2/sam_stats.json"),
        "cpp_time": time_stats(root / "example2/cpp.time"),
        "rust_time": time_stats(root / "example2/rust.time"),
    },
    "rrbs_se": {
        "cpp_exit": read_exit(root / "rrbs_se/cpp.exit"),
        "rust_index_exit": read_exit(root / "rrbs_index/rust.exit"),
        "rust_exit": read_exit(root / "rrbs_se/rust.exit"),
        "compare": read_json(root / "rrbs_se/compare_sam.json"),
        "rust_index_time": time_stats(root / "rrbs_index/rust.time"),
        "cpp_time": time_stats(root / "rrbs_se/cpp.time"),
        "rust_time": time_stats(root / "rrbs_se/rust.time"),
    },
    "rrbs_pe": {
        "rust_exit": read_exit(root / "rrbs_pe/rust.exit"),
        "stats": sam_stats(root / "rrbs_pe/rust.sam"),
        "rust_time": time_stats(root / "rrbs_pe/rust.time"),
    },
}
print(json.dumps(summary, indent=2, ensure_ascii=False))
PY

printf '%s\n' "$RUNS_ROOT"
