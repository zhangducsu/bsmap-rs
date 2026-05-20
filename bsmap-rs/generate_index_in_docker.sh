#!/bin/bash
set -e
cd /workspace

echo '[1/3] ???Rust???...'
apt-get update
apt-get install -y build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. /root/.cargo/env
rustup default stable

echo '[2/3] ???bsmap-rs...'
cd /workspace/bsmap
cargo build --release --features "rayon"

echo '[3/3] ???????????..'
cd /workspace/benchmark
../bsmap/target/release/bsmap index \
    -d data/chr22_tail_1M.fa \
    -s 16 \
    -I 4

echo ''
echo '=========================================='
echo '???????????
echo '??????: benchmark/data/chr22_tail_1M.fa.bsi'
echo '=========================================='
ls -lh data/
