//! K-mer index serialization, deserialization, and file I/O.
//!
//! Provides `IndexFile` for persisting a built `KmerIndex` to disk so that
//! subsequent runs can skip the expensive three-pass construction.
//!
//! File format (little-endian):
//! ```text
//! ┌─────────────────────────────────────────┐
//! │ Header (fixed 256 bytes)                │
//! │   magic:     [u8; 8]   "BSMAPIDX"      │
//! │   version:   u32        1               │
//! │   seed_size: u32                        │
//! │   mode:      u32        0=WGBS, 1=RRBS  │
//! │   total_kmers: u32                      │
//! │   max_kmer_num: u32                     │
//! │   index_interval: u32                   │
//! │   max_kmer_ratio: f64                   │
//! │   num_refs:  u32                        │
//! │   ref_names_len: u32                    │
//! │   reserved:  [u8; 220]                  │
//! ├─────────────────────────────────────────┤
//! │ Reference names (ref_names_len bytes)    │
//! │   each name: u16(len) + UTF-8 bytes     │
//! ├─────────────────────────────────────────┤
//! │ Index data (bincode-serialized)          │
//! │   KmerIndex (without ref_names)          │
//! └─────────────────────────────────────────┘
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};

use super::binseq::BinSeqCollection;
use super::index::{
    KmerIndex, MappedKmerIndex, MappedSection, PackedWgbsBucket, WgbsCountOverflow,
};
use super::storage::{MmapStorage, VecStorage};
use crate::param::{BINSEQPAD, REF_MARGIN, SEGLEN};

/// Magic bytes identifying a BSMAP-rs index file.
const INDEX_MAGIC: &[u8; 8] = b"BSMAPIDX";

/// Current index file format version.
const INDEX_VERSION: u32 = 1;

/// Version 2: includes refcat/crefcat data segments, supports mmap.
const INDEX_VERSION_V2: u32 = 2;

/// Version 3: version 2 layout with mode-aware RRBS hit encoding.
const INDEX_VERSION_RRBS_MODE_AWARE: u32 = 3;

/// 版本 4：reference metadata 同时保存染色体真实长度。
const INDEX_VERSION_CHR_LENGTHS: u32 = 4;

/// Version 5: compact RRBS offset table plus flat hit storage.
const INDEX_VERSION_RRBS_FLAT: u32 = 5;

/// Version 6: persists RRBS digestion sites for C++-compatible ZP/ZL tags.
const INDEX_VERSION_RRBS_SITES: u32 = 6;

/// Version 7: supports FASTA-stat and complete index-parameter compatibility checks.
const INDEX_VERSION_FAST_COMPAT: u32 = 7;

/// Version 8: succinct WGBS bucket metadata and raw-section layout marker v2.
const INDEX_VERSION_SUCCINCT_WGBS: u32 = 8;

/// WGBS alignment mode.
const MODE_WGBS: u32 = 0;

/// RRBS alignment mode.
const MODE_RRBS: u32 = 1;

/// Fixed header size in bytes.
const HEADER_SIZE: usize = 256;

const SOURCE_SIZE_OFFSET: usize = 64;
const SOURCE_MTIME_SECS_OFFSET: usize = 72;
const SOURCE_MTIME_NANOS_OFFSET: usize = 80;
const RRBS_MIN_INSERT_OFFSET: usize = 84;
const RRBS_MAX_INSERT_OFFSET: usize = 88;
const DIGEST_SITES_HASH_OFFSET: usize = 92;
const SECTION_DIRECTORY_OFFSET: usize = 100;
const SECTION_ENTRY_SIZE: usize = 16;
const SECTION_COUNT: usize = 9;
const RAW_SECTION_MARKER_OFFSET: usize = 248;
const RAW_SECTION_MARKER_V1: &[u8; 8] = b"RAWSECT1";
const RAW_SECTION_MARKER_V2: &[u8; 8] = b"RAWSECT2";

const SECTION_INDEX2: usize = 0;
const SECTION_POSITIONS: usize = 1;
const SECTION_START_OFFSETS: usize = 2;
const SECTION_RRBS_OFFSETS: usize = 3;
const SECTION_RRBS_HITS: usize = 4;
const SECTION_RRBS_SITE_OFFSETS: usize = 5;
const SECTION_RRBS_SITES: usize = 6;
const SECTION_REFCAT: usize = 7;
const SECTION_CREFCAT: usize = 8;

#[derive(Debug, Clone, Copy, Default)]
struct RawSection {
    offset: u64,
    len: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexParameters<'a> {
    pub seed_size: u32,
    pub index_interval: u32,
    pub max_kmer_ratio: f64,
    pub is_rrbs: bool,
    pub min_insert: u32,
    pub max_insert: u32,
    pub digest_sites: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceFingerprint {
    size: u64,
    mtime_secs: u64,
    mtime_nanos: u32,
}

impl SourceFingerprint {
    fn from_path(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("Cannot stat reference FASTA: {}", path.display()))?;
        let modified = metadata
            .modified()
            .with_context(|| format!("Cannot read reference FASTA mtime: {}", path.display()))?
            .duration_since(UNIX_EPOCH)
            .with_context(|| format!("Reference FASTA mtime predates Unix epoch: {}", path.display()))?;
        Ok(Self {
            size: metadata.len(),
            mtime_secs: modified.as_secs(),
            mtime_nanos: modified.subsec_nanos(),
        })
    }
}

fn digest_sites_hash(sites: &[String]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for site in sites {
        for byte in (site.len() as u64).to_le_bytes().iter().chain(site.as_bytes()) {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

/// Bincode configuration: little-endian, variable-length integers.
fn bincode_opts() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
}

// ── Serializable Index (without reference names) ─────────────────────────────

/// Serializable representation of the k-mer index.
///
/// Separated from `KmerIndex` to allow versioned format evolution.
/// For WGBS mode, `index2[i].n[0]` = reverse chain count, `index2[i].n[1]` = forward chain count.
/// The flat `positions` array layout is: [fwd_hits(hash0) | rev_hits(hash0) | fwd_hits(hash1) | rev_hits(hash1) | ...]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IndexDataV4 {
    total_kmers: u32,
    max_kmer_num: u32,
    /// WGBS index entries.
    index2: Vec<IndexKmerLoc2>,
    /// Flat position array.
    positions: Vec<u32>,
    /// Precomputed start offsets for O(1) lookup_separated.
    start_offsets: Vec<u32>,
    /// RRBS index entries (empty if WGBS mode).
    rrbs_index: Option<Vec<IndexKmerLoc>>,
}

#[derive(Deserialize)]
struct IndexDataV5 {
    total_kmers: u32,
    max_kmer_num: u32,
    index2: Vec<IndexKmerLoc2>,
    positions: Vec<u32>,
    start_offsets: Vec<u32>,
    rrbs_offsets: Vec<u32>,
    rrbs_hits: Vec<crate::param::Hit>,
}

#[allow(dead_code)]
#[derive(Serialize)]
struct IndexDataV6Ref<'a> {
    total_kmers: u32,
    max_kmer_num: u32,
    index2: Vec<IndexKmerLoc2>,
    positions: &'a [u32],
    start_offsets: &'a [u32],
    rrbs_offsets: &'a [u32],
    rrbs_hits: &'a [crate::param::Hit],
    rrbs_site_offsets: &'a [u32],
    rrbs_sites: Vec<(u32, u32)>,
}

#[derive(Deserialize)]
struct IndexDataV6 {
    total_kmers: u32,
    max_kmer_num: u32,
    index2: Vec<IndexKmerLoc2>,
    positions: Vec<u32>,
    start_offsets: Vec<u32>,
    rrbs_offsets: Vec<u32>,
    rrbs_hits: Vec<crate::param::Hit>,
    rrbs_site_offsets: Vec<u32>,
    rrbs_sites: Vec<(u32, u32)>,
}

/// Serializable WGBS index entry.
///
/// `n[0]` = reverse chain hit count, `n[1]` = forward chain hit count.
/// Matches the C++ `KmerLoc2` layout where positions are stored as:
///   [forward_chain_hits... | reverse_chain_hits...]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IndexKmerLoc2 {
    n: [u32; 2],
}

/// Serializable RRBS index entry.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IndexKmerLoc {
    n1: u32,
    loc1: Vec<IndexHit>,
}

/// Serializable hit.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct IndexHit {
    chr: u32,
    loc: u32,
}

// ── Conversion ───────────────────────────────────────────────────────────────

impl<'a> From<&'a KmerIndex> for IndexDataV6Ref<'a> {
    fn from(idx: &'a KmerIndex) -> Self {
        Self {
            total_kmers: idx.total_kmers,
            max_kmer_num: idx.max_kmer_num,
            index2: idx.index2.iter().map(|e| IndexKmerLoc2 { n: e.n }).collect(),
            positions: &idx.positions,
            start_offsets: &idx.start_offsets,
            rrbs_offsets: &idx.rrbs_offsets,
            rrbs_hits: &idx.rrbs_hits,
            rrbs_site_offsets: idx.rrbs_site_offsets_slice(),
            rrbs_sites: idx
                .rrbs_sites_slice()
                .iter()
                .map(|site| (site[0], site[1]))
                .collect(),
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Save a k-mer index to disk.
///
/// The index is tagged with the reference names and build parameters so that
/// subsequent loads can verify compatibility before deserializing.
pub fn save_index(
    path: &Path,
    index: &KmerIndex,
    seed_size: u32,
    index_interval: u32,
    max_kmer_ratio: f64,
    ref_names: &[String],
    is_rrbs: bool,
) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("Cannot create index file: {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    // ── Write header ─────────────────────────────────────────────────────
    let mut header = [0u8; HEADER_SIZE];
    header[0..8].copy_from_slice(INDEX_MAGIC);
    header[8..12].copy_from_slice(&INDEX_VERSION.to_le_bytes());
    header[12..16].copy_from_slice(&seed_size.to_le_bytes());
    let mode = if is_rrbs { MODE_RRBS } else { MODE_WGBS };
    header[16..20].copy_from_slice(&mode.to_le_bytes());
    header[20..24].copy_from_slice(&index.total_kmers.to_le_bytes());
    header[24..28].copy_from_slice(&index.max_kmer_num.to_le_bytes());
    header[28..32].copy_from_slice(&index_interval.to_le_bytes());
    header[32..40].copy_from_slice(&max_kmer_ratio.to_le_bytes());
    header[40..44].copy_from_slice(&(ref_names.len() as u32).to_le_bytes());

    // Serialize reference names
    let mut names_buf: Vec<u8> = Vec::new();
    for name in ref_names {
        let name_bytes = name.as_bytes();
        let len = name_bytes.len() as u16;
        names_buf.extend_from_slice(&len.to_le_bytes());
        names_buf.extend_from_slice(name_bytes);
    }
    header[44..48].copy_from_slice(&(names_buf.len() as u32).to_le_bytes());

    writer
        .write_all(&header)
        .context("Failed to write index header")?;
    writer
        .write_all(&names_buf)
        .context("Failed to write reference names")?;

    // ── Write index data ─────────────────────────────────────────────────
    let data = IndexDataV4::from_flat(index);
    bincode_opts()
        .serialize_into(&mut writer, &data)
        .context("Failed to serialize index data")?;

    writer.flush().context("Failed to flush index file")?;
    log::info!(
        "索引已保存到 {} ({} bytes header + {} names + data)",
        path.display(),
        HEADER_SIZE,
        names_buf.len(),
    );
    Ok(())
}

/// Save index in version 2 format (includes refcat/crefcat data segments).
pub fn save_index_v2(
    path: &Path,
    index: &KmerIndex,
    coll: &BinSeqCollection,
    ref_names: &[String],
    reference_path: &Path,
    params: &IndexParameters<'_>,
) -> Result<()> {
    if coll.chr_lengths.len() != ref_names.len() {
        bail!(
            "Reference name/length count mismatch: {} names, {} lengths",
            ref_names.len(),
            coll.chr_lengths.len(),
        );
    }
    let file = File::create(path)
        .with_context(|| format!("Cannot create index file: {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    // ── 当前完整索引格式的 header ──
    let mut header = [0u8; HEADER_SIZE];
    header[0..8].copy_from_slice(INDEX_MAGIC);
    let version = INDEX_VERSION_SUCCINCT_WGBS;
    header[8..12].copy_from_slice(&version.to_le_bytes());
    header[12..16].copy_from_slice(&params.seed_size.to_le_bytes());
    let mode = if params.is_rrbs { MODE_RRBS } else { MODE_WGBS };
    header[16..20].copy_from_slice(&mode.to_le_bytes());
    header[20..24].copy_from_slice(&index.total_kmers.to_le_bytes());
    header[24..28].copy_from_slice(&index.max_kmer_num.to_le_bytes());
    header[28..32].copy_from_slice(&params.index_interval.to_le_bytes());
    header[32..40].copy_from_slice(&params.max_kmer_ratio.to_le_bytes());
    header[40..44].copy_from_slice(&(ref_names.len() as u32).to_le_bytes());

    // reference metadata：每个名称后紧跟染色体真实长度。
    let mut names_buf: Vec<u8> = Vec::new();
    for (name, &chr_len) in ref_names.iter().zip(&coll.chr_lengths) {
        let name_bytes = name.as_bytes();
        names_buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        names_buf.extend_from_slice(name_bytes);
        names_buf.extend_from_slice(&chr_len.to_le_bytes());
    }
    header[44..48].copy_from_slice(&(names_buf.len() as u32).to_le_bytes());

    // 完整索引字段：refcat/crefcat word 数量。
    let refcat_slice = coll.refcat.as_slice();
    let crefcat_slice = coll.crefcat.as_slice();
    header[48..56].copy_from_slice(&(refcat_slice.len() as u64).to_le_bytes());
    header[56..64].copy_from_slice(&(crefcat_slice.len() as u64).to_le_bytes());

    let source = SourceFingerprint::from_path(reference_path)?;
    header[SOURCE_SIZE_OFFSET..SOURCE_SIZE_OFFSET + 8]
        .copy_from_slice(&source.size.to_le_bytes());
    header[SOURCE_MTIME_SECS_OFFSET..SOURCE_MTIME_SECS_OFFSET + 8]
        .copy_from_slice(&source.mtime_secs.to_le_bytes());
    header[SOURCE_MTIME_NANOS_OFFSET..SOURCE_MTIME_NANOS_OFFSET + 4]
        .copy_from_slice(&source.mtime_nanos.to_le_bytes());
    if params.is_rrbs {
        header[RRBS_MIN_INSERT_OFFSET..RRBS_MIN_INSERT_OFFSET + 4]
            .copy_from_slice(&params.min_insert.to_le_bytes());
        header[RRBS_MAX_INSERT_OFFSET..RRBS_MAX_INSERT_OFFSET + 4]
            .copy_from_slice(&params.max_insert.to_le_bytes());
        header[DIGEST_SITES_HASH_OFFSET..DIGEST_SITES_HASH_OFFSET + 8]
            .copy_from_slice(&digest_sites_hash(params.digest_sites).to_le_bytes());
    }

    if !cfg!(target_endian = "little") {
        bail!("v7 raw index format currently requires a little-endian target");
    }

    let positions = index.positions_slice();
    let lengths = if params.is_rrbs {
        [
            (0, std::mem::size_of::<crate::param::KmerLoc2>()),
            (positions.len(), std::mem::size_of::<u32>()),
            (0, std::mem::size_of::<u32>()),
            (index.rrbs_offsets_slice().len(), std::mem::size_of::<u32>()),
            (index.rrbs_hits_slice().len(), std::mem::size_of::<crate::param::Hit>()),
            (index.rrbs_site_offsets_slice().len(), std::mem::size_of::<u32>()),
            (index.rrbs_sites_slice().len(), std::mem::size_of::<[u32; 2]>()),
            (refcat_slice.len(), std::mem::size_of::<u64>()),
            (crefcat_slice.len(), std::mem::size_of::<u64>()),
        ]
    } else {
        [
            (index.wgbs_buckets_slice().len(), std::mem::size_of::<PackedWgbsBucket>()),
            (positions.len(), std::mem::size_of::<u32>()),
            (index.wgbs_occupancy_slice().len(), std::mem::size_of::<u64>()),
            (index.wgbs_rank_slice().len(), std::mem::size_of::<u32>()),
            (index.wgbs_overflow_slice().len(), std::mem::size_of::<WgbsCountOverflow>()),
            (0, std::mem::size_of::<u32>()),
            (0, std::mem::size_of::<[u32; 2]>()),
            (refcat_slice.len(), std::mem::size_of::<u64>()),
            (crefcat_slice.len(), std::mem::size_of::<u64>()),
        ]
    };
    let sections = raw_section_layout(names_buf.len(), &lengths)?;
    debug_assert_eq!(sections.len(), SECTION_COUNT);
    for (section_index, &section) in sections.iter().enumerate() {
        write_section_entry(&mut header, section_index, section);
    }
    header[RAW_SECTION_MARKER_OFFSET..RAW_SECTION_MARKER_OFFSET + RAW_SECTION_MARKER_V2.len()]
        .copy_from_slice(RAW_SECTION_MARKER_V2);

    writer.write_all(&header).context("Failed to write index header")?;
    writer.write_all(&names_buf).context("Failed to write reference names")?;
    let mut current = (HEADER_SIZE + names_buf.len()) as u64;

    let mut write_section = |section_index: usize, write: &mut dyn FnMut(&mut BufWriter<File>) -> Result<()>| -> Result<()> {
        write_padding(&mut writer, &mut current, sections[section_index].offset)?;
        write(&mut writer)?;
        current = current
            .checked_add(
                sections[section_index]
                    .len
                    .checked_mul(lengths[section_index].1 as u64)
                    .context("Index section size overflow")?,
            )
            .context("Index file size overflow")?;
        Ok(())
    };

    if params.is_rrbs {
        write_section(SECTION_INDEX2, &mut |_writer| Ok(()))?;
        write_section(SECTION_POSITIONS, &mut |writer| write_raw_slice(writer, positions))?;
        write_section(SECTION_START_OFFSETS, &mut |_writer| Ok(()))?;
        write_section(SECTION_RRBS_OFFSETS, &mut |writer| {
            write_raw_slice(writer, index.rrbs_offsets_slice())
        })?;
        write_section(SECTION_RRBS_HITS, &mut |writer| {
            write_raw_slice(writer, index.rrbs_hits_slice())
        })?;
        write_section(SECTION_RRBS_SITE_OFFSETS, &mut |writer| {
            write_raw_slice(writer, index.rrbs_site_offsets_slice())
        })?;
        write_section(SECTION_RRBS_SITES, &mut |writer| {
            write_rrbs_sites(writer, index.rrbs_sites_slice())
        })?;
    } else {
        write_section(SECTION_INDEX2, &mut |writer| {
            write_raw_slice(writer, index.wgbs_buckets_slice())
        })?;
        write_section(SECTION_POSITIONS, &mut |writer| write_raw_slice(writer, positions))?;
        write_section(SECTION_START_OFFSETS, &mut |writer| {
            write_raw_slice(writer, index.wgbs_occupancy_slice())
        })?;
        write_section(SECTION_RRBS_OFFSETS, &mut |writer| {
            write_raw_slice(writer, index.wgbs_rank_slice())
        })?;
        write_section(SECTION_RRBS_HITS, &mut |writer| {
            write_raw_slice(writer, index.wgbs_overflow_slice())
        })?;
        write_section(SECTION_RRBS_SITE_OFFSETS, &mut |_writer| Ok(()))?;
        write_section(SECTION_RRBS_SITES, &mut |_writer| Ok(()))?;
    }
    write_section(SECTION_REFCAT, &mut |writer| write_raw_slice(writer, refcat_slice))?;
    write_section(SECTION_CREFCAT, &mut |writer| write_raw_slice(writer, crefcat_slice))?;

    writer.flush().context("Failed to flush index file")?;
    log::info!(
        "索引已保存到 {} (v{}, refcat={} words, crefcat={} words)",
        path.display(),
        version,
        refcat_slice.len(),
        crefcat_slice.len(),
    );
    Ok(())
}

/// Metadata read from an index file header.
#[derive(Debug, Clone)]
pub struct IndexMeta {
    pub version: u32,
    pub seed_size: u32,
    pub is_rrbs: bool,
    pub total_kmers: u32,
    pub max_kmer_num: u32,
    pub index_interval: u32,
    pub max_kmer_ratio: f64,
    pub ref_names: Vec<String>,
    pub ref_lengths: Vec<u32>,
    pub source_size: u64,
    pub source_mtime_secs: u64,
    pub source_mtime_nanos: u32,
    pub rrbs_min_insert: u32,
    pub rrbs_max_insert: u32,
    pub digest_sites_hash: u64,
}

fn align_up(value: u64, alignment: u64) -> Result<u64> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .context("Index file offset overflow")
}

fn write_section_entry(header: &mut [u8; HEADER_SIZE], index: usize, section: RawSection) {
    let offset = SECTION_DIRECTORY_OFFSET + index * SECTION_ENTRY_SIZE;
    header[offset..offset + 8].copy_from_slice(&section.offset.to_le_bytes());
    header[offset + 8..offset + 16].copy_from_slice(&section.len.to_le_bytes());
}

fn read_section_entry(header: &[u8; HEADER_SIZE], index: usize) -> RawSection {
    let offset = SECTION_DIRECTORY_OFFSET + index * SECTION_ENTRY_SIZE;
    RawSection {
        offset: u64::from_le_bytes(header[offset..offset + 8].try_into().unwrap()),
        len: u64::from_le_bytes(header[offset + 8..offset + 16].try_into().unwrap()),
    }
}

fn raw_section_layout(names_len: usize, lengths: &[(usize, usize)]) -> Result<Vec<RawSection>> {
    let mut cursor = (HEADER_SIZE + names_len) as u64;
    let mut sections = Vec::with_capacity(lengths.len());
    for &(len, item_size) in lengths {
        cursor = align_up(cursor, 8)?;
        let byte_len = (len as u64)
            .checked_mul(item_size as u64)
            .context("Index section size overflow")?;
        sections.push(RawSection {
            offset: cursor,
            len: len as u64,
        });
        cursor = cursor
            .checked_add(byte_len)
            .context("Index file size overflow")?;
    }
    Ok(sections)
}

fn write_padding<W: Write>(writer: &mut W, current: &mut u64, target: u64) -> Result<()> {
    if target < *current {
        bail!("Index section offsets are not monotonic");
    }
    let mut remaining = (target - *current) as usize;
    const ZEROES: [u8; 64] = [0; 64];
    while remaining > 0 {
        let chunk = remaining.min(ZEROES.len());
        writer.write_all(&ZEROES[..chunk])?;
        remaining -= chunk;
    }
    *current = target;
    Ok(())
}

fn write_raw_slice<W: Write, T: Copy>(writer: &mut W, values: &[T]) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    let bytes = unsafe {
        std::slice::from_raw_parts(
            values.as_ptr() as *const u8,
            std::mem::size_of_val(values),
        )
    };
    writer.write_all(bytes)?;
    Ok(())
}

fn write_rrbs_sites<W: Write>(writer: &mut W, sites: &[[u32; 2]]) -> Result<()> {
    let mut buffer = Vec::with_capacity(64 * 1024);
    for &[position, reverse_offset] in sites {
        buffer.extend_from_slice(&position.to_le_bytes());
        buffer.extend_from_slice(&reverse_offset.to_le_bytes());
        if buffer.len() >= 64 * 1024 {
            writer.write_all(&buffer)?;
            buffer.clear();
        }
    }
    writer.write_all(&buffer)?;
    Ok(())
}

/// Load index metadata from a file without deserializing the full index.
///
/// Useful for checking whether the cached index is compatible with the
/// current reference and parameters.
pub fn read_index_meta(path: &Path) -> Result<IndexMeta> {
    let file = File::open(path)
        .with_context(|| format!("Cannot open index file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let mut header = [0u8; HEADER_SIZE];
    reader
        .read_exact(&mut header)
        .context("Failed to read index header")?;

    // Verify magic
    if &header[0..8] != INDEX_MAGIC {
        bail!(
            "Not a valid BSMAP-rs index file: {} (bad magic)",
            path.display()
        );
    }

    // Verify version
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
    if version != INDEX_VERSION
        && version != INDEX_VERSION_V2
        && version != INDEX_VERSION_RRBS_MODE_AWARE
        && version != INDEX_VERSION_CHR_LENGTHS
        && version != INDEX_VERSION_RRBS_FLAT
        && version != INDEX_VERSION_RRBS_SITES
        && version != INDEX_VERSION_FAST_COMPAT
        && version != INDEX_VERSION_SUCCINCT_WGBS
    {
        bail!(
            "Unsupported index version {} (expected {}, {}, {}, {}, {}, {}, {}, or {}): {}",
            version,
            INDEX_VERSION,
            INDEX_VERSION_V2,
            INDEX_VERSION_RRBS_MODE_AWARE,
            INDEX_VERSION_CHR_LENGTHS,
            INDEX_VERSION_RRBS_FLAT,
            INDEX_VERSION_RRBS_SITES,
            INDEX_VERSION_FAST_COMPAT,
            INDEX_VERSION_SUCCINCT_WGBS,
            path.display()
        );
    }

    let seed_size = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let mode = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let total_kmers = u32::from_le_bytes(header[20..24].try_into().unwrap());
    let max_kmer_num = u32::from_le_bytes(header[24..28].try_into().unwrap());
    let index_interval = u32::from_le_bytes(header[28..32].try_into().unwrap());
    let max_kmer_ratio = f64::from_le_bytes(header[32..40].try_into().unwrap());
    let num_refs = u32::from_le_bytes(header[40..44].try_into().unwrap());
    let names_len = u32::from_le_bytes(header[44..48].try_into().unwrap()) as usize;
    let source_size = u64::from_le_bytes(
        header[SOURCE_SIZE_OFFSET..SOURCE_SIZE_OFFSET + 8]
            .try_into()
            .unwrap(),
    );
    let source_mtime_secs = u64::from_le_bytes(
        header[SOURCE_MTIME_SECS_OFFSET..SOURCE_MTIME_SECS_OFFSET + 8]
            .try_into()
            .unwrap(),
    );
    let source_mtime_nanos = u32::from_le_bytes(
        header[SOURCE_MTIME_NANOS_OFFSET..SOURCE_MTIME_NANOS_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    let rrbs_min_insert = u32::from_le_bytes(
        header[RRBS_MIN_INSERT_OFFSET..RRBS_MIN_INSERT_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    let rrbs_max_insert = u32::from_le_bytes(
        header[RRBS_MAX_INSERT_OFFSET..RRBS_MAX_INSERT_OFFSET + 4]
            .try_into()
            .unwrap(),
    );
    let digest_sites_hash = u64::from_le_bytes(
        header[DIGEST_SITES_HASH_OFFSET..DIGEST_SITES_HASH_OFFSET + 8]
            .try_into()
            .unwrap(),
    );

    // Read reference names
    let mut names_buf = vec![0u8; names_len];
    reader
        .read_exact(&mut names_buf)
        .context("Failed to read reference names")?;

    let mut ref_names = Vec::with_capacity(num_refs as usize);
    let mut ref_lengths = Vec::with_capacity(num_refs as usize);
    let mut offset = 0;
    while offset < names_buf.len() && ref_names.len() < num_refs as usize {
        if offset + 2 > names_buf.len() {
            break;
        }
        let len = u16::from_le_bytes([names_buf[offset], names_buf[offset + 1]]) as usize;
        offset += 2;
        if offset + len > names_buf.len() {
            break;
        }
        let name = String::from_utf8_lossy(&names_buf[offset..offset + len]).to_string();
        ref_names.push(name);
        offset += len;
        if version >= INDEX_VERSION_CHR_LENGTHS {
            if offset + 4 > names_buf.len() {
                bail!("Truncated reference length metadata: {}", path.display());
            }
            ref_lengths.push(u32::from_le_bytes(
                names_buf[offset..offset + 4].try_into().unwrap(),
            ));
            offset += 4;
        }
    }
    if version >= INDEX_VERSION_CHR_LENGTHS
        && (ref_names.len() != num_refs as usize || ref_lengths.len() != num_refs as usize)
    {
        bail!("Incomplete reference metadata: {}", path.display());
    }

    Ok(IndexMeta {
        version,
        seed_size,
        is_rrbs: mode == MODE_RRBS,
        total_kmers,
        max_kmer_num,
        index_interval,
        max_kmer_ratio,
        ref_names,
        ref_lengths,
        source_size,
        source_mtime_secs,
        source_mtime_nanos,
        rrbs_min_insert,
        rrbs_max_insert,
        digest_sites_hash,
    })
}

impl IndexDataV4 {
    fn from_flat(idx: &KmerIndex) -> Self {
        let rrbs_offsets = idx.rrbs_offsets_slice();
        let rrbs_hits = idx.rrbs_hits_slice();
        let rrbs_index = if rrbs_offsets.is_empty() {
            None
        } else {
            Some(
                rrbs_offsets
                    .windows(2)
                    .map(|range| {
                        let start = range[0] as usize;
                        let end = range[1] as usize;
                        IndexKmerLoc {
                            n1: (end - start) as u32,
                            loc1: rrbs_hits[start..end]
                                .iter()
                                .map(|hit| IndexHit {
                                    chr: hit.chr,
                                    loc: hit.loc,
                                })
                                .collect(),
                        }
                    })
                    .collect(),
            )
        };
        let (index2, start_offsets) = if idx.wgbs_occupancy_slice().is_empty() {
            (
                idx.index2
                    .iter()
                    .map(|entry| IndexKmerLoc2 { n: entry.n })
                    .collect(),
                idx.start_offsets.clone(),
            )
        } else {
            let mut index2 = Vec::with_capacity(idx.total_kmers as usize);
            let mut start_offsets = Vec::with_capacity(idx.total_kmers as usize);
            for hash in 0..idx.total_kmers {
                if let Some((bucket, fwd, rev)) = idx.compact_bucket(hash) {
                    index2.push(IndexKmerLoc2 { n: [rev, fwd] });
                    start_offsets.push(bucket.offset);
                } else {
                    index2.push(IndexKmerLoc2 { n: [0, 0] });
                    start_offsets.push(0);
                }
            }
            (index2, start_offsets)
        };
        Self {
            total_kmers: idx.total_kmers,
            max_kmer_num: idx.max_kmer_num,
            index2,
            positions: idx.positions_slice().to_vec(),
            start_offsets,
            rrbs_index,
        }
    }
}

/// Load a full k-mer index from disk.
///
/// Returns the reconstructed `KmerIndex` and its metadata.
pub fn load_index(path: &Path) -> Result<(KmerIndex, IndexMeta)> {
    let meta = read_index_meta(path)?;
    if meta.version == INDEX_VERSION_FAST_COMPAT
        || meta.version == INDEX_VERSION_SUCCINCT_WGBS
    {
        let (_coll, index, meta) = load_index_with_mode(path, LoadMode::Memory)?;
        return Ok((index, meta));
    }

    let file = File::open(path)
        .with_context(|| format!("Cannot open index file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    // Skip header + names
    let stored_names_len = {
        let mut h = [0u8; HEADER_SIZE];
        reader.read_exact(&mut h)?;
        u32::from_le_bytes(h[44..48].try_into().unwrap()) as usize
    };

    // Skip names
    let mut names_skip = vec![0u8; stored_names_len];
    if stored_names_len > 0 {
        reader.read_exact(&mut names_skip)?;
    }

    let index = deserialize_kmer_index(&mut reader, meta.version, meta.seed_size)?;

    log::info!(
        "索引已从 {} 加载 ({} k-mers, mode={})",
        path.display(),
        index.total_kmers,
        if meta.is_rrbs { "RRBS" } else { "WGBS" },
    );

    Ok((index, meta))
}

/// Index loading mode.
#[derive(Debug, Clone, Copy)]
pub enum LoadMode {
    /// Load everything into heap memory.
    Memory,
    /// mmap reference sequence data segments (version 2 format only).
    Mmap,
}

fn has_raw_section_marker(header: &[u8; HEADER_SIZE], marker: &[u8; 8]) -> bool {
    &header[RAW_SECTION_MARKER_OFFSET..RAW_SECTION_MARKER_OFFSET + marker.len()] == marker
}

fn checked_mapped_section<T>(section: RawSection, file_size: u64) -> Result<MappedSection> {
    let offset = usize::try_from(section.offset).context("Index section offset exceeds usize")?;
    let len = usize::try_from(section.len).context("Index section length exceeds usize")?;
    if offset % std::mem::align_of::<T>() != 0 {
        bail!("Index section at byte {} is not aligned", offset);
    }
    let bytes = section
        .len
        .checked_mul(std::mem::size_of::<T>() as u64)
        .context("Index section byte length overflow")?;
    let end = section
        .offset
        .checked_add(bytes)
        .context("Index section end overflow")?;
    if end > file_size {
        bail!("Index section extends beyond end of file");
    }
    Ok(MappedSection { offset, len })
}

fn read_raw_vec<R: Read + Seek, T: Copy + Default>(
    reader: &mut R,
    section: RawSection,
) -> Result<Vec<T>> {
    let len = usize::try_from(section.len).context("Index section length exceeds usize")?;
    let mut values = vec![T::default(); len];
    reader.seek(std::io::SeekFrom::Start(section.offset))?;
    if !values.is_empty() {
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                values.as_mut_ptr() as *mut u8,
                std::mem::size_of_val(values.as_slice()),
            )
        };
        reader.read_exact(bytes)?;
    }
    Ok(values)
}

fn make_loaded_collection(
    meta: &IndexMeta,
    ref_anchor: Vec<u32>,
    refcat: Box<dyn super::storage::BinSeqStorage>,
    crefcat: Box<dyn super::storage::BinSeqStorage>,
) -> BinSeqCollection {
    BinSeqCollection {
        total_num: meta.ref_names.len() as u32 * 2,
        sum_length: meta.ref_lengths.iter().map(|&len| len as u64).sum(),
        refcat,
        crefcat,
        ref_anchor,
        chr_lengths: meta.ref_lengths.clone(),
        blocks: vec![],
        seqs: vec![],
        chr_names: meta.ref_names.clone(),
        chr_accessions: meta
            .ref_names
            .iter()
            .map(|name| name.split_whitespace().next().unwrap_or(name).to_string())
            .collect(),
    }
}

fn load_raw_index(
    path: &Path,
    mode: LoadMode,
    meta: IndexMeta,
    ref_anchor: Vec<u32>,
    header: &[u8; HEADER_SIZE],
) -> Result<(BinSeqCollection, KmerIndex, IndexMeta)> {
    let expected_marker = if meta.version == INDEX_VERSION_SUCCINCT_WGBS {
        RAW_SECTION_MARKER_V2
    } else {
        RAW_SECTION_MARKER_V1
    };
    if !has_raw_section_marker(header, expected_marker) {
        bail!(
            "v{} index uses an obsolete or unknown raw-section layout; rebuild it with `bsmap index`",
            meta.version
        );
    }
    if !cfg!(target_endian = "little") {
        bail!("raw index format currently requires a little-endian target");
    }

    let sections: [RawSection; SECTION_COUNT] =
        std::array::from_fn(|index| read_section_entry(header, index));
    let file_size = std::fs::metadata(path)?.len();
    let positions = checked_mapped_section::<u32>(sections[SECTION_POSITIONS], file_size)?;
    let refcat = checked_mapped_section::<u64>(sections[SECTION_REFCAT], file_size)?;
    let crefcat = checked_mapped_section::<u64>(sections[SECTION_CREFCAT], file_size)?;
    let succinct_wgbs = meta.version == INDEX_VERSION_SUCCINCT_WGBS && !meta.is_rrbs;

    let (index2, start_offsets, rrbs_offsets, rrbs_hits, rrbs_site_offsets, rrbs_sites) =
        if succinct_wgbs {
            (
                MappedSection::default(),
                MappedSection::default(),
                MappedSection::default(),
                MappedSection::default(),
                MappedSection::default(),
                MappedSection::default(),
            )
        } else {
            (
                checked_mapped_section::<crate::param::KmerLoc2>(
                    sections[SECTION_INDEX2],
                    file_size,
                )?,
                checked_mapped_section::<u32>(sections[SECTION_START_OFFSETS], file_size)?,
                checked_mapped_section::<u32>(sections[SECTION_RRBS_OFFSETS], file_size)?,
                checked_mapped_section::<crate::param::Hit>(sections[SECTION_RRBS_HITS], file_size)?,
                checked_mapped_section::<u32>(
                    sections[SECTION_RRBS_SITE_OFFSETS],
                    file_size,
                )?,
                checked_mapped_section::<[u32; 2]>(sections[SECTION_RRBS_SITES], file_size)?,
            )
        };
    let (wgbs_buckets, wgbs_occupancy, wgbs_rank, wgbs_overflow) = if succinct_wgbs {
        (
            checked_mapped_section::<PackedWgbsBucket>(sections[SECTION_INDEX2], file_size)?,
            checked_mapped_section::<u64>(sections[SECTION_START_OFFSETS], file_size)?,
            checked_mapped_section::<u32>(sections[SECTION_RRBS_OFFSETS], file_size)?,
            checked_mapped_section::<WgbsCountOverflow>(sections[SECTION_RRBS_HITS], file_size)?,
        )
    } else {
        (
            MappedSection::default(),
            MappedSection::default(),
            MappedSection::default(),
            MappedSection::default(),
        )
    };

    let mut reader = BufReader::new(File::open(path)?);
    let (coll, index) = match mode {
        LoadMode::Memory => {
            let index = if succinct_wgbs {
                KmerIndex {
                    total_kmers: meta.total_kmers,
                    max_kmer_num: meta.max_kmer_num,
                    index2: Vec::new(),
                    positions: read_raw_vec(&mut reader, sections[SECTION_POSITIONS])?,
                    start_offsets: Vec::new(),
                    rrbs_offsets: Vec::new(),
                    rrbs_hits: Vec::new(),
                    rrbs_site_offsets: Vec::new(),
                    rrbs_sites: Vec::new(),
                    wgbs_occupancy: read_raw_vec(
                        &mut reader,
                        sections[SECTION_START_OFFSETS],
                    )?,
                    wgbs_rank: read_raw_vec(&mut reader, sections[SECTION_RRBS_OFFSETS])?,
                    wgbs_buckets: read_raw_vec(&mut reader, sections[SECTION_INDEX2])?,
                    wgbs_overflow: read_raw_vec(&mut reader, sections[SECTION_RRBS_HITS])?,
                    seed_size: meta.seed_size,
                    mapped: None,
                }
            } else {
                KmerIndex {
                total_kmers: meta.total_kmers,
                max_kmer_num: meta.max_kmer_num,
                index2: read_raw_vec(&mut reader, sections[SECTION_INDEX2])?,
                positions: read_raw_vec(&mut reader, sections[SECTION_POSITIONS])?,
                start_offsets: read_raw_vec(&mut reader, sections[SECTION_START_OFFSETS])?,
                rrbs_offsets: read_raw_vec(&mut reader, sections[SECTION_RRBS_OFFSETS])?,
                rrbs_hits: read_raw_vec(&mut reader, sections[SECTION_RRBS_HITS])?,
                rrbs_site_offsets: read_raw_vec(
                    &mut reader,
                    sections[SECTION_RRBS_SITE_OFFSETS],
                )?,
                rrbs_sites: read_raw_vec(&mut reader, sections[SECTION_RRBS_SITES])?,
                wgbs_occupancy: Vec::new(),
                wgbs_rank: Vec::new(),
                wgbs_buckets: Vec::new(),
                wgbs_overflow: Vec::new(),
                seed_size: meta.seed_size,
                mapped: None,
                }
            };
            let refcat_data = read_raw_vec(&mut reader, sections[SECTION_REFCAT])?;
            let crefcat_data = read_raw_vec(&mut reader, sections[SECTION_CREFCAT])?;
            let coll = make_loaded_collection(
                &meta,
                ref_anchor,
                Box::new(VecStorage::new(refcat_data)),
                Box::new(VecStorage::new(crefcat_data)),
            );
            (coll, index)
        }
        LoadMode::Mmap => {
            let index_file = File::open(path)?;
            let index_mmap = unsafe { memmap2::Mmap::map(&index_file)? };
            #[cfg(unix)]
            if meta.is_rrbs {
                let _ = index_mmap.advise(memmap2::Advice::Random);
            }
            let index = KmerIndex {
                total_kmers: meta.total_kmers,
                max_kmer_num: meta.max_kmer_num,
                index2: Vec::new(),
                positions: Vec::new(),
                start_offsets: Vec::new(),
                rrbs_offsets: Vec::new(),
                rrbs_hits: Vec::new(),
                rrbs_site_offsets: Vec::new(),
                rrbs_sites: Vec::new(),
                wgbs_occupancy: Vec::new(),
                wgbs_rank: Vec::new(),
                wgbs_buckets: Vec::new(),
                wgbs_overflow: Vec::new(),
                seed_size: meta.seed_size,
                mapped: Some(MappedKmerIndex {
                    mmap: index_mmap,
                    index2,
                    positions,
                    start_offsets,
                    rrbs_offsets,
                    rrbs_hits,
                    rrbs_site_offsets,
                    rrbs_sites,
                    wgbs_occupancy,
                    wgbs_rank,
                    wgbs_buckets,
                    wgbs_overflow,
                }),
            };
            let ref_file = File::open(path)?;
            let ref_mmap = unsafe { memmap2::Mmap::map(&ref_file)? };
            let cref_file = File::open(path)?;
            let cref_mmap = unsafe { memmap2::Mmap::map(&cref_file)? };
            #[cfg(unix)]
            if meta.is_rrbs {
                let _ = ref_mmap.advise(memmap2::Advice::Random);
                let _ = cref_mmap.advise(memmap2::Advice::Random);
            }
            let coll = make_loaded_collection(
                &meta,
                ref_anchor,
                Box::new(MmapStorage::with_offset(ref_mmap, refcat.offset, refcat.len)),
                Box::new(MmapStorage::with_offset(cref_mmap, crefcat.offset, crefcat.len)),
            );
            (coll, index)
        }
    };

    log::info!(
        "索引已从 {} 加载 (v{}, {}, raw sections)",
        path.display(),
        meta.version,
        if matches!(mode, LoadMode::Mmap) { "mmap" } else { "memory" },
    );
    Ok((coll, index, meta))
}

/// Load index (supports version 1 and version 2, optional mmap).
pub fn load_index_with_mode(
    path: &Path,
    mode: LoadMode,
) -> Result<(BinSeqCollection, KmerIndex, IndexMeta)> {
    let meta = read_index_meta(path)?;
    let ref_anchor = rebuild_ref_anchor(&meta.ref_lengths)?;

    let file = File::open(path)
        .with_context(|| format!("Cannot open index file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let mut header = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header)?;
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());

    if version == INDEX_VERSION_FAST_COMPAT || version == INDEX_VERSION_SUCCINCT_WGBS {
        drop(reader);
        return load_raw_index(path, mode, meta, ref_anchor, &header);
    }

    if version == 1 {
        if matches!(mode, LoadMode::Mmap) {
            bail!(
                "Index {} is version 1, mmap not supported. Rebuild with `bsmap index`.",
                path.display()
            );
        }
        let stored_names_len = u32::from_le_bytes(header[44..48].try_into().unwrap()) as usize;
        if stored_names_len > 0 {
            let mut skip = vec![0u8; stored_names_len];
            reader.read_exact(&mut skip)?;
        }
        let index = deserialize_kmer_index(&mut reader, version, meta.seed_size)?;
        let coll = BinSeqCollection {
            total_num: meta.ref_names.len() as u32 * 2,
            sum_length: meta.ref_lengths.iter().map(|&len| len as u64).sum(),
            refcat: Box::new(VecStorage::new(vec![])),
            crefcat: Box::new(VecStorage::new(vec![])),
            ref_anchor,
            chr_lengths: meta.ref_lengths.clone(),
            blocks: vec![],
            seqs: vec![],
            chr_names: meta.ref_names.clone(),
            chr_accessions: meta
                .ref_names
                .iter()
                .map(|n| n.split_whitespace().next().unwrap_or(n).to_string())
                .collect(),
        };
        return Ok((coll, index, meta));
    }

    if version != INDEX_VERSION_V2
        && version != INDEX_VERSION_RRBS_MODE_AWARE
        && version != INDEX_VERSION_CHR_LENGTHS
        && version != INDEX_VERSION_RRBS_FLAT
        && version != INDEX_VERSION_RRBS_SITES
        && version != INDEX_VERSION_FAST_COMPAT
        && version != INDEX_VERSION_SUCCINCT_WGBS
    {
        bail!(
            "Unsupported index version {}: {}",
            version,
            path.display()
        );
    }

    // Version 2
    let refcat_len = u64::from_le_bytes(header[48..56].try_into().unwrap()) as usize;
    let crefcat_len = u64::from_le_bytes(header[56..64].try_into().unwrap()) as usize;
    let stored_names_len = u32::from_le_bytes(header[44..48].try_into().unwrap()) as usize;
    if stored_names_len > 0 {
        let mut skip = vec![0u8; stored_names_len];
        reader.read_exact(&mut skip)?;
    }

    let index = deserialize_kmer_index(&mut reader, version, meta.seed_size)?;
    drop(reader);

    let file_meta = std::fs::metadata(path)?;
    let file_size = file_meta.len() as usize;
    let names_and_header_size = HEADER_SIZE + stored_names_len;
    let expected_refcat_bytes = refcat_len * 8;
    let expected_crefcat_bytes = crefcat_len * 8;
    let raw_data_size = expected_refcat_bytes + expected_crefcat_bytes;
    let index_data_size = file_size - names_and_header_size - raw_data_size;
    // index_data_size includes bincode data + alignment padding
    // refcat starts at the next 8-byte boundary after bincode data
    let bincode_end = names_and_header_size + index_data_size;
    let refcat_offset = (bincode_end + 7) & !7; // round up to 8-byte alignment
    let crefcat_offset = refcat_offset + expected_refcat_bytes;

    match mode {
        LoadMode::Memory => {
            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            reader.seek(std::io::SeekFrom::Start(refcat_offset as u64))?;
            let mut refcat_data = vec![0u64; refcat_len];
            unsafe {
                let bytes = std::slice::from_raw_parts_mut(
                    refcat_data.as_mut_ptr() as *mut u8,
                    expected_refcat_bytes,
                );
                reader.read_exact(bytes)?;
            }
            let mut crefcat_data = vec![0u64; crefcat_len];
            unsafe {
                let bytes = std::slice::from_raw_parts_mut(
                    crefcat_data.as_mut_ptr() as *mut u8,
                    expected_crefcat_bytes,
                );
                reader.read_exact(bytes)?;
            }
            let coll = BinSeqCollection {
                total_num: meta.ref_names.len() as u32 * 2,
                sum_length: meta.ref_lengths.iter().map(|&len| len as u64).sum(),
                refcat: Box::new(VecStorage::new(refcat_data)),
                crefcat: Box::new(VecStorage::new(crefcat_data)),
                ref_anchor: ref_anchor.clone(),
                chr_lengths: meta.ref_lengths.clone(),
                blocks: vec![],
                seqs: vec![],
                chr_names: meta.ref_names.clone(),
                chr_accessions: meta
                    .ref_names
                    .iter()
                    .map(|n| n.split_whitespace().next().unwrap_or(n).to_string())
                    .collect(),
            };
            log::info!(
                "索引已从 {} 加载 (v{}, memory, refcat={} words, crefcat={} words)",
                path.display(),
                version,
                refcat_len,
                crefcat_len,
            );
            Ok((coll, index, meta))
        }
        LoadMode::Mmap => {
            let file1 = File::open(path)?;
            let mmap1 = unsafe { memmap2::Mmap::map(&file1)? };
            let file2 = File::open(path)?;
            let mmap2 = unsafe { memmap2::Mmap::map(&file2)? };
            let refcat_storage = MmapStorage::with_offset(mmap1, refcat_offset, refcat_len);
            let crefcat_storage = MmapStorage::with_offset(mmap2, crefcat_offset, crefcat_len);
            let coll = BinSeqCollection {
                total_num: meta.ref_names.len() as u32 * 2,
                sum_length: meta.ref_lengths.iter().map(|&len| len as u64).sum(),
                refcat: Box::new(refcat_storage),
                crefcat: Box::new(crefcat_storage),
                ref_anchor,
                chr_lengths: meta.ref_lengths.clone(),
                blocks: vec![],
                seqs: vec![],
                chr_names: meta.ref_names.clone(),
                chr_accessions: meta
                    .ref_names
                    .iter()
                    .map(|n| n.split_whitespace().next().unwrap_or(n).to_string())
                    .collect(),
            };
            log::info!(
                "索引已从 {} 加载 (v{}, mmap, refcat={} words, crefcat={} words)",
                path.display(),
                version,
                refcat_len,
                crefcat_len,
            );
            Ok((coll, index, meta))
        }
    }
}

fn rebuild_ref_anchor(ref_lengths: &[u32]) -> Result<Vec<u32>> {
    if ref_lengths.is_empty() {
        return Ok(Vec::new());
    }

    let mut anchor = (REF_MARGIN * SEGLEN) as u64;
    let mut anchors = Vec::with_capacity(ref_lengths.len() + 1);
    anchors.push(anchor as u32);
    for &length in ref_lengths {
        let words = (length as u64 + SEGLEN as u64 - 1) / SEGLEN as u64
            + BINSEQPAD as u64;
        anchor = anchor
            .checked_add(words * SEGLEN as u64)
            .context("Reference anchor overflow")?;
        if anchor > u32::MAX as u64 {
            bail!("Reference anchor exceeds u32 range");
        }
        anchors.push(anchor as u32);
    }
    Ok(anchors)
}

fn deserialize_kmer_index<R: Read>(
    reader: &mut R,
    version: u32,
    seed_size: u32,
) -> Result<KmerIndex> {
    if version >= INDEX_VERSION_RRBS_SITES {
        let data: IndexDataV6 = bincode_opts()
            .deserialize_from(reader)
            .context("Failed to deserialize v6 index data")?;
        return Ok(reconstruct_kmer_index_v6(data, seed_size));
    }
    if version >= INDEX_VERSION_RRBS_FLAT {
        let data: IndexDataV5 = bincode_opts()
            .deserialize_from(reader)
            .context("Failed to deserialize v5 index data")?;
        return Ok(reconstruct_kmer_index_v5(data, seed_size));
    }

    let data: IndexDataV4 = bincode_opts()
        .deserialize_from(reader)
        .context("Failed to deserialize legacy index data")?;
    Ok(reconstruct_kmer_index_v4(data, seed_size))
}

fn reconstruct_kmer_index_v6(data: IndexDataV6, seed_size: u32) -> KmerIndex {
    KmerIndex {
        total_kmers: data.total_kmers,
        max_kmer_num: data.max_kmer_num,
        index2: data.index2.into_iter().map(|e| crate::param::KmerLoc2 { n: e.n }).collect(),
        positions: data.positions,
        start_offsets: data.start_offsets,
        rrbs_offsets: data.rrbs_offsets,
        rrbs_hits: data.rrbs_hits,
        rrbs_site_offsets: data.rrbs_site_offsets,
        rrbs_sites: data
            .rrbs_sites
            .into_iter()
            .map(|(position, reverse_offset)| [position, reverse_offset])
            .collect(),
        wgbs_occupancy: Vec::new(),
        wgbs_rank: Vec::new(),
        wgbs_buckets: Vec::new(),
        wgbs_overflow: Vec::new(),
        seed_size,
        mapped: None,
    }
}

fn reconstruct_kmer_index_v5(data: IndexDataV5, seed_size: u32) -> KmerIndex {
    KmerIndex {
        total_kmers: data.total_kmers,
        max_kmer_num: data.max_kmer_num,
        index2: data
            .index2
            .into_iter()
            .map(|e| crate::param::KmerLoc2 {
                n: e.n,
            })
            .collect(),
        positions: data.positions,
        start_offsets: data.start_offsets,
        rrbs_offsets: data.rrbs_offsets,
        rrbs_hits: data.rrbs_hits,
        rrbs_site_offsets: Vec::new(),
        rrbs_sites: Vec::new(),
        wgbs_occupancy: Vec::new(),
        wgbs_rank: Vec::new(),
        wgbs_buckets: Vec::new(),
        wgbs_overflow: Vec::new(),
        seed_size,
        mapped: None,
    }
}

fn reconstruct_kmer_index_v4(data: IndexDataV4, seed_size: u32) -> KmerIndex {
    let (rrbs_offsets, rrbs_hits) = if let Some(buckets) = data.rrbs_index {
        let mut offsets = Vec::with_capacity(buckets.len() + 1);
        let mut hits = Vec::new();
        offsets.push(0);
        for bucket in buckets {
            debug_assert_eq!(bucket.n1 as usize, bucket.loc1.len());
            hits.extend(bucket.loc1.into_iter().map(|hit| crate::param::Hit {
                chr: hit.chr,
                loc: hit.loc,
            }));
            offsets.push(hits.len() as u32);
        }
        (offsets, hits)
    } else {
        (Vec::new(), Vec::new())
    };

    KmerIndex {
        total_kmers: data.total_kmers,
        max_kmer_num: data.max_kmer_num,
        index2: data
            .index2
            .into_iter()
            .map(|entry| crate::param::KmerLoc2 { n: entry.n })
            .collect(),
        positions: data.positions,
        start_offsets: data.start_offsets,
        rrbs_offsets,
        rrbs_hits,
        rrbs_site_offsets: Vec::new(),
        rrbs_sites: Vec::new(),
        wgbs_occupancy: Vec::new(),
        wgbs_rank: Vec::new(),
        wgbs_buckets: Vec::new(),
        wgbs_overflow: Vec::new(),
        seed_size,
        mapped: None,
    }
}

/// Check whether a v8 cached index matches the source FASTA and all build parameters.
pub fn is_index_compatible(
    path: &Path,
    reference_path: &Path,
    params: &IndexParameters<'_>,
) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let meta = read_index_meta(path)?;
    if meta.version != INDEX_VERSION_SUCCINCT_WGBS {
        log::info!(
            "缓存索引版本 {} 不兼容，需要重建 v{} 索引",
            meta.version,
            INDEX_VERSION_SUCCINCT_WGBS,
        );
        return Ok(false);
    }
    let mut header = [0u8; HEADER_SIZE];
    File::open(path)?.read_exact(&mut header)?;
    if !has_raw_section_marker(&header, RAW_SECTION_MARKER_V2) {
        log::info!("缓存 v8 索引使用旧布局，需要重建 raw-section 索引");
        return Ok(false);
    }

    let source = SourceFingerprint::from_path(reference_path)?;
    if meta.source_size != source.size
        || meta.source_mtime_secs != source.mtime_secs
        || meta.source_mtime_nanos != source.mtime_nanos
    {
        log::info!("Cached index is incompatible: reference FASTA stat changed");
        return Ok(false);
    }

    if meta.seed_size != params.seed_size
        || meta.is_rrbs != params.is_rrbs
        || meta.index_interval != params.index_interval
        || meta.max_kmer_ratio.to_bits() != params.max_kmer_ratio.to_bits()
    {
        log::info!(
            "缓存索引不兼容: 文件 seed_size={}, mode={}，需要 seed_size={}, mode={}",
            meta.seed_size,
            if meta.is_rrbs { "RRBS" } else { "WGBS" },
            params.seed_size,
            if params.is_rrbs { "RRBS" } else { "WGBS" },
        );
        return Ok(false);
    }

    if params.is_rrbs
        && (meta.rrbs_min_insert != params.min_insert
            || meta.rrbs_max_insert != params.max_insert
            || meta.digest_sites_hash != digest_sites_hash(params.digest_sites))
    {
        log::info!("缓存 RRBS 索引参数不兼容");
        return Ok(false);
    }

    Ok(true)
}

/// Default index file path derived from the reference FASTA path.
///
/// E.g., `ref.fa` → `ref.fa.bsi` (BSMAP Index)
pub fn default_index_path(ref_path: &Path) -> std::path::PathBuf {
    let mut p = ref_path.to_path_buf();
    let ext = p.extension().map(|e| format!("{}.bsi", e.to_string_lossy()));
    match ext {
        Some(e) => { p.set_extension(e); }
        None => {
            p.set_extension("bsi");
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::binseq::BinSeqCollection;
    use crate::reference::fasta::Reference;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    fn make_test_refs() -> Vec<Reference> {
        vec![
            Reference {
                name: "chr1".into(),
                seq: b"ACGTACGTACGTACGTACGTACGTACGTACGT".to_vec(),
                len: 32,
            },
            Reference {
                name: "chr2".into(),
                seq: b"TGCAACGTACGT".to_vec(),
                len: 12,
            },
        ]
    }

    fn make_test_fasta() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b">chr1\nACGTACGTACGTACGTACGTACGTACGTACGT\n>chr2\nTGCAACGTACGT\n")
            .unwrap();
        file.flush().unwrap();
        file
    }

    fn wgbs_params() -> IndexParameters<'static> {
        IndexParameters {
            seed_size: 3,
            index_interval: 4,
            max_kmer_ratio: 0.01,
            is_rrbs: false,
            min_insert: 28,
            max_insert: 1000,
            digest_sites: &[],
        }
    }

    fn save_legacy_v6(
        path: &Path,
        index: &KmerIndex,
        coll: &BinSeqCollection,
        ref_names: &[String],
        params: &IndexParameters<'_>,
    ) {
        let mut header = [0u8; HEADER_SIZE];
        header[0..8].copy_from_slice(INDEX_MAGIC);
        header[8..12].copy_from_slice(&INDEX_VERSION_RRBS_SITES.to_le_bytes());
        header[12..16].copy_from_slice(&params.seed_size.to_le_bytes());
        header[16..20].copy_from_slice(
            &(if params.is_rrbs { MODE_RRBS } else { MODE_WGBS }).to_le_bytes(),
        );
        header[20..24].copy_from_slice(&index.total_kmers.to_le_bytes());
        header[24..28].copy_from_slice(&index.max_kmer_num.to_le_bytes());
        header[28..32].copy_from_slice(&params.index_interval.to_le_bytes());
        header[32..40].copy_from_slice(&params.max_kmer_ratio.to_le_bytes());
        header[40..44].copy_from_slice(&(ref_names.len() as u32).to_le_bytes());

        let mut names = Vec::new();
        for (name, &length) in ref_names.iter().zip(&coll.chr_lengths) {
            names.extend_from_slice(&(name.len() as u16).to_le_bytes());
            names.extend_from_slice(name.as_bytes());
            names.extend_from_slice(&length.to_le_bytes());
        }
        header[44..48].copy_from_slice(&(names.len() as u32).to_le_bytes());
        header[48..56].copy_from_slice(&(coll.refcat.len() as u64).to_le_bytes());
        header[56..64].copy_from_slice(&(coll.crefcat.len() as u64).to_le_bytes());

        let file = File::create(path).unwrap();
        let mut writer = BufWriter::new(file);
        writer.write_all(&header).unwrap();
        writer.write_all(&names).unwrap();
        let data = IndexDataV6Ref::from(index);
        bincode_opts().serialize_into(&mut writer, &data).unwrap();
        let current = HEADER_SIZE + names.len()
            + bincode_opts().serialized_size(&data).unwrap() as usize;
        let padding = (8 - current % 8) % 8;
        writer.write_all(&[0u8; 8][..padding]).unwrap();
        write_raw_slice(&mut writer, coll.refcat.as_slice()).unwrap();
        write_raw_slice(&mut writer, coll.crefcat.as_slice()).unwrap();
        writer.flush().unwrap();
    }

    #[test]
    fn test_save_and_load_wgbs_index() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();

        let seed_size = 3;
        let index_interval = 4;
        let max_kmer_ratio = 0.01;

        let index = KmerIndex::build_wgbs(&coll, seed_size, index_interval, max_kmer_ratio);

        // Save
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        save_index(&path, &index, seed_size, index_interval, max_kmer_ratio, &ref_names, false)
            .unwrap();

        // Load
        let (loaded_index, meta) = load_index(&path).unwrap();

        // Verify metadata
        assert_eq!(meta.seed_size, seed_size);
        assert!(!meta.is_rrbs);
        assert_eq!(meta.ref_names, ref_names);
        assert_eq!(meta.total_kmers, index.total_kmers);

        // Verify index data
        assert_eq!(loaded_index.total_kmers, index.total_kmers);
        assert_eq!(loaded_index.max_kmer_num, index.max_kmer_num);
        assert_eq!(loaded_index.positions, index.positions);
        assert_eq!(loaded_index.index2.len(), index.total_kmers as usize);
        for hash in 0..index.total_kmers {
            assert_eq!(
                loaded_index.lookup_separated(hash),
                index.lookup_separated(hash),
                "legacy round-trip differs for hash {hash}",
            );
            assert_eq!(
                loaded_index.wgbs_candidate_count(hash),
                index.wgbs_candidate_count(hash),
                "legacy raw count differs for hash {hash}",
            );
        }
    }

    #[test]
    fn test_v7_save_load_preserves_chromosome_lengths_and_anchors() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        let tmp = NamedTempFile::new().unwrap();
        let fasta = make_test_fasta();

        save_index_v2(
            tmp.path(),
            &index,
            &coll,
            &ref_names,
            fasta.path(),
            &wgbs_params(),
        )
        .unwrap();
        let (loaded_coll, loaded_index, meta) =
            load_index_with_mode(tmp.path(), LoadMode::Mmap).unwrap();

        assert_eq!(meta.version, INDEX_VERSION_SUCCINCT_WGBS);
        assert_eq!(meta.ref_lengths, vec![32, 12]);
        assert_eq!(loaded_coll.chr_lengths, coll.chr_lengths);
        assert_eq!(loaded_coll.ref_anchor, coll.ref_anchor);
        assert_eq!(loaded_coll.sum_length, coll.sum_length);
        assert_eq!(loaded_coll.total_num, coll.total_num);
        assert!(loaded_index.mapped.is_some());
        assert!(loaded_index.index2.is_empty());
        for hash in 0..index.total_kmers {
            assert_eq!(loaded_index.lookup_separated(hash), index.lookup_separated(hash));
        }
    }

    #[test]
    fn test_v7_rejects_section_outside_file() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|reference| reference.name.clone()).collect();
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        let index_file = NamedTempFile::new().unwrap();
        let fasta = make_test_fasta();
        save_index_v2(
            index_file.path(),
            &index,
            &coll,
            &ref_names,
            fasta.path(),
            &wgbs_params(),
        )
        .unwrap();

        let invalid_offset = std::fs::metadata(index_file.path()).unwrap().len() + 8;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(index_file.path())
            .unwrap();
        file.seek(std::io::SeekFrom::Start(
            (SECTION_DIRECTORY_OFFSET + SECTION_INDEX2 * SECTION_ENTRY_SIZE) as u64,
        ))
        .unwrap();
        file.write_all(&invalid_offset.to_le_bytes()).unwrap();
        file.flush().unwrap();

        let error = match load_index_with_mode(index_file.path(), LoadMode::Mmap) {
            Ok(_) => panic!("corrupt section should be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("beyond end of file"));
    }

    #[test]
    fn test_v7_rrbs_roundtrip_preserves_flat_buckets_and_sites() {
        let refs = vec![Reference {
            name: "chr1".into(),
            seq: b"ACGTCCGGAAAAAAAAAAAAAAAAAAAAAAACCGGTTTTTTTTTTTTTTTTTTTTTTTTCCGG"
                .to_vec(),
            len: 68,
        }];
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names = vec!["chr1".to_string()];
        let index = KmerIndex::build_rrbs(
            &coll,
            &refs,
            3,
            4,
            &["C-CGG".to_string()],
            4,
            1000,
        );
        assert!(!index.rrbs_hits.is_empty());
        let tmp = NamedTempFile::new().unwrap();
        let mut fasta = NamedTempFile::new().unwrap();
        fasta.write_all(b">chr1\nACGTCCGGAAAAAAAAAAAAAAAAAAAAAAACCGGTTTTTTTTTTTTTTTTTTTTTTCCGG\n")
            .unwrap();
        fasta.flush().unwrap();
        let digest_sites = vec!["C-CGG".to_string()];
        let params = IndexParameters {
            is_rrbs: true,
            min_insert: 4,
            digest_sites: &digest_sites,
            ..wgbs_params()
        };

        save_index_v2(tmp.path(), &index, &coll, &ref_names, fasta.path(), &params).unwrap();
        let (_loaded_coll, loaded, meta) =
            load_index_with_mode(tmp.path(), LoadMode::Memory).unwrap();

        assert_eq!(meta.version, INDEX_VERSION_SUCCINCT_WGBS);
        assert!(meta.is_rrbs);
        assert_eq!(loaded.rrbs_offsets, index.rrbs_offsets);
        assert_eq!(loaded.rrbs_hits, index.rrbs_hits);
        assert_eq!(loaded.rrbs_site_offsets, index.rrbs_site_offsets);
        assert_eq!(loaded.rrbs_sites, index.rrbs_sites);
        for hash in 0..index.total_kmers {
            assert_eq!(loaded.lookup_rrbs(hash), index.lookup_rrbs(hash));
        }

        let (_mapped_coll, mapped, _) =
            load_index_with_mode(tmp.path(), LoadMode::Mmap).unwrap();
        assert!(mapped.mapped.is_some());
        for hash in 0..index.total_kmers {
            assert_eq!(mapped.lookup_rrbs(hash), index.lookup_rrbs(hash));
        }
        assert_eq!(mapped.rrbs_fragment(0, 6, 20), index.rrbs_fragment(0, 6, 20));
    }

    #[test]
    fn test_v7_compatibility_checks_source_stat_and_wgbs_parameters() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let mut fasta = make_test_fasta();
        let params = wgbs_params();
        save_index_v2(&path, &index, &coll, &ref_names, fasta.path(), &params).unwrap();

        assert!(is_index_compatible(&path, fasta.path(), &params).unwrap());

        let mut changed = params;
        changed.seed_size += 1;
        assert!(!is_index_compatible(&path, fasta.path(), &changed).unwrap());
        changed = params;
        changed.index_interval += 1;
        assert!(!is_index_compatible(&path, fasta.path(), &changed).unwrap());
        changed = params;
        changed.max_kmer_ratio *= 2.0;
        assert!(!is_index_compatible(&path, fasta.path(), &changed).unwrap());
        changed = params;
        changed.is_rrbs = true;
        assert!(!is_index_compatible(&path, fasta.path(), &changed).unwrap());

        std::thread::sleep(Duration::from_millis(10));
        fasta.seek(std::io::SeekFrom::Start(0)).unwrap();
        fasta.write_all(b"!").unwrap();
        fasta.flush().unwrap();
        assert!(!is_index_compatible(&path, fasta.path(), &params).unwrap());

        save_index_v2(&path, &index, &coll, &ref_names, fasta.path(), &params).unwrap();
        fasta.as_file_mut().set_len(0).unwrap();
        fasta.write_all(b">chr1\nACGT\n").unwrap();
        fasta.flush().unwrap();
        assert!(!is_index_compatible(&path, fasta.path(), &params).unwrap());

        assert!(!is_index_compatible(Path::new("/nonexistent.bsi"), fasta.path(), &params).unwrap());
    }

    #[test]
    fn test_v7_compatibility_checks_all_rrbs_parameters() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
        let digest_sites = vec!["C-CGG".to_string()];
        let index = KmerIndex::build_rrbs(&coll, &refs, 3, 4, &digest_sites, 28, 1000);
        let index_file = NamedTempFile::new().unwrap();
        let fasta = make_test_fasta();
        let params = IndexParameters {
            is_rrbs: true,
            digest_sites: &digest_sites,
            ..wgbs_params()
        };
        save_index_v2(
            index_file.path(),
            &index,
            &coll,
            &ref_names,
            fasta.path(),
            &params,
        )
        .unwrap();

        assert!(is_index_compatible(index_file.path(), fasta.path(), &params).unwrap());

        let mut changed = params;
        changed.min_insert += 1;
        assert!(!is_index_compatible(index_file.path(), fasta.path(), &changed).unwrap());
        changed = params;
        changed.max_insert += 1;
        assert!(!is_index_compatible(index_file.path(), fasta.path(), &changed).unwrap());
        let changed_sites = vec!["C-CGG".to_string(), "C-CWGG".to_string()];
        changed = params;
        changed.digest_sites = &changed_sites;
        assert!(!is_index_compatible(index_file.path(), fasta.path(), &changed).unwrap());
    }

    #[test]
    fn test_v6_full_index_remains_readable_but_is_not_cache_compatible() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        let index_file = NamedTempFile::new().unwrap();
        let fasta = make_test_fasta();
        let params = wgbs_params();
        save_legacy_v6(index_file.path(), &index, &coll, &ref_names, &params);

        let (loaded_coll, loaded_index, meta) =
            load_index_with_mode(index_file.path(), LoadMode::Memory).unwrap();
        assert_eq!(meta.version, INDEX_VERSION_RRBS_SITES);
        assert_eq!(loaded_coll.ref_anchor, coll.ref_anchor);
        assert_eq!(loaded_index.positions, index.positions);
        assert!(!is_index_compatible(index_file.path(), fasta.path(), &params).unwrap());
    }

    #[test]
    fn test_default_index_path() {
        let p = default_index_path(Path::new("/data/ref.fa"));
        assert_eq!(p.to_string_lossy(), "/data/ref.fa.bsi");

        let p = default_index_path(Path::new("/data/genome.fasta"));
        assert_eq!(p.to_string_lossy(), "/data/genome.fasta.bsi");
    }

    #[test]
    fn test_read_meta_only() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();

        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);

        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        save_index(&path, &index, 3, 4, 0.01, &ref_names, false).unwrap();

        // Read only metadata (without deserializing full index)
        let meta = read_index_meta(&path).unwrap();
        assert_eq!(meta.seed_size, 3);
        assert!(!meta.is_rrbs);
        assert_eq!(meta.ref_names, ref_names);
        assert_eq!(meta.index_interval, 4);
    }
}
