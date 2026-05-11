//! FASTA reference genome loader.
//!
//! Loads multi-FASTA files (plain or gzipped), returning per-chromosome
//! sequences as uppercase byte vectors. Mirrors C++ `RefSeq::LoadNextSeq()`.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::bufread::GzDecoder;

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

/// Load all sequences from a FASTA file (plain or gzipped).
///
/// If `is_gz` is true, decompresses via gzip on the fly.
pub fn load_fasta(path: &Path, is_gz: bool) -> Result<Vec<Reference>> {
    let file = File::open(path)
        .with_context(|| format!("Cannot open reference: {}", path.display()))?;

    if is_gz {
        load_fasta_reader(BufReader::new(GzDecoder::new(BufReader::new(file))))
    } else {
        load_fasta_reader(BufReader::new(file))
    }
}

/// Detect whether a FASTA file is gzipped by checking the magic bytes.
pub fn is_gzipped(path: &Path) -> Result<bool> {
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic)?;
    Ok(magic == [0x1f, 0x8b])
}

fn load_fasta_reader<R: BufRead>(mut reader: R) -> Result<Vec<Reference>> {
    let mut refs: Vec<Reference> = Vec::new();
    let mut line = String::new();
    let mut current_name = String::new();
    let mut current_seq: Vec<u8> = Vec::new();

    // Read first character to check for '>'
    let mut buf = [0u8; 1];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let c = buf[0];
                if c == b'>' {
                    // Save previous sequence if any
                    if !current_name.is_empty() {
                        refs.push(Reference {
                            name: std::mem::take(&mut current_name),
                            len: current_seq.len() as u32,
                            seq: std::mem::take(&mut current_seq),
                        });
                    }
                    line.clear();
                    reader.read_line(&mut line)?;
                    current_name = line.trim().to_string();
                } else if c == b'\n' || c == b'\r' {
                    // Skip whitespace
                    continue;
                } else {
                    // Sequence line: read one byte, then read rest of line
                    reader.read_line(&mut line)?;
                    let trimmed = line.trim();
                    current_seq.extend_from_slice(trimmed.as_bytes());
                }
            }
            Err(e) => return Err(e.into()),
        }
        line.clear();
    }

    // Don't forget the last sequence
    if !current_name.is_empty() {
        refs.push(Reference {
            name: current_name,
            len: current_seq.len() as u32,
            seq: current_seq,
        });
    }

    // Convert all sequences to uppercase
    for r in &mut refs {
        r.seq.make_ascii_uppercase();
        r.len = r.seq.len() as u32;
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
        let reader = BufReader::new(Cursor::new(&data[..]));
        let refs = load_fasta_reader(reader).unwrap();

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "chr1");
        assert_eq!(refs[0].seq, b"ACGTACGT");
        assert_eq!(refs[1].name, "chr2");
        assert_eq!(refs[1].seq, b"TGCA");
    }

    #[test]
    fn test_load_empty() {
        let data = b"";
        let reader = BufReader::new(Cursor::new(&data[..]));
        let refs = load_fasta_reader(reader).unwrap();
        assert!(refs.is_empty());
    }

    #[test]
    fn test_case_conversion() {
        let data = b">chr1\nacgt\n";
        let reader = BufReader::new(Cursor::new(&data[..]));
        let refs = load_fasta_reader(reader).unwrap();
        assert_eq!(refs[0].seq, b"ACGT");
    }
}
