#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C
export TZ=UTC
ulimit -c 0

usage() {
  cat >&2 <<'EOF'
Usage: run_server_rrbs_subset.sh REPO_ROOT RUNS_ROOT

Environment:
  SSH2_LIMITS="10000 100000"         read-end limits to run against full R1
  REFERENCE=/workspace/00_data/reference/mm10.fa
  READ_1_FULL=/workspace/00_data/rrbs/Ctrl_R1.fq.gz
  RUST_BINARY, CPP_BINARY, THREADS, RANDOM_SEED
  SSH2_PROFILE_RRBS=stage|1|0        Rust-only RRBS profiling mode

The Rust standalone index must already exist at REFERENCE.bsi. This runner
compares warm Rust align time against C++ normal invocation using identical
alignment parameters plus -E <limit>.
EOF
}

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

if (( $# != 2 )); then
  usage
  exit 2
fi

readonly REPO_ROOT="$(cd "$1" && pwd)"
readonly RUNS_ROOT="$2"
readonly TOOL_DIR="$REPO_ROOT/bsmap-rs/benchmark/ssh1"
readonly REFERENCE="${REFERENCE:-/workspace/00_data/reference/mm10.fa}"
readonly INDEX_PATH="${INDEX_PATH:-$REFERENCE.bsi}"
readonly READ_1_FULL="${READ_1_FULL:-/workspace/00_data/rrbs/Ctrl_R1.fq.gz}"
readonly RUST_BINARY="${RUST_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly CPP_BINARY="${CPP_BINARY:-$REPO_ROOT/bsmap-original/bsmap-2.90/bsmap}"
readonly THREADS="${THREADS:-8}"
readonly RANDOM_SEED="${RANDOM_SEED:-1}"
readonly LIMIT_SPEC="${SSH2_LIMITS:-10000 100000}"
readonly PROFILE_RRBS="${SSH2_PROFILE_RRBS:-0}"
readonly -a COMMON_ARGS=(-s 12 -v 0.08 -I 4 -D C-CGG -p "$THREADS" -S "$RANDOM_SEED")

for command_name in date git ln mkdir python3 readlink sha256sum stat; do
  command -v "$command_name" >/dev/null 2>&1 || fail "missing command: $command_name"
done
[[ -x /usr/bin/time ]] || fail "missing executable: /usr/bin/time"
for path in "$REFERENCE" "$INDEX_PATH" "$READ_1_FULL" "$RUST_BINARY" "$CPP_BINARY"; do
  [[ -f "$path" ]] || fail "missing file: $path"
done
for script in parse_time.py sam_stats.py sam_compare_stream.py summarize_server_rrbs.py; do
  [[ -f "$TOOL_DIR/$script" ]] || fail "missing script: $TOOL_DIR/$script"
done

read -r -a LIMITS <<< "$LIMIT_SPEC"
if (( ${#LIMITS[@]} == 0 )); then
  fail "SSH2_LIMITS produced no limits"
fi
for limit in "${LIMITS[@]}"; do
  [[ "$limit" =~ ^[1-9][0-9]*$ ]] || fail "invalid read limit: $limit"
done

mkdir -p "$RUNS_ROOT"
readonly RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
readonly RUN_DIR="$RUNS_ROOT/$RUN_ID"
readonly WORK_DIR="$RUN_DIR/work"
readonly RUST_REF="$WORK_DIR/rust/mm10.fa"
readonly CPP_REF="$WORK_DIR/cpp/mm10.fa"
readonly METADATA="$RUN_DIR/metadata.tsv"
mkdir -p "$WORK_DIR/rust" "$WORK_DIR/cpp" "$RUN_DIR/comparisons"
ln -s "$REFERENCE" "$RUST_REF"
ln -s "$INDEX_PATH" "$RUST_REF.bsi"
ln -s "$REFERENCE" "$CPP_REF"

write_metadata() {
  printf '%s\t%s\n' "$1" "$2" >> "$METADATA"
}

sha256_value() {
  local result
  result="$(sha256sum -- "$1")"
  printf '%s\n' "${result%% *}"
}

COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null)" || fail "not a Git checkout: $REPO_ROOT"
REPO_DIRTY=false
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)" ]]; then
  REPO_DIRTY=true
fi
readonly INDEX_SHA_BEFORE="$(sha256_value "$INDEX_PATH")"

: > "$METADATA"
write_metadata schema_version 1
write_metadata run_id "$RUN_ID"
write_metadata started_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
write_metadata repo_root "$REPO_ROOT"
write_metadata commit "$COMMIT"
write_metadata repo_dirty "$REPO_DIRTY"
write_metadata run_dir "$RUN_DIR"
write_metadata reference "$(readlink -f "$REFERENCE")"
write_metadata reference_sha256 "$(sha256_value "$REFERENCE")"
write_metadata index "$(readlink -f "$INDEX_PATH")"
write_metadata index_sha256_before "$INDEX_SHA_BEFORE"
write_metadata index_size_bytes "$(stat -c '%s' "$INDEX_PATH")"
write_metadata read_1_full "$READ_1_FULL"
write_metadata read_1_full_sha256 "$(sha256_value "$READ_1_FULL")"
write_metadata rust_binary "$(readlink -f "$RUST_BINARY")"
write_metadata rust_binary_sha256 "$(sha256_value "$RUST_BINARY")"
write_metadata cpp_binary "$(readlink -f "$CPP_BINARY")"
write_metadata cpp_binary_sha256 "$(sha256_value "$CPP_BINARY")"
write_metadata limits "$LIMIT_SPEC"
write_metadata threads "$THREADS"
write_metadata random_seed "$RANDOM_SEED"
write_metadata rrbs_parameters "-s 12 -v 0.08 -I 4 -D C-CGG -p $THREADS -S $RANDOM_SEED"
write_metadata standalone_index_included false
write_metadata rrbs_profile_enabled "$PROFILE_RRBS"

run_case() {
  local case_name="$1"
  shift
  local case_dir="$RUN_DIR/case_$case_name"
  local sam_path="$case_dir/output.sam"
  local status
  local -a command=("$@")
  local -a env_args=()
  if [[ "$case_name" == rust_* && "$PROFILE_RRBS" != "0" ]]; then
    env_args=(env "BSMAP_PROFILE_RRBS=$PROFILE_RRBS")
  fi
  mkdir -p "$case_dir"
  : > "$sam_path"
  printf '%q ' "${env_args[@]}" "${command[@]}" > "$case_dir/command.txt"
  printf '\n' >> "$case_dir/command.txt"

  set +e
  /usr/bin/time -v -o "$case_dir/time.txt" -- \
    "${env_args[@]}" "${command[@]}" > "$case_dir/stdout.txt" 2> "$case_dir/stderr.txt"
  status=$?
  set -e
  printf '%s\n' "$status" > "$case_dir/exit_code.txt"
  sha256sum -- "$sam_path" > "$case_dir/output.sam.sha256"
  python3 "$TOOL_DIR/parse_time.py" "$case_dir/time.txt" \
    --outer-exit-code "$status" --output "$case_dir/time.json" \
    > "$case_dir/parse_time.stdout"
  python3 "$TOOL_DIR/sam_stats.py" "$sam_path" --output "$case_dir/sam_stats.json" \
    > "$case_dir/sam_stats.stdout"
  printf '%-18s exit=%s\n' "$case_name" "$status"
}

run_compare() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  local status
  set +e
  python3 "$TOOL_DIR/sam_compare_stream.py" "$expected" "$actual" \
    --summary "$RUN_DIR/comparisons/$label.json" \
    --field-diff "$RUN_DIR/comparisons/$label.field_diff.tsv" \
    > "$RUN_DIR/comparisons/$label.stdout" \
    2> "$RUN_DIR/comparisons/$label.stderr"
  status=$?
  set -e
  printf '%s\n' "$status" > "$RUN_DIR/comparisons/$label.exit_code.txt"
  printf 'compare %-12s exit=%s\n' "$label" "$status"
}

for limit in "${LIMITS[@]}"; do
  run_case "rust_se_$limit" "$RUST_BINARY" align \
    -a "$READ_1_FULL" -d "$RUST_REF" -o "$RUN_DIR/case_rust_se_$limit/output.sam" \
    "${COMMON_ARGS[@]}" -E "$limit"
  run_case "cpp_se_$limit" "$CPP_BINARY" \
    -a "$READ_1_FULL" -d "$CPP_REF" -o "$RUN_DIR/case_cpp_se_$limit/output.sam" \
    "${COMMON_ARGS[@]}" -E "$limit"
  run_compare "se_$limit" "$RUN_DIR/case_cpp_se_$limit/output.sam" "$RUN_DIR/case_rust_se_$limit/output.sam"
done

readonly INDEX_SHA_AFTER="$(sha256_value "$INDEX_PATH")"
write_metadata index_sha256_after "$INDEX_SHA_AFTER"
write_metadata finished_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if [[ "$INDEX_SHA_AFTER" != "$INDEX_SHA_BEFORE" ]]; then
  fail "Rust alignment changed or rebuilt the reference index"
fi

python3 "$TOOL_DIR/summarize_server_rrbs.py" "$RUN_DIR" > "$RUN_DIR/summary.json"
printf 'summary=%s\n' "$RUN_DIR/summary.json"
