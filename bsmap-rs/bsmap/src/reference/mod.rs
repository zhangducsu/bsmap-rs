//! Reference genome handling: FASTA loading, 2-bit binary encoding,
//! k-mer seed index construction (WGBS and RRBS modes).
//!
//! Mirrors C++ `dbseq.h` / `dbseq.cpp`.

pub mod fasta;
pub mod binseq;
pub mod index;
pub mod index_io;
pub mod rrbs;
pub mod storage;

pub use fasta::{Reference, ReferenceReader};
pub use binseq::{BinarySeq, BinSeqCollection, BinSeqCollectionBuilder, Block};
pub use index::{KmerIndex, RrbsIndexBuilder};
pub use index_io::{default_index_path, is_index_compatible, load_index, load_index_with_mode, save_index, save_index_v2, IndexMeta, IndexParameters, LoadMode};
pub use storage::{BinSeqStorage, VecStorage, MmapStorage};
