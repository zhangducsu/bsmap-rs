#!/usr/bin/env python3
"""验证 read 446 在反向链位置的 mismatch 计数"""
import sys
sys.path.insert(0, "C:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark")

# 读取参考序列
ref_fa = "C:/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
with open(ref_fa) as f:
    header = f.readline().strip()
    ref_seq = f.read().replace('\n', '').upper()

print(f"Reference: {header}")
print(f"Reference length: {len(ref_seq)}")

# Read 446: ATTT...TTT (75 bases)
read = "ATTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT"
assert len(read) == 75, f"Read length: {len(read)}"

# Encoding: A=0, C=1, G=2, T=3
ENC = {'A': 0, 'C': 1, 'G': 2, 'T': 3}

# Forward reference at loc=772040 (0-indexed)
print(f"\n=== Forward chain hit (++) ===")
fwd_loc = 772040
ref_seg = ref_seq[fwd_loc:fwd_loc+75]
print(f"Ref at {fwd_loc}: {ref_seg}")

# Count mismatches with C->T tolerance
mm_fwd = 0
for i, (r, ref_b) in enumerate(zip(read, ref_seg)):
    r_code = ENC.get(r, 0)
    ref_code = ENC.get(ref_b, 0)
    if ref_code == 1:  # ref is C -> tolerance for T in read
        continue  # tolerated
    if r_code != ref_code:
        mm_fwd += 1
        print(f"  MM at pos {i}: read={r} ref={ref_b}")

print(f"Forward MM count: {mm_fwd}")

# Reverse complement reference at the -+ hit position
print(f"\n=== Reverse chain hit (-+) ===")
# Rust: loc = 156825, alignment_start = 855900 for reverse chain
# This means the RC reference at position 855900 should match the read
# The RC reference is built from the reverse complement of the forward

# For the RC hit, alignment_start=855900 means the read aligns to RC reference
# starting at base 855900.
# The RC reference is: reverse(forward) with each base complemented
# RC_ref[i] = complement(forward[len-forward-1-i])
# So for RC reference position i, the corresponding forward position is:
# forward_pos = len - 1 - i
# And the base at forward_pos, complemented, gives the RC base.

# SEGLEN = 32, so the RC reference is organized in 32-base words with padding
# For chr22_tail_1M:
# padded_len = (1000000+31)/32 + 2 = 31253 words * 32 = 1,000,096 bases
# RC reference has the same padded length

# But the actual chromosome data in the RC reference starts at REF_MARGIN (400 words)
# and goes for n words. Let me compute the actual RC base at position 855900.

# In the RC reference, the actual data starts at REF_MARGIN * SEGLEN = 400 * 32 = 12800 bases
# and the data consists of n_words * SEGLEN = 31253 * 32 = 1,000,096 bases.
# But only the first len (1,000,000) bases contain real data, the rest is padding.

# RC reference position 855900 is within the data region (12800 to 1012800).
# The actual offset from the start of data: 855900 - 12800 = 843100
# This corresponds to the RC of forward sequence, where:
# RC data starts at position 0 of the RC data = reverse(len-1) complemented

# So RC_data[i] = complement(forward_data[len-1-i]) for i < len
# And RC_data[i] = padding (T? A?) for i >= len

# Actually, the RC_data is built by encoding the reverse complement of the forward sequence
# into u64 words (32 bases each), using REV_ALPHABET encoding.
# REV_ALPHABET: A->3(T), C->2(G), G->1(C), T->0(A)

# So RC_data[i] = RevAlphabet[forward[len-1-i]]
# RC code 0 = A in the standard encoding, but it represents T in the original

# For position i=843100 in the RC data:
forward_pos = len(ref_seq) - 1 - 843100
print(f"RC data offset: 843100")
print(f"Corresponding forward position: {forward_pos}")

# The full read aligns at RC_data offset 843100
# RC codes at that position:
rc_codes = []
for i in range(75):
    rc_data_idx = 843100 + i
    if rc_data_idx < len(ref_seq):
        fwd_pos = len(ref_seq) - 1 - rc_data_idx
        fwd_base = ref_seq[fwd_pos]
        # RevAlphabet mapping: A->3(T), C->2(G), G->1(C), T->0(A)
        rev_map = {'A': 3, 'C': 2, 'G': 1, 'T': 0}
        rc_code = rev_map.get(fwd_base, 0)
        rc_codes.append(rc_code)
    else:
        rc_codes.append(0)  # padding

print(f"RC codes (first 20): {rc_codes[:20]}")
print(f"Read codes (all T=3, first A=0): {[ENC.get(r, 0) for r in read[:20]]}")

# Count mismatches with C->T tolerance on the RC reference
# In Rust's XC64: for ref code 01 (C), mask=01 (tolerance)
# For all other ref codes, mask=11 (no tolerance)
# Then: diff = (read_code ^ ref_code) & xc_mask
mm_rc = 0
for i in range(75):
    read_code = ENC.get(read[i], 0)
    rc_code = rc_codes[i]
    diff = read_code ^ rc_code

    # XC64 mask for C->T tolerance
    # XC64 formula: ((!tt) << 1) | tt | 0x5555...
    # For 2-bit codes: A(00)->11, C(01)->01, G(10)->11, T(11)->11
    xc_lookup = {0: 3, 1: 1, 2: 3, 3: 3}  # 2-bit xc64 mask per code
    xc_mask = xc_lookup[rc_code]

    diff &= xc_mask

    if diff != 0:
        mm_rc += 1
        rc_data_idx = 843100 + i
        if rc_data_idx < len(ref_seq):
            fwd_pos = len(ref_seq) - 1 - rc_data_idx
            fwd_base = ref_seq[fwd_pos]
        else:
            fwd_base = '?'
        print(f"  MM at pos {i}: read={read[i]}({read_code:02b}) rc_code={rc_code:02b} xc_mask={xc_mask:02b} diff={diff:02b} fwd_base={fwd_base}")

print(f"Reverse chain MM count: {mm_rc}")

# Also check: what does the forward reference look like at the corresponding position?
print(f"\n=== Forward reference at the forward position ===")
if forward_pos >= 0 and forward_pos + 75 <= len(ref_seq):
    fwd_seg = ref_seq[forward_pos:forward_pos+75]
    print(f"Forward seq at {forward_pos}: {fwd_seg}")
else:
    print(f"Forward position {forward_pos} out of range")

# Summary
print(f"\n=== Summary ===")
print(f"Forward hit: ++, loc=772040, MM={mm_fwd}")
print(f"Reverse hit: -+, loc=156825 (Rust), MM={mm_rc}")
print(f"C++ would compute loc=156921 for the -+ hit")
print(f"C++ would classify as {'unique' if (mm_fwd == 0 and mm_rc > 0) or (mm_fwd > 0 and mm_rc == 0) else 'depends on both'} if one MM>0")
