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

use anyhow::{bail, Context, Result};
use bincode::Options;
use serde::{Deserialize, Serialize};

use super::binseq::BinSeqCollection;
use super::index::KmerIndex;
use super::storage::{MmapStorage, VecStorage};

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

/// WGBS alignment mode.
const MODE_WGBS: u32 = 0;

/// RRBS alignment mode.
const MODE_RRBS: u32 = 1;

/// Fixed header size in bytes.
const HEADER_SIZE: usize = 256;

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

#[derive(Serialize)]
struct IndexDataV5Ref<'a> {
    total_kmers: u32,
    max_kmer_num: u32,
    index2: Vec<IndexKmerLoc2>,
    positions: &'a [u32],
    start_offsets: &'a [u32],
    rrbs_offsets: &'a [u32],
    rrbs_hits: &'a [crate::param::Hit],
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

impl<'a> From<&'a KmerIndex> for IndexDataV5Ref<'a> {
    fn from(idx: &'a KmerIndex) -> Self {
        Self {
            total_kmers: idx.total_kmers,
            max_kmer_num: idx.max_kmer_num,
            index2: idx
                .index2
                .iter()
                .map(|e| IndexKmerLoc2 { n: e.n })
                .collect(),
            positions: &idx.positions,
            start_offsets: &idx.start_offsets,
            rrbs_offsets: &idx.rrbs_offsets,
            rrbs_hits: &idx.rrbs_hits,
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
    seed_size: u32,
    index_interval: u32,
    max_kmer_ratio: f64,
    ref_names: &[String],
    is_rrbs: bool,
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
    let version = INDEX_VERSION_RRBS_FLAT;
    header[8..12].copy_from_slice(&version.to_le_bytes());
    header[12..16].copy_from_slice(&seed_size.to_le_bytes());
    let mode = if is_rrbs { MODE_RRBS } else { MODE_WGBS };
    header[16..20].copy_from_slice(&mode.to_le_bytes());
    header[20..24].copy_from_slice(&index.total_kmers.to_le_bytes());
    header[24..28].copy_from_slice(&index.max_kmer_num.to_le_bytes());
    header[28..32].copy_from_slice(&index_interval.to_le_bytes());
    header[32..40].copy_from_slice(&max_kmer_ratio.to_le_bytes());
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

    writer.write_all(&header).context("Failed to write index header")?;
    writer.write_all(&names_buf).context("Failed to write reference names")?;

    // ── Index data (bincode) ──
    let data = IndexDataV5Ref::from(index);
    bincode_opts()
        .serialize_into(&mut writer, &data)
        .context("Failed to serialize index data")?;

    // ── Padding to 8-byte alignment for refcat ──
    let current_pos = HEADER_SIZE + names_buf.len()
        + bincode_opts().serialized_size(&data).unwrap() as usize;
    let padding = (8 - (current_pos % 8)) % 8;
    if padding > 0 {
        writer.write_all(&[0u8; 8][..padding]).context("Failed to write alignment padding")?;
    }
    // Record actual refcat offset in header (overwrite bytes 64-71 which were reserved)
    // We can't seek back in BufWriter easily, so we'll compute the offset on load side instead.

    // ── refcat raw data ──
    let refcat_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(refcat_slice.as_ptr() as *const u8, refcat_slice.len() * 8)
    };
    writer.write_all(refcat_bytes).context("Failed to write refcat data")?;

    // ── crefcat raw data ──
    let crefcat_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(crefcat_slice.as_ptr() as *const u8, crefcat_slice.len() * 8)
    };
    writer.write_all(crefcat_bytes).context("Failed to write crefcat data")?;

    writer.flush().context("Failed to flush index file")?;
    log::info!(
        "索引已保存到 {} (v5, refcat={} words, crefcat={} words)",
        path.display(),
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
    {
        bail!(
            "Unsupported index version {} (expected {}, {}, {}, {}, or {}): {}",
            version,
            INDEX_VERSION,
            INDEX_VERSION_V2,
            INDEX_VERSION_RRBS_MODE_AWARE,
            INDEX_VERSION_CHR_LENGTHS,
            INDEX_VERSION_RRBS_FLAT,
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
    })
}

impl IndexDataV4 {
    fn from_flat(idx: &KmerIndex) -> Self {
        let rrbs_index = if idx.rrbs_offsets.is_empty() {
            None
        } else {
            Some(
                idx.rrbs_offsets
                    .windows(2)
                    .map(|range| {
                        let start = range[0] as usize;
                        let end = range[1] as usize;
                        IndexKmerLoc {
                            n1: (end - start) as u32,
                            loc1: idx.rrbs_hits[start..end]
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
        Self {
            total_kmers: idx.total_kmers,
            max_kmer_num: idx.max_kmer_num,
            index2: idx
                .index2
                .iter()
                .map(|entry| IndexKmerLoc2 { n: entry.n })
                .collect(),
            positions: idx.positions.clone(),
            start_offsets: idx.start_offsets.clone(),
            rrbs_index,
        }
    }
}

/// Load a full k-mer index from disk.
///
/// Returns the reconstructed `KmerIndex` and its metadata.
pub fn load_index(path: &Path) -> Result<(KmerIndex, IndexMeta)> {
    let meta = read_index_meta(path)?;

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

/// Load index (supports version 1 and version 2, optional mmap).
pub fn load_index_with_mode(
    path: &Path,
    mode: LoadMode,
) -> Result<(BinSeqCollection, KmerIndex, IndexMeta)> {
    let meta = read_index_meta(path)?;

    let file = File::open(path)
        .with_context(|| format!("Cannot open index file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let mut header = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header)?;
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());

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
            ref_anchor: vec![],
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
                ref_anchor: vec![],
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
                ref_anchor: vec![],
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

fn deserialize_kmer_index<R: Read>(
    reader: &mut R,
    version: u32,
    seed_size: u32,
) -> Result<KmerIndex> {
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
        seed_size,
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
        seed_size,
    }
}

/// Check if a cached index file exists and is compatible with the given parameters.
///
/// Returns `Ok(true)` if the file exists and matches the reference names,
/// seed_size, and mode. Returns `Ok(false)` if the file doesn't exist.
/// Returns `Err` if the file exists but is incompatible.
pub fn is_index_compatible(
    path: &Path,
    ref_names: &[String],
    seed_size: u32,
    is_rrbs: bool,
) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    let meta = read_index_meta(path)?;
    if is_rrbs && meta.version != INDEX_VERSION_RRBS_FLAT {
        log::info!(
            "缂撳瓨 RRBS 绱㈠紩鐗堟湰 {} 涓嶅吋瀹癸紝闇€瑕侀噸寤虹储寮?",
            meta.version,
        );
        return Ok(false);
    }
    if meta.seed_size != seed_size || meta.is_rrbs != is_rrbs {
        log::info!(
            "缓存索引不兼容: 文件 seed_size={}, mode={}，需要 seed_size={}, mode={}",
            meta.seed_size,
            if meta.is_rrbs { "RRBS" } else { "WGBS" },
            seed_size,
            if is_rrbs { "RRBS" } else { "WGBS" },
        );
        return Ok(false);
    }

    if meta.ref_names != ref_names {
        log::info!("缓存索引不兼容: 参考序列名称不匹配");
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
        assert_eq!(loaded_index.index2.len(), index.index2.len());
        for (a, b) in loaded_index.index2.iter().zip(index.index2.iter()) {
            assert_eq!(a.n, b.n);
        }
    }

    #[test]
    fn test_v5_save_load_preserves_chromosome_lengths() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();
        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);
        let tmp = NamedTempFile::new().unwrap();

        save_index_v2(tmp.path(), &index, &coll, 3, 4, 0.01, &ref_names, false).unwrap();
        let (loaded_coll, _loaded_index, meta) =
            load_index_with_mode(tmp.path(), LoadMode::Memory).unwrap();

        assert_eq!(meta.version, INDEX_VERSION_RRBS_FLAT);
        assert_eq!(meta.ref_lengths, vec![32, 12]);
        assert_eq!(loaded_coll.chr_lengths, coll.chr_lengths);
        assert_eq!(loaded_coll.sum_length, coll.sum_length);
        assert_eq!(loaded_coll.total_num, coll.total_num);
    }

    #[test]
    fn test_v5_rrbs_roundtrip_preserves_flat_buckets() {
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

        save_index_v2(tmp.path(), &index, &coll, 3, 4, 0.01, &ref_names, true).unwrap();
        let (_loaded_coll, loaded, meta) =
            load_index_with_mode(tmp.path(), LoadMode::Memory).unwrap();

        assert_eq!(meta.version, INDEX_VERSION_RRBS_FLAT);
        assert!(meta.is_rrbs);
        assert_eq!(loaded.rrbs_offsets, index.rrbs_offsets);
        assert_eq!(loaded.rrbs_hits, index.rrbs_hits);
        for hash in 0..index.total_kmers {
            assert_eq!(loaded.lookup_rrbs(hash), index.lookup_rrbs(hash));
        }
    }

    #[test]
    fn test_compatibility_check() {
        let refs = make_test_refs();
        let coll = BinSeqCollection::from_references(&refs);
        let ref_names: Vec<String> = refs.iter().map(|r| r.name.clone()).collect();

        let index = KmerIndex::build_wgbs(&coll, 3, 4, 0.01);

        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        save_index(&path, &index, 3, 4, 0.01, &ref_names, false).unwrap();

        // Compatible
        assert!(is_index_compatible(&path, &ref_names, 3, false).unwrap());

        // Wrong seed_size
        assert!(!is_index_compatible(&path, &ref_names, 4, false).unwrap());

        // Wrong mode
        assert!(!is_index_compatible(&path, &ref_names, 3, true).unwrap());

        // Wrong ref names
        assert!(!is_index_compatible(
            &path,
            &["chrX".to_string()],
            3,
            false
        )
        .unwrap());

        // Non-existent file
        assert!(!is_index_compatible(Path::new("/nonexistent.bsi"), &ref_names, 3, false).unwrap());
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
