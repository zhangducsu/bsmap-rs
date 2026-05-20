
use bsmap::reference::{index_io::load_index_with_mode, index_io::LoadMode};
use std::path::Path;
use std::time::Instant;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    
    let index_path = Path::new("data/chr22_tail_1M.fa.bsi");
    
    println!("Step 1: Loading index in mmap mode...");
    let start = Instant::now();
    let (coll, index, _meta) = load_index_with_mode(index_path, LoadMode::Mmap).unwrap();
    let load_duration = start.elapsed();
    println!("✓ Index loaded in {:.2}s", load_duration.as_secs_f64());
    
    println!("Step 2: Testing simple lookup...");
    let test_hash = 0u32;
    let (fwd, rev) = index.lookup_separated(test_hash);
    println!("✓ Lookup successful: fwd={}, rev={}", fwd.len(), rev.len());
    
    println!("Step 3: Testing refcat access...");
    let refcat_slice = coll.refcat.as_slice();
    println!("✓ Refcat access successful: len={}", refcat_slice.len());
    
    println!("Step 4: Testing crefcat access...");
    let crefcat_slice = coll.crefcat.as_slice();
    println!("✓ Crefcat access successful: len={}", crefcat_slice.len());
    
    println!("\n✅ All mmap tests passed!");
}
