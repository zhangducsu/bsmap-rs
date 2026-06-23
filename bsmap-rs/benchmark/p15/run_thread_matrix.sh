#!/usr/bin/env bash
set -euo pipefail

if (( $# != 4 )); then
  printf 'Usage: run_thread_matrix.sh REPO_ROOT REFERENCE READ_1 RUNS_ROOT\n' >&2
  exit 2
fi

readonly REPO_ROOT="$1"
readonly REFERENCE="$2"
readonly READ_1="$3"
readonly RUNS_ROOT="$4"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly THREAD_MATRIX="${THREAD_MATRIX:-1 2 4 8 16}"
readonly MATRIX_REPETITIONS="${MATRIX_REPETITIONS:-3}"

[[ -n "${REPEATS:-}" || -n "${TARGET_SOURCE_BYTES:-}" || -n "${TARGET_EMITTED_BYTES:-}" ]] || {
  printf 'error: set the scale workload with REPEATS, TARGET_SOURCE_BYTES, or TARGET_EMITTED_BYTES\n' >&2
  exit 2
}

for threads in $THREAD_MATRIX; do
  for (( repetition = 1; repetition <= MATRIX_REPETITIONS; repetition += 1 )); do
    THREADS="$threads" bash "$SCRIPT_DIR/run_stream_scale.sh" \
      "$REPO_ROOT" "$REFERENCE" "$READ_1" "$RUNS_ROOT/p$threads"
  done
done

python3 "$SCRIPT_DIR/summarize_matrix.py" "$RUNS_ROOT" \
  --output "$RUNS_ROOT/thread_matrix.json"
