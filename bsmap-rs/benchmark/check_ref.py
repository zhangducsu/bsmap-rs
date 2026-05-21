import sys
ref_file = "/mnt/c/Users/zhang_i5edc0/OneDrive/Documents/TraeSOLO/BSMAP/bsmap-rs/benchmark/data/chr22_tail_1M.fa"
with open(ref_file) as f:
    lines = f.readlines()
    seq = "".join(line.strip() for line in lines if not line.startswith(">"))
    print("chr22_tail_1M:", len(seq), "bp")
    print()

    # Read 446 region
    print("=== Ref at 772041-772115 (read 446) ===")
    r446 = seq[772041-1:772115].upper()
    print("ref :", r446)
    read446 = "A" + "T"*74
    print("read:", read446)
    ct = "".join("T" if c == "C" else c for c in r446)
    print("C>T:", ct)
    print("match:", ct == read446)
    print()

    # Read 58512 region
    print("=== Ref at 772128-772202 (read 58512) ===")
    r58512 = seq[772128-1:772202].upper()
    print("ref :", r58512)
    read58512 = "T"*66 + "G" + "T"*8
    print("read:", read58512)
    ct2 = "".join("T" if c == "C" else c for c in r58512)
    print("C>T:", ct2)
    print("match:", ct2 == read58512)
    print()

    # 16-mer at various positions
    print("=== 16-mers in ref region ===")
    for i in range(0, 70, 10):
        kmer = r446[i:i+16]
        ct_kmer = "".join("T" if c=="C" else c for c in kmer)
        print(f"  pos {i}: ref={kmer} -> C>T={ct_kmer}")
