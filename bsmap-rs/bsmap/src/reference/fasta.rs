//! FASTA reference genome loader (needletail backend).
//!
//! Loads multi-FASTA files (plain or gzipped) using the `needletail` crate
//! for zero-copy parsing. Returns per-chromosome sequences as uppercase
//! byte vectors. Mirrors C++ `RefSeq::LoadNextSeq()`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use needletail::{parse_fastx_file, parse_fastx_reader, FastxReader};

/// One reference sequence (typically a chromosome).
#[derive(Debug, Clone)]
pub struct Reference {
    /// Sequence name (FASTA header minus the '>').
    pub name: String,
    /// Raw sequence as uppercase ACGT bytes.
    pub seq: Vec<u8>,
    /// Length in bases.
    pub len: u32,
}

/// Streaming FASTA reader that owns at most one decoded chromosome at a time.
pub struct ReferenceReader {
    reader: Box<dyn FastxReader>,
}

impl ReferenceReader {
    pub fn open(path: &Path) -> Result<Self> {
        let reader = parse_fastx_file(path)
            .with_context(|| format!("Cannot open reference: {}", path.display()))?;
        Ok(Self { reader })
    }

    pub fn next_reference(&mut self) -> Result<Option<Reference>> {
        loop {
            let Some(record) = self.reader.next() else {
                return Ok(None);
            };
            let record = record.context("Error reading FASTA record")?;
            let name = std::str::from_utf8(record.id())
                .unwrap_or("<non-utf8-id>")
                .to_string();
            let mut seq = record.seq().to_vec();
            seq.make_ascii_uppercase();
            let len = seq.len() as u32;
            if len > 0 {
                return Ok(Some(Reference { name, seq, len }));
            }
        }
    }
}

/// Load all sequences from a FASTA file (plain or gzipped).
///
/// Uses `needletail` for zero-copy, streaming parsing with automatic
/// gzip detection and decompression.
pub fn load_fasta(path: &Path, _is_gz: bool) -> Result<Vec<Reference>> {
    let mut reader =
        parse_fastx_file(path).with_context(|| format!("Cannot open reference: {}", path.display()))?;
    read_all_records(&mut *reader)
}

/// Load all sequences from a FASTA file, explicitly controlling gzip detection.
///
/// If `is_gz` is true, wraps the file in a gzip decoder before parsing.
pub fn load_fasta_with_gzip(path: &Path, is_gz: bool) -> Result<Vec<Reference>> {
    let file = File::open(path)
        .with_context(|| format!("Cannot open reference: {}", path.display()))?;

    if is_gz {
        use flate2::bufread::GzDecoder;
        use std::io::BufReader;
        let gz_reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        let mut fastx_reader = parse_fastx_reader(gz_reader)
            .context("Failed to create FASTA reader for gzipped input")?;
        read_all_records(&mut *fastx_reader)
    } else {
        let mut reader =
            parse_fastx_file(path).with_context(|| format!("Cannot open reference: {}", path.display()))?;
        read_all_records(&mut *reader)
    }
}

/// Detect whether a FASTA file is gzipped by checking the magic bytes.
pub fn is_gzipped(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic)?;
    Ok(magic == [0x1f, 0x8b])
}

/// Read all records from any FastxReader into Reference structs.
fn read_all_records(reader: &mut dyn FastxReader) -> Result<Vec<Reference>> {
    let mut refs: Vec<Reference> = Vec::new();

    while let Some(record) = reader.next() {
        let record = record.context("Error reading FASTA record")?;

        let name = std::str::from_utf8(record.id())
            .unwrap_or("<non-utf8-id>")
            .to_string();

        let mut seq: Vec<u8> = record.seq().to_vec();
        seq.make_ascii_uppercase();

        let len = seq.len() as u32;
        if len > 0 {
            refs.push(Reference { name, seq, len });
        }
    }

    Ok(refs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_load_single_fasta() {
        let data = b">chr1\nACGT\nACGT\n>chr2\nTGCA\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = parse_fastx_reader(cursor).unwrap();
        let refs = read_all_records(&mut *reader).unwrap();

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "chr1");
        assert_eq!(refs[0].seq, b"ACGTACGT");
        assert_eq!(refs[1].name, "chr2");
        assert_eq!(refs[1].seq, b"TGCA");
    }

    #[test]
    fn test_load_empty() {
        // needletail 对空文件报错，这是已知行为
        // 验证空输入返回错误而非 panic
        let data = b"";
        let cursor = Cursor::new(&data[..]);
        let result = parse_fastx_reader(cursor);
        assert!(result.is_err(), "空文件应返回解析错误");
    }

    #[test]
    fn test_case_conversion() {
        let data = b">chr1\nacgt\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = parse_fastx_reader(cursor).unwrap();
        let refs = read_all_records(&mut *reader).unwrap();
        assert_eq!(refs[0].seq, b"ACGT");
    }

    #[test]
    fn test_multiline_sequence() {
        let data = b">chr1\nACGT\nTGCA\nACGT\n>chr2\nAAAA\nCCCC\nGGGG\nTTTT\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = parse_fastx_reader(cursor).unwrap();
        let refs = read_all_records(&mut *reader).unwrap();

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].seq, b"ACGTTGCAACGT");
        assert_eq!(refs[1].seq, b"AAAACCCCGGGGTTTT");
    }

    #[test]
    fn test_lowercase_mixed_sequence() {
        let data = b">chr1\nAcGt\naCcG\n";
        let cursor = Cursor::new(&data[..]);
        let mut reader = parse_fastx_reader(cursor).unwrap();
        let refs = read_all_records(&mut *reader).unwrap();
        assert_eq!(refs[0].seq, b"ACGTACCG");
    }
}
