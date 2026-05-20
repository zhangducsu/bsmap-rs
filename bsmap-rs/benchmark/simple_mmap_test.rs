
use bsmap::reference::{index_io::load_index_with_mode, index_io::LoadMode};
use std::path::Path;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("=== Simple Mmap Test ===\n");
    
    let index_path = Path::new("data/chr22_tail_1M.fa.bsi");
    
    // 1. Load index
    println!("Step 1: Loading index (mmap mode)...");
    let (coll, index, _meta) = match load_index_with_mode(index_path, LoadMode::Mmap) {
        Ok(result) => {
            println!("✓ Index loaded successfully");
            result
        }
        Err(e) => {
            eprintln!("✗ Failed to load index: {}", e);
            return;
        }
    };
    println!();
    
    // 2. Test get_index2_entry
    println!("Step 2: Testing index2 access...");
    for i in 0..10 {
        if let Some(storage) = &index.storage {
            if let Some((n0, n1)) = storage.get_index2_entry(i) {
                println!("  [{}] n0={}, n1={}", i, n0, n1);
            }
        }
    }
    println!("✓ index2 access OK");
    println!();
    
    // 3. Test positions access
    println!("Step 3: Testing positions access...");
    let positions = index.get_positions_slice();
    println!("  positions.len() = {}", positions.len());
    if positions.len() > 10 {
        for i in 0..10 {
            print!("  [{}] = {}", i, positions[i]);
        }
        println!();
    }
    println!("✓ positions access OK");
    println!();
    
    // 4. Test start_offsets access
    println!("Step 4: Testing start_offsets access...");
    let start_offsets = index.get_start_offsets_slice();
    println!("  start_offsets.len() = {}", start_offsets.len());
    if start_offsets.len() > 10 {
        for i in 0..10 {
            print!("  [{}] = {}", i, start_offsets[i]);
        }
        println!();
    }
    println!("✓ start_offsets access OK");
    println!();
    
    // 5. Test lookup_separated
    println!("Step 5: Testing lookup_separated...");
    let (fwd, rev) = index.lookup_separated(0);
    println!("  lookup(0): fwd={}, rev={}", fwd.len(), rev.len());
    println!("✓ lookup_separated OK");
    println!();
    
    // 6. Test refcat access
    println!("Step 6: Testing refcat access...");
    let refcat_slice = coll.refcat.as_slice();
    println!("  refcat.len() = {}", refcat_slice.len());
    println!("✓ refcat access OK");
    println!();
    
    // 7. Test crefcat access
    println!("Step 7: Testing crefcat access...");
    let crefcat_slice = coll.crefcat.as_slice();
    println!("  crefcat.len() = {}", crefcat_slice.len());
    println!("✓ crefcat access OK");
    println!();
    
    println!("=== All tests passed! ===");
}
