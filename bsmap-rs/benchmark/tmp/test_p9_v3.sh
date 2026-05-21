#!/bin/bash
set -e

BIN="/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/target/release/bsmap"
OUT=/tmp/p9_debug_4reads

echo "=== Delete old .bsi ==="
rm -f /tmp/p9_ref.fa.bsi

echo "=== Run bsmap ==="
"$BIN" align -s 16 -v 0.08 -I 4 -p 1 -r 2 \
    -d /tmp/p9_ref.fa \
    -a "$OUT/all4.fastq" \
    -o "$OUT/all4_out_v3.sam" 2>/tmp/index_debug_stderr.txt

echo ""
echo "=== INDEX_DEBUG lines ==="
grep 'INDEX_DEBUG' /tmp/index_debug_stderr.txt || echo "(none)"

echo ""
echo "=== Other stderr ==="
grep -v 'INDEX_DEBUG' /tmp/index_debug_stderr.txt | head -10

echo ""
echo "=== SAM output ==="
cat "$OUT/all4_out_v3.sam"

echo ""
echo "=== .bsi header max_kmer_num ==="
python3 -c "
import struct
with open('/tmp/p9_ref.fa.bsi', 'rb') as f:
    header = f.read(256)
    magic = header[0:8]
    ver = struct.unpack_from('<I', header, 8)[0]
    max_kmer = struct.unpack_from('<I', header, 24)[0]
    total_kmers = struct.unpack_from('<Q', header, 40)[0]
    seed_size = struct.unpack_from('<I', header, 16)[0]
    interval = struct.unpack_from('<I', header, 20)[0]
    mode = struct.unpack_from('<I', header, 48)[0]
    print(f'magic={magic} ver={ver} max_kmer_num={max_kmer} total_kmers={total_kmers}')
    print(f'seed_size={seed_size} index_interval={interval} mode={mode}')
"
