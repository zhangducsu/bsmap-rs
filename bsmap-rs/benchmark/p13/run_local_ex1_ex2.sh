#!/usr/bin/env bash
set -euo pipefail

STEP_LABEL="${1:-step1_mode_filter}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BENCH="$ROOT/bsmap-rs/benchmark"
OUT="$BENCH/p13/runs/$STEP_LABEL/local"
CPP="$ROOT/bsmap-original/bsmap-2.90/bsmap"
RUST="$ROOT/bsmap-rs/target/release/bsmap"
REF="$BENCH/data/chr22_tail_1M.fa"

mkdir -p "$OUT"/{tmp,example1,example2}

gzip -dc "$BENCH/data/wgbs/ex1_se75_10x/simulated.fastq.gz" > "$OUT/tmp/ex1.fastq"
gzip -dc "$BENCH/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz" > "$OUT/tmp/ex2_1.fastq"
gzip -dc "$BENCH/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz" > "$OUT/tmp/ex2_2.fastq"

sha256sum "$CPP" "$RUST" > "$OUT/binary_sha256.txt"
{
  echo "step=$STEP_LABEL"
  echo "root=$ROOT"
  echo "reference=$REF"
  echo "example1_reads=$BENCH/data/wgbs/ex1_se75_10x/simulated.fastq.gz"
  echo "example2_reads=$BENCH/data/wgbs/ex2_pe150_10x/simulated_1.fastq.gz,$BENCH/data/wgbs/ex2_pe150_10x/simulated_2.fastq.gz"
  echo "cpp=$CPP"
  echo "rust=$RUST"
  echo "params_ex1=-s 16 -v 0.08 -I 4 -p 1"
  echo "params_ex2=-s 16 -v 0.08 -I 4 -p 1"
} > "$OUT/metadata.txt"

run_cpp() {
  local name="$1"; shift
  local outdir="$OUT/$name"
  rm -f "$REF.bsi"
  set +e
  /usr/bin/time -v -o "$outdir/cpp.time" \
    "$CPP" "$@" -d "$REF" -o "$outdir/cpp.sam" -s 16 -v 0.08 -I 4 -p 1 \
    > "$outdir/cpp.stdout" 2> "$outdir/cpp.stderr"
  local status=$?
  set -e
  echo "$status" > "$outdir/cpp.exit"
}

run_rust() {
  local name="$1"; shift
  local outdir="$OUT/$name"
  rm -f "$REF.bsi"
  set +e
  /usr/bin/time -v -o "$outdir/rust.time" \
    "$RUST" align "$@" -d "$REF" -o "$outdir/rust.sam" -s 16 -v 0.08 -I 4 -p 1 \
    > "$outdir/rust.stdout" 2> "$outdir/rust.stderr"
  local status=$?
  set -e
  echo "$status" > "$outdir/rust.exit"
  if [ "$status" -ne 0 ]; then
    return "$status"
  fi
}

run_cpp example1 -a "$OUT/tmp/ex1.fastq"
run_rust example1 -a "$OUT/tmp/ex1.fastq"
python3 "$BENCH/p13/sam_stats.py" \
  --cpp "$OUT/example1/cpp.sam" \
  --rust "$OUT/example1/rust.sam" \
  --out "$OUT/example1/sam_stats.json"

run_cpp example2 -a "$OUT/tmp/ex2_1.fastq" -b "$OUT/tmp/ex2_2.fastq"
run_rust example2 -a "$OUT/tmp/ex2_1.fastq" -b "$OUT/tmp/ex2_2.fastq"
python3 "$BENCH/p13/sam_stats.py" \
  --cpp "$OUT/example2/cpp.sam" \
  --rust "$OUT/example2/rust.sam" \
  --out "$OUT/example2/sam_stats.json"

python3 - "$OUT" > "$OUT/summary.json" <<'PY'
import json, pathlib, re, sys
root = pathlib.Path(sys.argv[1])

def time_stats(path):
    text = pathlib.Path(path).read_text(errors="replace")
    def grab(label):
        m = re.search(rf"{re.escape(label)}: *(.+)", text)
        return m.group(1).strip() if m else ""
    return {
        "elapsed": grab("Elapsed (wall clock) time (h:mm:ss or m:ss)"),
        "user_sec": grab("User time (seconds)"),
        "sys_sec": grab("System time (seconds)"),
        "cpu_pct": grab("Percent of CPU this job got"),
        "max_rss_kb": grab("Maximum resident set size (kbytes)"),
    }

summary = {}
for example in ["example1", "example2"]:
    stats = json.loads((root / example / "sam_stats.json").read_text())
    stats["cpp_time"] = time_stats(root / example / "cpp.time")
    stats["rust_time"] = time_stats(root / example / "rust.time")
    summary[example] = stats
print(json.dumps(summary, indent=2, ensure_ascii=False))
PY

echo "$OUT"
