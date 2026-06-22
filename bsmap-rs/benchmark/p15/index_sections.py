#!/usr/bin/env python3
"""Inspect and validate BSMAP v8-v10 raw index sections without loading them."""

import argparse
import json
import struct
from pathlib import Path


HEADER_SIZE = 256
SECTION_DIRECTORY_OFFSET = 100
SECTION_ENTRY_SIZE = 16
SECTION_COUNT = 9
RAW_SECTION_MARKER_OFFSET = 248

WGBS_SECTIONS = (
    ("wgbs_buckets", 8),
    ("positions", 4),
    ("wgbs_occupancy", 8),
    ("wgbs_rank", 4),
    ("wgbs_overflow", 12),
    ("unused_rrbs_site_offsets", 4),
    ("unused_rrbs_sites", 8),
    ("refcat", 8),
    ("crefcat", 8),
)

RRBS_SECTIONS = (
    ("unused_index2", 8),
    ("positions", 4),
    ("unused_start_offsets", 4),
    ("rrbs_offsets", 4),
    ("rrbs_hits", 8),
    ("rrbs_site_offsets", 4),
    ("rrbs_sites", 8),
    ("refcat", 8),
    ("crefcat", 8),
)

RRBS_V10_SECTIONS = (
    *RRBS_SECTIONS[:4],
    ("packed_rrbs_hits", 7),
    *RRBS_SECTIONS[5:],
)


def unpack(header, offset, fmt):
    return struct.unpack_from("<" + fmt, header, offset)[0]


def inspect_index(path):
    path = Path(path)
    file_size = path.stat().st_size
    with path.open("rb") as handle:
        header = handle.read(HEADER_SIZE)
    if len(header) != HEADER_SIZE:
        raise ValueError(f"索引头不足 {HEADER_SIZE} bytes")

    magic = header[:8]
    version = unpack(header, 8, "I")
    mode_code = unpack(header, 16, "I")
    mode = {0: "WGBS", 1: "RRBS"}.get(mode_code, f"UNKNOWN({mode_code})")
    marker = header[RAW_SECTION_MARKER_OFFSET:HEADER_SIZE].decode("ascii", errors="replace")
    errors = []
    if magic != b"BSMAPIDX":
        errors.append("magic 不匹配")
    if version not in (8, 9, 10):
        errors.append(f"只支持检查 v8-v10 raw index，实际为 v{version}")
    if marker != "RAWSECT2":
        errors.append(f"raw section marker 不是 RAWSECT2：{marker!r}")
    if mode_code not in (0, 1):
        errors.append(f"未知 mode：{mode_code}")
    elif mode_code == 0 and version != 8:
        errors.append(f"WGBS raw index 必须为 v8，实际为 v{version}")
    elif mode_code == 1 and version not in (8, 9, 10):
        errors.append(f"RRBS raw index 必须为 v8、v9 或 v10，实际为 v{version}")

    refcat_words = unpack(header, 48, "Q")
    crefcat_words = unpack(header, 56, "Q")
    if version in (9, 10) and crefcat_words != 0:
        errors.append(f"RRBS v{version} 必须省略 materialized crefcat")

    specs = RRBS_V10_SECTIONS if mode_code == 1 and version == 10 else RRBS_SECTIONS if mode_code == 1 else WGBS_SECTIONS
    sections = []
    previous_end = HEADER_SIZE + unpack(header, 44, "I")
    for index, (name, item_size) in enumerate(specs):
        entry_offset = SECTION_DIRECTORY_OFFSET + index * SECTION_ENTRY_SIZE
        offset, count = struct.unpack_from("<QQ", header, entry_offset)
        byte_length = count * item_size
        end = offset + byte_length
        section_errors = []
        if offset % 8 != 0:
            section_errors.append("offset 未按 8 bytes 对齐")
        if offset < previous_end:
            section_errors.append("section 与前一 section 重叠或逆序")
        if end > file_size:
            section_errors.append("section 越过文件末尾")
        errors.extend(f"{name}: {message}" for message in section_errors)
        previous_end = max(previous_end, end)
        sections.append(
            {
                "index": index,
                "name": name,
                "offset": offset,
                "item_count": count,
                "item_size": item_size,
                "byte_length": byte_length,
                "end": end,
                "errors": section_errors,
            }
        )

    if sections[7]["item_count"] != refcat_words:
        errors.append("refcat header word 数与 section item_count 不一致")
    if sections[8]["item_count"] != crefcat_words:
        errors.append("crefcat header word 数与 section item_count 不一致")

    return {
        "schema_version": 1,
        "path": str(path),
        "file_size_bytes": file_size,
        "magic": magic.decode("ascii", errors="replace"),
        "version": version,
        "seed_size": unpack(header, 12, "I"),
        "mode": mode,
        "total_kmers": unpack(header, 20, "I"),
        "max_kmer_num": unpack(header, 24, "I"),
        "index_interval": unpack(header, 28, "I"),
        "reference_count": unpack(header, 40, "I"),
        "reference_names_bytes": unpack(header, 44, "I"),
        "refcat_words": refcat_words,
        "crefcat_words": crefcat_words,
        "source_size_bytes": unpack(header, 64, "Q"),
        "marker": marker,
        "sections": sections,
        "total_section_bytes": sum(section["byte_length"] for section in sections),
        "valid": not errors,
        "errors": errors,
    }


def main():
    parser = argparse.ArgumentParser(description="检查 BSMAP v8-v10 索引 section 布局。")
    parser.add_argument("index")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    report = inspect_index(args.index)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
