//! Index prefetching/warming utilities.
//!
//! Preloads index data into memory/caches to eliminate page fault overhead
//! during alignment. This is especially effective for mmap-based index loading.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::reference::storage::{KmerIndexStorage, BinSeqStorage};

/// Progress callback for index prefetching.
pub type PrefetchCallback = Box<dyn Fn(usize, usize) + Send + Sync>;

/// Simple progress tracker for prefetching.
pub struct PrefetchProgress {
    total: AtomicUsize,
    current: AtomicUsize,
    callback: PrefetchCallback,
}

impl PrefetchProgress {
    pub fn new(total: usize, callback: PrefetchCallback) -> Self {
        Self {
            total: AtomicUsize::new(total),
            current: AtomicUsize::new(0),
            callback,
        }
    }

    #[inline]
    pub fn update(&self, chunk_idx: usize) {
        self.current.store(chunk_idx, Ordering::Relaxed);
        (self.callback)(chunk_idx, self.total.load(Ordering::Relaxed));
    }
}

/// Prefetch configuration.
#[derive(Debug, Clone)]
pub struct PrefetchConfig {
    /// Enable prefetching.
    pub enabled: bool,
    /// Chunk size for sequential prefetch (in elements).
    pub chunk_size: usize,
    /// Number of worker threads for parallel prefetch.
    pub num_threads: usize,
    /// Verbose logging.
    pub verbose: bool,
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            chunk_size: 1024,
            num_threads: 1,
            verbose: false,
        }
    }
}

/// Warm up the index by sequentially accessing all data.
///
/// This triggers page faults for all mmap regions, loading the index
/// into physical memory before alignment begins.
pub fn warm_index(
    kmer_storage: &Arc<dyn KmerIndexStorage>,
    refcat_storage: &dyn BinSeqStorage,
    crefcat_storage: &dyn BinSeqStorage,
    config: &PrefetchConfig,
) {
    if !config.enabled {
        return;
    }

    if config.verbose {
        log::info!("开始索引预热...");
    }

    let total_elements = kmer_storage.index2_len()
        + kmer_storage.positions_len()
        + kmer_storage.start_offsets_len()
        + refcat_storage.len()
        + crefcat_storage.len();

    if config.verbose {
        log::info!("索引总计 {} 个元素", total_elements);
    }

    let chunk_size = config.chunk_size;

    // Sequential prefetch of index2 entries
    let index2_len = kmer_storage.index2_len();
    for chunk_start in (0..index2_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(index2_len);
        for i in chunk_start..chunk_end {
            let _ = kmer_storage.get_index2_entry(i);
        }
        if config.verbose && chunk_start % (chunk_size * 4) == 0 {
            log::debug!("预热进度: {}/{}", chunk_end, index2_len);
        }
    }

    // Prefetch positions
    let positions = kmer_storage.as_positions_slice();
    let positions_len = positions.len();
    for chunk_start in (0..positions_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(positions_len);
        let _chunk = &positions[chunk_start..chunk_end];
    }

    // Prefetch start_offsets
    let start_offsets = kmer_storage.as_start_offsets_slice();
    let offsets_len = start_offsets.len();
    for chunk_start in (0..offsets_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(offsets_len);
        let _chunk = &start_offsets[chunk_start..chunk_end];
    }

    // Prefetch refcat
    let refcat = refcat_storage.as_slice();
    let refcat_len = refcat.len();
    for chunk_start in (0..refcat_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(refcat_len);
        let _chunk = &refcat[chunk_start..chunk_end];
    }

    // Prefetch crefcat
    let crefcat = crefcat_storage.as_slice();
    let crefcat_len = crefcat.len();
    for chunk_start in (0..crefcat_len).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(crefcat_len);
        let _chunk = &crefcat[chunk_start..chunk_end];
    }

    if config.verbose {
        log::info!("索引预热完成");
    }
}

/// Parallel warm up using multiple threads.
pub fn warm_index_parallel(
    kmer_storage: &Arc<dyn KmerIndexStorage>,
    refcat_storage: &dyn BinSeqStorage,
    crefcat_storage: &dyn BinSeqStorage,
    config: &PrefetchConfig,
) {
    use rayon::prelude::*;

    if !config.enabled {
        return;
    }

    if config.verbose {
        log::info!("开始并行索引预热...");
    }

    let chunk_size = config.chunk_size;
    let num_threads = config.num_threads.max(1);

    // Prefetch index2 entries in parallel
    let index2_len = kmer_storage.index2_len();
    let index2_chunks: Vec<usize> = (0..index2_len)
        .step_by(chunk_size)
        .collect();

    index2_chunks.par_iter().for_each(|&chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(index2_len);
        for i in chunk_start..chunk_end {
            let _ = kmer_storage.get_index2_entry(i);
        }
    });

    // Prefetch other data
    let positions = kmer_storage.as_positions_slice();
    let positions_chunks: Vec<usize> = (0..positions.len())
        .step_by(chunk_size)
        .collect();

    positions_chunks.par_iter().for_each(|&chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(positions.len());
        let _ = &positions[chunk_start..chunk_end];
    });

    let start_offsets = kmer_storage.as_start_offsets_slice();
    let offsets_chunks: Vec<usize> = (0..start_offsets.len())
        .step_by(chunk_size)
        .collect();

    offsets_chunks.par_iter().for_each(|&chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(start_offsets.len());
        let _ = &start_offsets[chunk_start..chunk_end];
    });

    let refcat = refcat_storage.as_slice();
    let refcat_chunks: Vec<usize> = (0..refcat.len())
        .step_by(chunk_size)
        .collect();

    refcat_chunks.par_iter().for_each(|&chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(refcat.len());
        let _ = &refcat[chunk_start..chunk_end];
    });

    let crefcat = crefcat_storage.as_slice();
    let crefcat_chunks: Vec<usize> = (0..crefcat.len())
        .step_by(chunk_size)
        .collect();

    crefcat_chunks.par_iter().for_each(|&chunk_start| {
        let chunk_end = (chunk_start + chunk_size).min(crefcat.len());
        let _ = &crefcat[chunk_start..chunk_end];
    });

    if config.verbose {
        log::info!("并行索引预热完成");
    }
}

/// Create a default prefetch config based on system resources.
pub fn auto_config() -> PrefetchConfig {
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    PrefetchConfig {
        enabled: true,
        chunk_size: 4096,
        num_threads: num_cpus,
        verbose: log::max_level() == log::Level::Debug,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::storage::{VecKmerIndexStorage, VecStorage};

    #[test]
    fn test_warm_index_vec_storage() {
        let index2: Vec<crate::param::KmerLoc2> = (0..100)
            .map(|i| crate::param::KmerLoc2 {
                n: [i as u32, (i * 2) as u32],
                loc1: None,
            })
            .collect();

        let kmer_storage: Arc<dyn KmerIndexStorage> = Arc::new(VecKmerIndexStorage::new(
            index2,
            vec![0u32; 100],
            vec![0u32; 100],
        ));

        let refcat_storage: Arc<dyn BinSeqStorage> = Arc::new(VecStorage::new(
            vec![0u64; 1000],
        ));

        let crefcat_storage: Arc<dyn BinSeqStorage> = Arc::new(VecStorage::new(
            vec![0u64; 1000],
        ));

        let config = PrefetchConfig {
            enabled: true,
            chunk_size: 10,
            num_threads: 1,
            verbose: false,
        };

        warm_index(&kmer_storage, refcat_storage.as_ref(), crefcat_storage.as_ref(), &config);
    }

    #[test]
    fn test_auto_config() {
        let config = auto_config();
        assert!(config.enabled);
        assert!(config.num_threads >= 1);
        assert!(config.chunk_size > 0);
    }
}
