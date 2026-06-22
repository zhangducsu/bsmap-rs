#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C
export TZ=UTC

usage() {
  cat >&2 <<'EOF'
Usage: run_rust_benchmark.sh REPO_ROOT REFERENCE READ_1 [RUNS_ROOT]

Optional environment variables:
  RUST_BINARY, READ_2, SEED_SIZE, INDEX_INTERVAL, DIGESTION_SITE,
  THREADS, RANDOM_SEED, MISMATCH_RATE, WARM_RUNS, GIT_COMMIT, REPO_DIRTY
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

if (( $# < 3 || $# > 4 )); then
  usage
  exit 2
fi

readonly REPO_ROOT="$1"
readonly REFERENCE="$2"
readonly READ_1="$3"
readonly RUNS_ROOT="${4:-$REPO_ROOT/bsmap-rs/benchmark/p14/runs}"
readonly RUST_BINARY="${RUST_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly READ_2="${READ_2:-}"
readonly SEED_SIZE="${SEED_SIZE:-16}"
readonly INDEX_INTERVAL="${INDEX_INTERVAL:-4}"
readonly DIGESTION_SITE="${DIGESTION_SITE:-}"
readonly THREADS="${THREADS:-1}"
readonly RANDOM_SEED="${RANDOM_SEED:-1}"
readonly MISMATCH_RATE="${MISMATCH_RATE:-0.08}"
readonly WARM_RUNS="${WARM_RUNS:-3}"

[[ "$WARM_RUNS" =~ ^[1-9][0-9]*$ ]] || fail "WARM_RUNS must be a positive integer"

for command_name in date git readlink sha256sum stat; do
  command -v "$command_name" >/dev/null 2>&1 || fail "missing command: $command_name"
done
[[ -x /usr/bin/time ]] || fail "missing executable: /usr/bin/time"

for required_file in "$REFERENCE" "$READ_1" "$RUST_BINARY"; do
  [[ -f "$required_file" ]] || fail "missing file: $required_file"
done
if [[ -n "$READ_2" && ! -f "$READ_2" ]]; then
  fail "missing file: $READ_2"
fi

readonly RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
readonly RUN_DIR="$RUNS_ROOT/$RUN_ID"
readonly WORK_DIR="$RUN_DIR/work"
readonly TEMP_REFERENCE="$WORK_DIR/reference.fa"
readonly INDEX_PATH="$TEMP_REFERENCE.bsi"
readonly HASH_FILE="$RUN_DIR/sha256.tsv"
readonly METADATA_FILE="$RUN_DIR/metadata.tsv"

mkdir -p "$WORK_DIR"
ln -s "$(readlink -f -- "$REFERENCE")" "$TEMP_REFERENCE"
: > "$HASH_FILE"
: > "$METADATA_FILE"

write_metadata() {
  printf '%s\t%s\n' "$1" "$2" >> "$METADATA_FILE"
}

sha256_of() {
  local digest
  digest="$(sha256sum -- "$1")"
  printf '%s\n' "${digest%% *}"
}

write_hash() {
  local category="$1"
  local name="$2"
  local path="$3"
  write_hash_value "$category" "$name" "$(sha256_of "$path")" "$path"
}

write_hash_value() {
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$HASH_FILE"
}

run_timed() {
  local case_name="$1"
  shift
  local case_dir="$RUN_DIR/$case_name"
  local status
  local -a command=("$@")

  mkdir -p "$case_dir"
  printf '%q ' "${command[@]}" > "$case_dir/command.txt"
  printf '\n' >> "$case_dir/command.txt"

  set +e
  /usr/bin/time -v -o "$case_dir/time.txt" -- \
    "${command[@]}" > "$case_dir/stdout.txt" 2> "$case_dir/stderr.txt"
  status=$?
  set -e
  printf '%s\n' "$status" > "$case_dir/exit_code.txt"
  return "$status"
}

COMMIT="${GIT_COMMIT:-}"
if [[ -z "$COMMIT" ]]; then
  COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null)" || \
    fail "cannot resolve commit; set GIT_COMMIT for a Windows linked worktree"
fi
REPO_DIRTY="${REPO_DIRTY:-false}"
if git -C "$REPO_ROOT" status --porcelain >/dev/null 2>&1; then
  if [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]]; then
    REPO_DIRTY=true
  fi
fi

write_metadata schema_version 1
write_metadata run_id "$RUN_ID"
write_metadata started_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
write_metadata commit "$COMMIT"
write_metadata repo_dirty "$REPO_DIRTY"
write_metadata repo_root "$REPO_ROOT"
write_metadata run_dir "$RUN_DIR"
write_metadata seed_size "$SEED_SIZE"
write_metadata index_interval "$INDEX_INTERVAL"
write_metadata digestion_site "${DIGESTION_SITE:-none}"
write_metadata threads "$THREADS"
write_metadata random_seed "$RANDOM_SEED"
write_metadata mismatch_rate "$MISMATCH_RATE"
write_metadata warm_runs "$WARM_RUNS"

write_hash input reference "$REFERENCE"
write_hash input read_1 "$READ_1"
if [[ -n "$READ_2" ]]; then
  write_hash input read_2 "$READ_2"
fi
write_hash binary rust "$RUST_BINARY"

index_command=(
  "$RUST_BINARY" index -d "$TEMP_REFERENCE"
  -s "$SEED_SIZE" -I "$INDEX_INTERVAL"
)
if [[ -n "$DIGESTION_SITE" ]]; then
  index_command+=(-D "$DIGESTION_SITE")
fi

run_timed standalone_index "${index_command[@]}"
[[ -f "$INDEX_PATH" ]] || fail "standalone index did not create: $INDEX_PATH"
readonly INDEX_SHA256="$(sha256_of "$INDEX_PATH")"
write_hash_value output standalone_index "$INDEX_SHA256" "$INDEX_PATH"
write_metadata index_size_bytes "$(stat -c '%s' -- "$INDEX_PATH")"

for ((run = 1; run <= WARM_RUNS; run++)); do
  case_name="warm_align_$run"
  align_command=(
    "$RUST_BINARY" align -a "$READ_1" -d "$TEMP_REFERENCE"
    -o "$RUN_DIR/$case_name/output.sam"
    -s "$SEED_SIZE" -v "$MISMATCH_RATE" -I "$INDEX_INTERVAL"
    -p "$THREADS" -S "$RANDOM_SEED"
  )
  if [[ -n "$READ_2" ]]; then
    align_command+=(-b "$READ_2")
  fi
  if [[ -n "$DIGESTION_SITE" ]]; then
    align_command+=(-D "$DIGESTION_SITE")
  fi

  run_timed "$case_name" "${align_command[@]}"
  [[ "$(sha256_of "$INDEX_PATH")" == "$INDEX_SHA256" ]] || \
    fail "warm align modified or rebuilt the standalone index"
  write_hash output "${case_name}_sam" "$RUN_DIR/$case_name/output.sam"
done
write_metadata finished_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf 'run_dir=%s\n' "$RUN_DIR"
