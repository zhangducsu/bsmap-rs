#!/usr/bin/env python3
"""Check what positions hash 21523361 has in the Rust-built index."""

import struct
import sys

bsi_path = "/tmp/p9_ref.fa.bsi"

with open(bsi_path, "rb") as f:
    data = f.read()

# Parse header
magic = data[0:8]
ver = struct.unpack_from('<I', data, 8)[0]
max_kmer = struct.unpack_from('<I', data, 24)[0]
total_kmers = struct.unpack_from('<Q', data, 40)[0]
seed_size = struct.unpack_from('<I', data, 16)[0]
interval = struct.unpack_from('<I', data, 20)[0]
mode = struct.unpack_from('<I', data, 48)[0]
print(f"ver={ver} max_kmer_num={max_kmer} total_kmers={total_kmers} seed_size={seed_size} interval={interval} mode={mode}")

# Skip header + ref_names
ptr = 256
# Skip reference names (u16 length-prefixed strings)
num_refs = struct.unpack_from('<I', data, 52)[0]
for _ in range(num_refs):
    name_len = struct.unpack_from('<H', data, ptr)[0]
    ptr += 2 + name_len

# Now read index data: total_kmers, max_kmer_num, index2 (KmerLoc2 entries), positions, start_offsets
idx_total_kmers = struct.unpack_from('<I', data, ptr)[0]
ptr += 4
idx_max_kmer = struct.unpack_from('<I', data, ptr)[0]
ptr += 4

print(f"index total_kmers={idx_total_kmers} max_kmer={idx_max_kmer}")

# index2: for each of total_kmers entries: n[0]=u32, n[1]=u32, loc1_len=u64
# Read all KmerLoc2 entries
index2 = []
for i in range(idx_total_kmers):
    n0 = struct.unpack_from('<I', data, ptr)[0]
    ptr += 4
    n1 = struct.unpack_from('<I', data, ptr)[0]
    ptr += 4
    loc1_len = struct.unpack_from('<Q', data, ptr)[0]
    ptr += 8
    index2.append((n0, n1, loc1_len))

# Check hash 21523361
h = 21523361
n0, n1, loc1_len = index2[h]
print(f"\nhash={h}: n[0]={n0} n[1]={n1} loc1_len={loc1_len}")

# positions array
num_positions = struct.unpack_from('<Q', data, ptr)[0]
ptr += 8
print(f"\nnum_positions={num_positions}")

# start_offsets
start_offsets = []
for i in range(idx_total_kmers):
    off = struct.unpack_from('<I', data, ptr)[0]
    ptr += 4
    start_offsets.append(off)

# Now read positions
positions = []
for i in range(num_positions):
    pos = struct.unpack_from('<I', data, ptr)[0]
    ptr += 4
    positions.append(pos)

print(f"\nPositions for hash 21523361 (start_offset={start_offsets[h]}, n0+n1={n0+n1}):")
start = start_offsets[h]
end = start + n0 + n1
if end <= len(positions):
    fwd_positions = positions[start:start+n0]
    rev_positions = positions[start+n0:end]
    print(f"  Forward positions ({n0}): {fwd_positions[:20]}{'...' if n0 > 20 else ''}")
    print(f"  Reverse positions ({n1}): {rev_positions[:20]}{'...' if n1 > 20 else ''}")

    # The expected position for read 58512 on ++ strand is:
    # ref_anchor[0] = 400 * 32 = 12800
    # Binary position = 12800 + 772179 = 784979
    # hit2int(0, 784979) = 12800 + 784979 = 797779
    expected = 12800 + 12800 + 772179  # hit2int(0, margin+real_pos) = ref_anchor[0] + margin+real_pos
    print(f"\n  Expected flat_pos for correct hit: ~{expected}")

    # Find closest fwd positions
    fwd_sorted = sorted(fwd_positions)
    for fp in fwd_sorted[:10]:
        diff = fp as i64 - expected as i64
        loc = fp - 12800 - 12800  # real position
        print(f"    flat_pos={fp} (real_pos={loc}) diff={diff}")

    # Check if any position is within 100 of expected
    for fp in fwd_positions:
        diff = fp as i64 - expected as i64
        if abs(diff) < 1000:
            loc = fp - 12800 - 12800
            print(f"  NEARBY: flat_pos={fp} real_pos={loc} diff={diff}")
else:
    print(f"  Invalid range: {start}..{end} > {len(positions)}")
