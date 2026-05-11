//! BSMAP-rs: Bisulfite Sequence MAPping in Rust.
//!
//! A high-performance bisulfite sequencing read aligner supporting
//! WGBS, RRBS, single-end and paired-end alignment with
//! C→T tolerant mapping.

pub mod alphabet;
pub mod cli;
pub mod param;
pub mod reference;
pub mod utils;

// Future modules:
// pub mod reads;
// pub mod align;
// pub mod pairs;
