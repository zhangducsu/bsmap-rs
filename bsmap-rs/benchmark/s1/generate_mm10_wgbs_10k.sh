#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)}"
OUT_ROOT="${2:-/mnt/d/BSMAP/benchmark-data/mm10/wgbs_10k}"
MM10_FA="${MM10_FA:-/mnt/d/BSMAP/benchmark-data/mm10/mm10.fa}"
SHERMAN="${SHERMAN:-$REPO_ROOT/bsmap-rs/tools/sherman/Sherman}"

SE_READS="${SE_READS:-10000}"
PE_PAIRS="${PE_PAIRS:-10000}"
SE_LEN="${SE_LEN:-75}"
PE_LEN="${PE_LEN:-150}"
CONVERSION_RATE="${CONVERSION_RATE:-99.0}"
MIN_FRAG="${MIN_FRAG:-70}"
MAX_FRAG="${MAX_FRAG:-400}"

if [[ ! -f "$MM10_FA" ]]; then
  echo "missing mm10 reference: $MM10_FA" >&2
  exit 1
fi

if [[ ! -x "$SHERMAN" ]]; then
  echo "missing executable Sherman: $SHERMAN" >&2
  echo "Set SHERMAN=/path/to/Sherman if the submodule is expanded elsewhere." >&2
  exit 1
fi

mkdir -p "$OUT_ROOT"

GENOME_DIR="$OUT_ROOT/genome"
mkdir -p "$GENOME_DIR"
if [[ ! -e "$GENOME_DIR/mm10.fa" ]]; then
  ln -s "$MM10_FA" "$GENOME_DIR/mm10.fa"
fi

SE_DIR="$OUT_ROOT/se75_10k"
PE_DIR="$OUT_ROOT/pe150_10k"
mkdir -p "$SE_DIR" "$PE_DIR"

run_sherman_se() {
  if [[ -e "$SE_DIR/simulated.fastq.gz" || -e "$SE_DIR/simulated.fastq" ]]; then
    echo "SE output already exists, not overwriting: $SE_DIR" >&2
    return
  fi
  "$SHERMAN" \
    --genome_folder "$GENOME_DIR" \
    -l "$SE_LEN" \
    -n "$SE_READS" \
    -cr "$CONVERSION_RATE" \
    -o "$SE_DIR"
  gzip -n "$SE_DIR/simulated.fastq"
}

run_sherman_pe() {
  if [[ -e "$PE_DIR/simulated_1.fastq.gz" || -e "$PE_DIR/simulated_1.fastq" ]]; then
    echo "PE output already exists, not overwriting: $PE_DIR" >&2
    return
  fi
  "$SHERMAN" \
    --genome_folder "$GENOME_DIR" \
    -l "$PE_LEN" \
    -n "$PE_PAIRS" \
    -pe \
    -I "$MIN_FRAG" \
    -X "$MAX_FRAG" \
    -cr "$CONVERSION_RATE" \
    -o "$PE_DIR"
  gzip -n "$PE_DIR/simulated_1.fastq"
  gzip -n "$PE_DIR/simulated_2.fastq"
}

run_sherman_se
run_sherman_pe

SE_COUNT="$(gzip -cd "$SE_DIR/simulated.fastq.gz" | awk 'END { print NR / 4 }')"
PE_COUNT_1="$(gzip -cd "$PE_DIR/simulated_1.fastq.gz" | awk 'END { print NR / 4 }')"
PE_COUNT_2="$(gzip -cd "$PE_DIR/simulated_2.fastq.gz" | awk 'END { print NR / 4 }')"

if [[ "$SE_COUNT" != "$SE_READS" ]]; then
  echo "unexpected SE read count: $SE_COUNT != $SE_READS" >&2
  exit 1
fi
if [[ "$PE_COUNT_1" != "$PE_PAIRS" || "$PE_COUNT_2" != "$PE_PAIRS" ]]; then
  echo "unexpected PE read counts: R1=$PE_COUNT_1 R2=$PE_COUNT_2 expected=$PE_PAIRS" >&2
  exit 1
fi

METADATA="$OUT_ROOT/metadata.tsv"
{
  printf 'key\tvalue\n'
  printf 'created_utc\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'repo_head\t%s\n' "${GIT_COMMIT:-$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || true)}"
  printf 'sherman_path\t%s\n' "$SHERMAN"
  printf 'sherman_sha256\t%s\n' "$(sha256sum "$SHERMAN" | awk '{print $1}')"
  printf 'reference\t%s\n' "$MM10_FA"
  printf 'reference_sha256\t%s\n' "$(sha256sum "$MM10_FA" | awk '{print $1}')"
  printf 'conversion_rate\t%s\n' "$CONVERSION_RATE"
  printf 'se_reads\t%s\n' "$SE_READS"
  printf 'se_length\t%s\n' "$SE_LEN"
  printf 'pe_pairs\t%s\n' "$PE_PAIRS"
  printf 'pe_length\t%s\n' "$PE_LEN"
  printf 'pe_min_frag\t%s\n' "$MIN_FRAG"
  printf 'pe_max_frag\t%s\n' "$MAX_FRAG"
  printf 'se_fastq_gz\t%s\n' "$SE_DIR/simulated.fastq.gz"
  printf 'se_fastq_gz_sha256\t%s\n' "$(sha256sum "$SE_DIR/simulated.fastq.gz" | awk '{print $1}')"
  printf 'pe_r1_fastq_gz\t%s\n' "$PE_DIR/simulated_1.fastq.gz"
  printf 'pe_r1_fastq_gz_sha256\t%s\n' "$(sha256sum "$PE_DIR/simulated_1.fastq.gz" | awk '{print $1}')"
  printf 'pe_r2_fastq_gz\t%s\n' "$PE_DIR/simulated_2.fastq.gz"
  printf 'pe_r2_fastq_gz_sha256\t%s\n' "$(sha256sum "$PE_DIR/simulated_2.fastq.gz" | awk '{print $1}')"
} > "$METADATA"

cat "$METADATA"
