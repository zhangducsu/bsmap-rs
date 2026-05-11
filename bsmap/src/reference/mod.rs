//! Reference genome handling: FASTA loading, 2-bit binary encoding,
//! k-mer seed index construction (WGBS and RRBS modes).
//!
//! Mirrors C++ `dbseq.h` / `dbseq.cpp`.

pub mod fasta;
pub mod binseq;
pub mod index;
pub mod rrbs;

pub use fasta::Reference;
pub use binseq::{BinarySeq, BinSeqCollection, Block};
pub use index::KmerIndex;
