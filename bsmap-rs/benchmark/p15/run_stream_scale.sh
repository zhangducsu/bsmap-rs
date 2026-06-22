#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C
export TZ=UTC
ulimit -c 0

usage() {
  cat >&2 <<'EOF'
Usage: run_stream_scale.sh REPO_ROOT REFERENCE READ_1 [RUNS_ROOT]

Exactly one target must be set:
  TARGET_SOURCE_BYTES=90G | TARGET_EMITTED_BYTES=90G | REPEATS=100

Optional environment variables:
  RUST_BINARY, READ_2, SEED_SIZE, INDEX_INTERVAL, DIGESTION_SITE,
  THREADS, RANDOM_SEED, MISMATCH_RATE, SINK_MIB_PER_SEC,
  PAGE_CACHE_STATE, GIT_COMMIT, REPO_DIRTY

The reference.bsi must already exist. Standalone index time is intentionally
excluded from this scale alignment measurement.
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
readonly RUNS_ROOT="${4:-$REPO_ROOT/bsmap-rs/benchmark/p15/runs}"
readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly RUST_BINARY="${RUST_BINARY:-$REPO_ROOT/bsmap-rs/target/release/bsmap}"
readonly READ_2="${READ_2:-}"
readonly SEED_SIZE="${SEED_SIZE:-16}"
readonly INDEX_INTERVAL="${INDEX_INTERVAL:-4}"
readonly DIGESTION_SITE="${DIGESTION_SITE:-}"
readonly THREADS="${THREADS:-8}"
readonly RANDOM_SEED="${RANDOM_SEED:-1}"
readonly MISMATCH_RATE="${MISMATCH_RATE:-0.08}"
readonly SINK_MIB_PER_SEC="${SINK_MIB_PER_SEC:-0}"
readonly PAGE_CACHE_STATE="${PAGE_CACHE_STATE:-uncontrolled}"
readonly INDEX_PATH="$REFERENCE.bsi"

for command_name in date git mkfifo mktemp python3 readlink sha256sum stat; do
  command -v "$command_name" >/dev/null 2>&1 || fail "missing command: $command_name"
done
[[ -x /usr/bin/time ]] || fail "missing executable: /usr/bin/time"
for required_file in "$REFERENCE" "$READ_1" "$INDEX_PATH" "$RUST_BINARY"; do
  [[ -f "$required_file" ]] || fail "missing file: $required_file"
done
if [[ -n "$READ_2" && ! -f "$READ_2" ]]; then
  fail "missing file: $READ_2"
fi

target_count=0
[[ -n "${TARGET_SOURCE_BYTES:-}" ]] && ((target_count += 1))
[[ -n "${TARGET_EMITTED_BYTES:-}" ]] && ((target_count += 1))
[[ -n "${REPEATS:-}" ]] && ((target_count += 1))
(( target_count == 1 )) || fail "set exactly one of TARGET_SOURCE_BYTES, TARGET_EMITTED_BYTES, REPEATS"
if [[ -n "$READ_2" && -n "${TARGET_EMITTED_BYTES:-}" ]]; then
  fail "PE scale runs require TARGET_SOURCE_BYTES or REPEATS"
fi

mkdir -p "$RUNS_ROOT"
readonly RUN_DIR="$(mktemp -d "$RUNS_ROOT/$(date -u +%Y%m%dT%H%M%SZ).XXXXXX")"
readonly RUN_ID="${RUN_DIR##*/}"
readonly FIFO_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bsmap-p15-scale.XXXXXX")"
readonly FIFO_R1="$FIFO_DIR/input_r1.fastq"
readonly FIFO_R2="$FIFO_DIR/input_r2.fastq"
readonly FIFO_SAM="$FIFO_DIR/output.sam"
readonly METADATA="$RUN_DIR/metadata.tsv"
mkfifo "$FIFO_R1" "$FIFO_SAM"
if [[ -n "$READ_2" ]]; then
  mkfifo "$FIFO_R2"
fi

producer_pid=""
sink_pid=""
cleanup() {
  if [[ -n "$producer_pid" ]]; then kill "$producer_pid" 2>/dev/null || true; fi
  if [[ -n "$sink_pid" ]]; then kill "$sink_pid" 2>/dev/null || true; fi
  rm -f -- "$FIFO_R1" "$FIFO_R2" "$FIFO_SAM"
  rmdir -- "$FIFO_DIR" 2>/dev/null || true
}
trap cleanup EXIT

write_metadata() {
  printf '%s\t%s\n' "$1" "$2" >> "$METADATA"
}

sha256_of() {
  local digest
  digest="$(sha256sum -- "$1")"
  printf '%s\n' "${digest%% *}"
}

COMMIT="${GIT_COMMIT:-}"
if [[ -z "$COMMIT" ]]; then
  COMMIT="$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null)" || \
    fail "cannot resolve commit; set GIT_COMMIT for a Windows linked worktree"
fi
REPO_DIRTY="${REPO_DIRTY:-false}"
if git -C "$REPO_ROOT" status --porcelain >/dev/null 2>&1 && \
   [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]]; then
  REPO_DIRTY=true
fi

: > "$METADATA"
write_metadata schema_version 1
write_metadata run_id "$RUN_ID"
write_metadata started_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
write_metadata commit "$COMMIT"
write_metadata repo_dirty "$REPO_DIRTY"
write_metadata rust_binary "$(readlink -f -- "$RUST_BINARY")"
write_metadata rust_binary_sha256 "$(sha256_of "$RUST_BINARY")"
write_metadata reference "$(readlink -f -- "$REFERENCE")"
write_metadata reference_sha256 "$(sha256_of "$REFERENCE")"
write_metadata index "$(readlink -f -- "$INDEX_PATH")"
readonly INDEX_SHA256="$(sha256_of "$INDEX_PATH")"
write_metadata index_sha256 "$INDEX_SHA256"
write_metadata index_size_bytes "$(stat -c '%s' -- "$INDEX_PATH")"
write_metadata read_1 "$(readlink -f -- "$READ_1")"
write_metadata read_1_sha256 "$(sha256_of "$READ_1")"
if [[ -n "$READ_2" ]]; then
  write_metadata read_2 "$(readlink -f -- "$READ_2")"
  write_metadata read_2_sha256 "$(sha256_of "$READ_2")"
fi
write_metadata seed_size "$SEED_SIZE"
write_metadata index_interval "$INDEX_INTERVAL"
write_metadata digestion_site "${DIGESTION_SITE:-none}"
write_metadata threads "$THREADS"
write_metadata random_seed "$RANDOM_SEED"
write_metadata mismatch_rate "$MISMATCH_RATE"
write_metadata page_cache_state "$PAGE_CACHE_STATE"
write_metadata sink_mib_per_sec "$SINK_MIB_PER_SEC"
write_metadata target_source_bytes "${TARGET_SOURCE_BYTES:-}"
write_metadata target_emitted_bytes "${TARGET_EMITTED_BYTES:-}"
write_metadata repeats "${REPEATS:-}"
write_metadata standalone_index_included false

producer_command=(
  python3 "$SCRIPT_DIR/stream_fastq.py"
  --input-r1 "$READ_1" --output-r1 "$FIFO_R1"
  --summary "$RUN_DIR/producer.json"
)
if [[ -n "$READ_2" ]]; then
  producer_command+=(--input-r2 "$READ_2" --output-r2 "$FIFO_R2")
fi
if [[ -n "${TARGET_SOURCE_BYTES:-}" ]]; then
  producer_command+=(--target-source-bytes "$TARGET_SOURCE_BYTES")
elif [[ -n "${TARGET_EMITTED_BYTES:-}" ]]; then
  producer_command+=(--target-emitted-bytes "$TARGET_EMITTED_BYTES")
else
  producer_command+=(--repeats "$REPEATS")
fi

sink_command=(
  python3 "$SCRIPT_DIR/slow_sink.py" "$FIFO_SAM"
  --summary "$RUN_DIR/sink.json"
  --rate-mib-per-sec "$SINK_MIB_PER_SEC"
)

align_command=(
  "$RUST_BINARY" align -a "$FIFO_R1" -d "$REFERENCE" -o "$FIFO_SAM"
  -s "$SEED_SIZE" -v "$MISMATCH_RATE" -I "$INDEX_INTERVAL"
  -p "$THREADS" -S "$RANDOM_SEED"
)
if [[ -n "$READ_2" ]]; then align_command+=(-b "$FIFO_R2"); fi
if [[ -n "$DIGESTION_SITE" ]]; then align_command+=(-D "$DIGESTION_SITE"); fi
printf '%q ' "${align_command[@]}" > "$RUN_DIR/command.txt"
printf '\n' >> "$RUN_DIR/command.txt"

"${sink_command[@]}" > "$RUN_DIR/sink.stdout" 2> "$RUN_DIR/sink.stderr" &
sink_pid=$!
"${producer_command[@]}" > "$RUN_DIR/producer.stdout" 2> "$RUN_DIR/producer.stderr" &
producer_pid=$!

set +e
/usr/bin/time -v -o "$RUN_DIR/time.txt" -- \
  "${align_command[@]}" > "$RUN_DIR/align.stdout" 2> "$RUN_DIR/align.stderr"
align_status=$?
if (( align_status != 0 )); then
  kill "$producer_pid" "$sink_pid" 2>/dev/null || true
fi
wait "$producer_pid"
producer_status=$?
wait "$sink_pid"
sink_status=$?
set -e
printf '%s\n' "$align_status" > "$RUN_DIR/align.exit"
printf '%s\n' "$producer_status" > "$RUN_DIR/producer.exit"
printf '%s\n' "$sink_status" > "$RUN_DIR/sink.exit"

python3 "$SCRIPT_DIR/metrics.py" "$RUN_DIR/time.txt" \
  --outer-exit-code "$align_status" --output "$RUN_DIR/time.json" \
  > "$RUN_DIR/time.stdout"

if [[ "$(sha256_of "$INDEX_PATH")" != "$INDEX_SHA256" ]]; then
  fail "alignment modified or rebuilt the existing index"
fi
write_metadata finished_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

set +e
python3 "$SCRIPT_DIR/summarize_scale.py" \
  --metadata "$METADATA" --producer "$RUN_DIR/producer.json" \
  --sink "$RUN_DIR/sink.json" --time "$RUN_DIR/time.json" \
  --align-exit "$align_status" --producer-exit "$producer_status" \
  --sink-exit "$sink_status" --output "$RUN_DIR/summary.json" \
  > "$RUN_DIR/summary.stdout"
summary_status=$?
set -e

printf 'run_dir=%s\n' "$RUN_DIR"
(( summary_status == 0 )) || exit "$summary_status"
