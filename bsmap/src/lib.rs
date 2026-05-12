//! BSMAP-rs: Bisulfite Sequence MAPping in Rust.
//!
//! A high-performance bisulfite sequencing read aligner supporting
//! WGBS, RRBS, single-end and paired-end alignment with
//! C→T tolerant mapping.

pub mod align;
pub mod alphabet;
pub mod cli;
pub mod pairs;
pub mod param;
pub mod reads;
pub mod reference;
pub mod utils;
