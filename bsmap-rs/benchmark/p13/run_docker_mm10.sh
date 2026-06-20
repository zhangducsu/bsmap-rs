#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C
export TZ=UTC

readonly REPO_ROOT="${1:-/tmp/p13_codex_github}"
readonly RUNS_ROOT="${2:-$REPO_ROOT/bsmap-rs/benchmark/p13/runs/docker_mm10}"
readonly SCRIPT_DIR="$REPO_ROOT/bsmap-rs/benchmark/p13"

readonly REFERENCE="/workspace/00_data/reference/mm10.fa"
readonly READ_1="/workspace/00_data/rrbs/Ctrl_10K_R1.fq"
readonly READ_2="/workspace/00_data/rrbs/Ctrl_10K_R2.fq"
readonly RUST_BINARY="${RUST_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly CPP_BINARY="${CPP_BINARY:-$REPO_ROOT/bsmap-original/bsmap-2.90/bsmap}"
readonly SAM_STATS="$SCRIPT_DIR/sam_stats.py"
readonly SUMMARIZER="$SCRIPT_DIR/summarize_mm10_run.py"

readonly -a COMMON_PARAMETERS=(-s 12 -v 0.08 -I 4 -D C-CGG -p 8)

fail() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

for command_name in git python3 sha256sum; do
  command -v "$command_name" >/dev/null 2>&1 || fail "missing command: $command_name"
done
[[ -x /usr/bin/time ]] || fail "missing executable: /usr/bin/time"

for required_file in \
  "$REFERENCE" \
  "$READ_1" \
  "$READ_2" \
  "$RUST_BINARY" \
  "$CPP_BINARY" \
  "$SAM_STATS" \
  "$SUMMARIZER"; do
  [[ -f "$required_file" ]] || fail "missing file: $required_file"
done

readonly STARTED_AT_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
readonly RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
readonly RUN_DIR="$RUNS_ROOT/$RUN_ID"
readonly TEMP_DIR="$RUN_DIR/work"
readonly TEMP_REFERENCE="$TEMP_DIR/mm10.fa"
readonly METADATA_FILE="$RUN_DIR/metadata.tsv"
readonly HASH_FILE="$RUN_DIR/sha256.tsv"

COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null)" || fail "not a Git checkout: $REPO_ROOT"
[[ -n "$COMMIT" ]] || fail "empty Git commit: $REPO_ROOT"
readonly COMMIT
REPO_DIRTY=false
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain 2>/dev/null)" ]]; then
  REPO_DIRTY=true
fi
readonly REPO_DIRTY

mkdir -p "$TEMP_DIR" "$RUN_DIR/comparisons"
ln -s "$REFERENCE" "$TEMP_REFERENCE"
: > "$HASH_FILE"

write_metadata() {
  printf '%s\t%s\n' "$1" "$2" >> "$METADATA_FILE"
}

write_hash() {
  local category="$1"
  local name="$2"
  local path="$3"
  local digest
  digest="$(sha256sum -- "$path")" || fail "sha256 failed: $path"
  digest="${digest%% *}"
  printf '%s\t%s\t%s\t%s\n' "$category" "$name" "$digest" "$path" >> "$HASH_FILE"
}

: > "$METADATA_FILE"
write_metadata schema_version 1
write_metadata run_id "$RUN_ID"
write_metadata started_at_utc "$STARTED_AT_UTC"
write_metadata commit "$COMMIT"
write_metadata repo_dirty "$REPO_DIRTY"
write_metadata repo_root "$REPO_ROOT"
write_metadata run_dir "$RUN_DIR"
write_metadata reference "$REFERENCE"
write_metadata read_1 "$READ_1"
write_metadata read_2 "$READ_2"
write_metadata rust_binary "$RUST_BINARY"
write_metadata cpp_binary "$CPP_BINARY"
write_metadata common_parameters "-s 12 -v 0.08 -I 4 -D C-CGG -p 8"

write_hash input reference "$REFERENCE"
write_hash input read_1 "$READ_1"
write_hash input read_2 "$READ_2"
write_hash binary rust "$RUST_BINARY"
write_hash binary cpp "$CPP_BINARY"

run_case() {
  local case_name="$1"
  local implementation="$2"
  local layout="$3"
  shift 3

  local case_dir="$RUN_DIR/$case_name"
  local sam_path="$case_dir/output.sam"
  local status
  local -a command=("$@")

  mkdir -p "$case_dir"
  : > "$sam_path"
  printf '%q ' "${command[@]}" > "$case_dir/command.txt"
  printf '\n' >> "$case_dir/command.txt"

  # The source reference index is never touched; only the temporary symlink's index is removed.
  rm -f -- "$TEMP_REFERENCE.bsi"
  set +e
  /usr/bin/time -v -o "$case_dir/time.txt" \
    "${command[@]}" > "$case_dir/stdout.txt" 2> "$case_dir/stderr.txt"
  status=$?
  set -e
  printf '%s\n' "$status" > "$case_dir/exit_code.txt"
  rm -f -- "$TEMP_REFERENCE.bsi"

  printf '%s\n' "$implementation" > "$case_dir/implementation.txt"
  printf '%s\n' "$layout" > "$case_dir/layout.txt"
  write_hash output "${case_name}_sam" "$sam_path"
  printf '%-8s exit=%s\n' "$case_name" "$status"
}

run_case rust_se rust SE \
  "$RUST_BINARY" align -a "$READ_1" -d "$TEMP_REFERENCE" \
  -o "$RUN_DIR/rust_se/output.sam" "${COMMON_PARAMETERS[@]}"

run_case cpp_se cpp SE \
  "$CPP_BINARY" -a "$READ_1" -d "$TEMP_REFERENCE" \
  -o "$RUN_DIR/cpp_se/output.sam" "${COMMON_PARAMETERS[@]}"

run_case rust_pe rust PE \
  "$RUST_BINARY" align -a "$READ_1" -b "$READ_2" -d "$TEMP_REFERENCE" \
  -o "$RUN_DIR/rust_pe/output.sam" "${COMMON_PARAMETERS[@]}"

# C++ PE is known to return 134 on this dataset. run_case records it and returns normally.
run_case cpp_pe cpp PE \
  "$CPP_BINARY" -a "$READ_1" -b "$READ_2" -d "$TEMP_REFERENCE" \
  -o "$RUN_DIR/cpp_pe/output.sam" "${COMMON_PARAMETERS[@]}"

run_sam_stats() {
  local layout="$1"
  local cpp_sam="$2"
  local rust_sam="$3"
  local stats_file="$RUN_DIR/comparisons/${layout}.json"
  local status

  set +e
  python3 "$SAM_STATS" --cpp "$cpp_sam" --rust "$rust_sam" --out "$stats_file" \
    > "$RUN_DIR/comparisons/${layout}.stdout.txt" \
    2> "$RUN_DIR/comparisons/${layout}.stderr.txt"
  status=$?
  set -e
  printf '%s\n' "$status" > "$RUN_DIR/comparisons/${layout}.exit_code.txt"
  printf 'sam_stats %-2s exit=%s\n' "$layout" "$status"
}

run_sam_stats se "$RUN_DIR/cpp_se/output.sam" "$RUN_DIR/rust_se/output.sam"
run_sam_stats pe "$RUN_DIR/cpp_pe/output.sam" "$RUN_DIR/rust_pe/output.sam"

readonly FINISHED_AT_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
write_metadata finished_at_utc "$FINISHED_AT_UTC"

python3 "$SUMMARIZER" "$RUN_DIR" > "$RUN_DIR/summary.json" || fail "summary generation failed"
printf 'summary=%s\n' "$RUN_DIR/summary.json"
