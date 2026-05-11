//! BSMAP-rs: Bisulfite Sequence MAPping — ultra-fast BS-seq aligner in Rust.
//!
//! Entry point. Parses CLI, builds reference index, and dispatches
//! single-end or paired-end alignment.

use std::process;

use anyhow::Result;
use clap::Parser;
use log::{info, LevelFilter};

use bsmap::cli::Cli;

fn main() -> Result<()> {
    // Parse and validate CLI
    let cli = Cli::parse();

    // Check validation errors
    let errors = cli.validate();
    if !errors.is_empty() {
        eprintln!("Error: invalid arguments:");
        for err in &errors {
            eprintln!("  - {}", err);
        }
        process::exit(1);
    }

    // Initialize logging
    let log_level = match cli.verbose {
        0 => LevelFilter::Off,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    info!("BSMAP-rs v{}", env!("CARGO_PKG_VERSION"));
    info!("Query file: {}", cli.query_a.display());
    if let Some(ref qb) = cli.query_b {
        info!("Query file (mate): {}", qb.display());
    }
    info!("Reference: {}", cli.reference.display());

    // Placeholder: alignment pipeline will be wired up in Phase 1-3
    info!("Phase 0 scaffolding complete. Alignment engine coming in next phases.");

    Ok(())
}
