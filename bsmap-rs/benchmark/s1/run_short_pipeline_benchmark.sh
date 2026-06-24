#!/usr/bin/env bash
set -euo pipefail

if (( $# != 3 )); then
  printf 'Usage: run_short_pipeline_benchmark.sh REPO_ROOT BASELINE_BINARY RUN_ROOT\n' >&2
  exit 2
fi

readonly REPO_ROOT="$(cd "$1" && pwd)"
readonly BASELINE_BINARY="$2"
readonly RUN_ROOT="$3"
readonly CURRENT_BINARY="${CURRENT_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly MM10_DATA="${MM10_DATA:-/mnt/d/BSMAP/benchmark-data/mm10}"
readonly MM10_REF="$MM10_DATA/mm10.fa"
readonly WGBS_SE="$MM10_DATA/wgbs_10k/se75_10k/simulated.fastq.gz"
readonly WGBS_R1="$MM10_DATA/wgbs_10k/pe150_10k/simulated_1.fastq.gz"
readonly WGBS_R2="$MM10_DATA/wgbs_10k/pe150_10k/simulated_2.fastq.gz"
readonly RRBS_R1="$MM10_DATA/Ctrl_10K_R1.fq"
readonly RRBS_R2="$MM10_DATA/Ctrl_10K_R2.fq"
readonly THREADS="${THREADS:-8}"
readonly RANDOM_SEED="${RANDOM_SEED:-1}"

mkdir -p "$RUN_ROOT"/{tmp,index,align}
ln -sf "$MM10_REF" "$RUN_ROOT/tmp/mm10.fa"
readonly REF_LINK="$RUN_ROOT/tmp/mm10.fa"
readonly INDEX_FILE="$RUN_ROOT/tmp/mm10.fa.bsi"

sha256sum \
  "$BASELINE_BINARY" "$CURRENT_BINARY" "$MM10_REF" \
  "$WGBS_SE" "$WGBS_R1" "$WGBS_R2" "$RRBS_R1" "$RRBS_R2" \
  > "$RUN_ROOT/input_sha256.txt"

cat > "$RUN_ROOT/metadata.tsv" <<META
repo_root	$REPO_ROOT
baseline_binary	$BASELINE_BINARY
current_binary	$CURRENT_BINARY
mm10_reference	$MM10_REF
wgbs_se	$WGBS_SE
wgbs_r1	$WGBS_R1
wgbs_r2	$WGBS_R2
rrbs_r1	$RRBS_R1
rrbs_r2	$RRBS_R2
threads	$THREADS
random_seed	$RANDOM_SEED
standalone_index_included	false
META

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
}

sam_summary() {
  local sam="$1"
  local out="$2"
  python3 - "$sam" "$out" <<'PY'
import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

sam = Path(sys.argv[1])
out = Path(sys.argv[2])
total = mapped = unmapped = 0
flags = Counter()
rnames = Counter()
with sam.open("r", encoding="utf-8", errors="replace") as handle:
    for line in handle:
        if not line.strip() or line.startswith("@"):
            continue
        fields = line.rstrip("\n").split("\t")
        if len(fields) < 11:
            continue
        total += 1
        flag = int(fields[1])
        flags[str(flag)] += 1
        rnames[fields[2]] += 1
        if flag & 0x4:
            unmapped += 1
        else:
            mapped += 1
top_rname, top_count = ("NA", 0)
if rnames:
    top_rname, top_count = rnames.most_common(1)[0]
summary = {
    "sam_sha256": hashlib.sha256(sam.read_bytes()).hexdigest(),
    "total": total,
    "mapped": mapped,
    "unmapped": unmapped,
    "flags": dict(flags.most_common()),
    "top_rname": top_rname,
    "top_rname_count": top_count,
    "top_rname_pct": round((top_count / mapped * 100.0) if mapped else 0.0, 4),
}
out.write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
PY
}

run_index() {
  local mode="$1"
  local binary="$2"
  local name="$3"
  rm -f "$INDEX_FILE"
  if [[ "$mode" == "rrbs" ]]; then
    run_timed "$RUN_ROOT/index/${name}.time" "$RUN_ROOT/index/${name}.stdout" \
      "$RUN_ROOT/index/${name}.stderr" "$RUN_ROOT/index/${name}.exit" \
      "$binary" index -d "$REF_LINK" -s 12 -I 4 -D C-CGG
  else
    run_timed "$RUN_ROOT/index/${name}.time" "$RUN_ROOT/index/${name}.stdout" \
      "$RUN_ROOT/index/${name}.stderr" "$RUN_ROOT/index/${name}.exit" \
      "$binary" index -d "$REF_LINK" -s 16 -I 4
  fi
}

run_align() {
  local label="$1"
  local binary="$2"
  local depth_arg="$3"
  local mode="$4"
  local layout="$5"
  local out_dir="$RUN_ROOT/align/$label"
  mkdir -p "$out_dir"
  if [[ "$mode" == "rrbs" && "$layout" == "pe" ]]; then
    run_timed "$out_dir/time.txt" "$out_dir/stdout.txt" "$out_dir/stderr.txt" "$out_dir/exit.txt" \
      "$binary" align -a "$RRBS_R1" -b "$RRBS_R2" -d "$REF_LINK" -o "$out_dir/out.sam" \
      -s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED" $depth_arg
  elif [[ "$mode" == "rrbs" ]]; then
    run_timed "$out_dir/time.txt" "$out_dir/stdout.txt" "$out_dir/stderr.txt" "$out_dir/exit.txt" \
      "$binary" align -a "$RRBS_R1" -d "$REF_LINK" -o "$out_dir/out.sam" \
      -s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED" $depth_arg
  elif [[ "$layout" == "pe" ]]; then
    run_timed "$out_dir/time.txt" "$out_dir/stdout.txt" "$out_dir/stderr.txt" "$out_dir/exit.txt" \
      "$binary" align -a "$WGBS_R1" -b "$WGBS_R2" -d "$REF_LINK" -o "$out_dir/out.sam" \
      -s 16 -v 0.08 -I 4 -p "$THREADS" -S "$RANDOM_SEED" $depth_arg
  else
    run_timed "$out_dir/time.txt" "$out_dir/stdout.txt" "$out_dir/stderr.txt" "$out_dir/exit.txt" \
      "$binary" align -a "$WGBS_SE" -d "$REF_LINK" -o "$out_dir/out.sam" \
      -s 16 -v 0.08 -I 4 -p "$THREADS" -S "$RANDOM_SEED" $depth_arg
  fi
  sam_summary "$out_dir/out.sam" "$out_dir/sam_summary.json"
}

run_index wgbs "$BASELINE_BINARY" baseline_wgbs
run_align baseline_wgbs_se "$BASELINE_BINARY" "" wgbs se
run_align baseline_wgbs_pe "$BASELINE_BINARY" "" wgbs pe

run_index wgbs "$CURRENT_BINARY" current_wgbs
run_align current_wgbs_se_d1 "$CURRENT_BINARY" "--pipeline-depth 1" wgbs se
run_align current_wgbs_se_d2 "$CURRENT_BINARY" "--pipeline-depth 2" wgbs se
run_align current_wgbs_pe_d1 "$CURRENT_BINARY" "--pipeline-depth 1" wgbs pe
run_align current_wgbs_pe_d2 "$CURRENT_BINARY" "--pipeline-depth 2" wgbs pe

run_index rrbs "$BASELINE_BINARY" baseline_rrbs
run_align baseline_rrbs_se "$BASELINE_BINARY" "" rrbs se
run_align baseline_rrbs_pe "$BASELINE_BINARY" "" rrbs pe

run_index rrbs "$CURRENT_BINARY" current_rrbs
run_align current_rrbs_se_d1 "$CURRENT_BINARY" "--pipeline-depth 1" rrbs se
run_align current_rrbs_se_d2 "$CURRENT_BINARY" "--pipeline-depth 2" rrbs se
run_align current_rrbs_pe_d1 "$CURRENT_BINARY" "--pipeline-depth 1" rrbs pe
run_align current_rrbs_pe_d2 "$CURRENT_BINARY" "--pipeline-depth 2" rrbs pe

python3 "$REPO_ROOT/bsmap-rs/benchmark/s1/summarize_short_pipeline.py" "$RUN_ROOT" \
  > "$RUN_ROOT/summary.json"
printf '%s\n' "$RUN_ROOT"
