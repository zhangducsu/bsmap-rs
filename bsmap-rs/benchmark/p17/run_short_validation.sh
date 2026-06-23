#!/usr/bin/env bash
set -euo pipefail

if (( $# != 2 )); then
  printf 'Usage: run_short_validation.sh REPO_ROOT RUNS_ROOT\n' >&2
  exit 2
fi

readonly REPO_ROOT="$(cd "$1" && pwd)"
readonly RUNS_ROOT="$2"
readonly BENCH="$REPO_ROOT/bsmap-rs/benchmark"
readonly P16_RUNNER="$BENCH/p16/run_short_validation.sh"
readonly BASELINE_SUMMARY="${P17_BASELINE_SUMMARY:-}"

if [[ ! -f "$P16_RUNNER" ]]; then
  printf 'P16 runner is missing: %s\n' "$P16_RUNNER" >&2
  exit 2
fi

mkdir -p "$RUNS_ROOT"

cat > "$RUNS_ROOT/p17_metadata.txt" <<META
phase=P17
scale_tests_enabled=false
standalone_index_included_in_align_comparison=false
benefit_large_sample_numbers=estimated_only
baseline_summary=${BASELINE_SUMMARY:-}
META

bash "$P16_RUNNER" "$REPO_ROOT" "$RUNS_ROOT"

if [[ -n "$BASELINE_SUMMARY" ]]; then
  python3 "$BENCH/p17/summarize_short_validation.py" \
    --baseline "$BASELINE_SUMMARY" \
    --candidate "$RUNS_ROOT/summary.json" \
    --out "$RUNS_ROOT/p17_comparison.json"
else
  python3 "$BENCH/p17/summarize_short_validation.py" \
    --candidate "$RUNS_ROOT/summary.json" \
    --out "$RUNS_ROOT/p17_comparison.json"
fi

printf '%s\n' "$RUNS_ROOT"
