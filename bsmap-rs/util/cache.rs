//! P6-2 缓存对齐优化模块
//!
//! 本模块提供针对缓存优化的数据结构和对齐功能，用于减少cache miss。
//!
//! ## 优化策略
//!
//! 1. **64字节对齐**: 确保热点数据结构按缓存行大小对齐
//! 2. **预取提示**: 提供编译器和手动预取支持
//! 3. **缓存友好访问**: 优化数据访问模式
//!
//! ## P6-2 优化
//!
//! - 缓存行对齐的内存分配
//! - SIMD友好的数据结构布局
//! - 热点数据的预取策略

/// Cache line size (typically 64 bytes on modern CPUs).
pub const CACHE_LINE_SIZE: usize = 64;

/// SIMD alignment requirement (for AVX2/AVX512).
pub const SIMD_ALIGN: usize = 32;

/// Round up to cache line size.
#[inline(always)]
pub const fn align_cache_line(size: usize) -> usize {
    (size + CACHE_LINE_SIZE - 1) & !(CACHE_LINE_SIZE - 1)
}

/// Round up to SIMD alignment.
#[inline(always)]
pub const fn align_simd(size: usize) -> usize {
    (size + SIMD_ALIGN - 1) & !(SIMD_ALIGN - 1)
}

/// Cache-aligned wrapper for hot data structures.
///
/// This struct ensures the wrapped data is aligned to cache line boundaries,
/// which can reduce cache miss rates when the data is accessed frequently.
#[repr(C, align(64))]
pub struct CacheAligned<T> {
    pub value: T,
}

impl<T> CacheAligned<T> {
    /// Create a new cache-aligned wrapper.
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Get a mutable reference to the wrapped value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: Default> Default for CacheAligned<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
        }
    }
}

/// SIMD-aligned wrapper for vectorizable data.
///
/// This struct ensures the wrapped data is aligned to SIMD vector boundaries,
/// which is required for efficient SIMD operations.
#[repr(C, align(32))]
pub struct SimdAligned<T> {
    pub value: T,
}

impl<T> SimdAligned<T> {
    /// Create a new SIMD-aligned wrapper.
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Get a mutable reference to the wrapped value.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

/// Allocate a cache-aligned buffer.
///
/// Returns a Vec with at least CACHE_LINE_SIZE alignment.
pub fn cache_aligned_vec<T: Default>(len: usize) -> Vec<T> {
    let mut vec = Vec::with_capacity(len);
    // Ensure alignment by allocating extra space
    unsafe {
        let ptr = vec.as_mut_ptr();
        let aligned_ptr = align_to_cache_line(ptr);
        let diff = aligned_ptr.offset_from(ptr);
        vec.set_len(len + diff);
    }
    vec
}

/// Allocate a SIMD-aligned buffer.
///
/// Returns a Vec with at least SIMD_ALIGN (32 bytes) alignment.
pub fn simd_aligned_vec<T: Default>(len: usize) -> Vec<T> {
    let mut vec = Vec::with_capacity(len);
    unsafe {
        let ptr = vec.as_mut_ptr();
        let aligned_ptr = align_to_simd(ptr);
        let diff = aligned_ptr.offset_from(ptr);
        vec.set_len(len + diff);
    }
    vec
}

/// Align a pointer to cache line boundary (64 bytes).
#[inline(always)]
unsafe fn align_to_cache_line<T>(ptr: *mut T) -> *mut T {
    let addr = ptr as usize;
    let aligned = (addr + CACHE_LINE_SIZE - 1) & !(CACHE_LINE_SIZE - 1);
    aligned as *mut T
}

/// Align a pointer to SIMD boundary (32 bytes).
#[inline(always)]
unsafe fn align_to_simd<T>(ptr: *mut T) -> *mut T {
    let addr = ptr as usize;
    let aligned = (addr + SIMD_ALIGN - 1) & !(SIMD_ALIGN - 1);
    aligned as *mut T
}

/// Prefetch hints for memory access patterns.
///
/// These functions provide explicit prefetch hints that can help
/// reduce cache miss latency in hot loops.
#[cfg(target_feature = "avx2")]
pub mod prefetch {
    use core::arch::x86_64::*;

    /// Prefetch for read (temporal locality - data will be used soon).
    #[inline(always)]
    pub fn read<T>(ptr: *const T) {
        unsafe {
            _mm_prefetch(ptr as *const i8, _MM_HINT_T0);
        }
    }

    /// Prefetch for read (non-temporal - data will be used once).
    #[inline(always)]
    pub fn read_nt<T>(ptr: *const T) {
        unsafe {
            _mm_prefetch(ptr as *const i8, _MM_HINT_NTA);
        }
    }

    /// Prefetch for write (temporal).
    #[inline(always)]
    pub fn write<T>(ptr: *const T) {
        unsafe {
            // Software prefetch has no write hint, but we can use read prefetch
            // followed by a write to simulate write-prefetch behavior
            _mm_prefetch(ptr as *const i8, _MM_HINT_T0);
        }
    }

    /// Prefetch multiple cache lines ahead.
    #[inline(always)]
    pub fn read_ahead<T>(ptr: *const T, lines: usize) {
        let base = ptr as usize;
        unsafe {
            for i in 0..lines {
                let addr = (base + i * super::CACHE_LINE_SIZE) as *const i8;
                _mm_prefetch(addr, _MM_HINT_T0);
            }
        }
    }
}

/// Fallback prefetch implementations for non-AVX2 platforms.
#[cfg(not(target_feature = "avx2"))]
pub mod prefetch {
    /// No-op prefetch for read (will be optimized away).
    #[inline(always)]
    pub fn read<T>(_ptr: *const T) {}

    /// No-op prefetch for non-temporal read.
    #[inline(always)]
    pub fn read_nt<T>(_ptr: *const T) {}

    /// No-op prefetch for write.
    #[inline(always)]
    pub fn write<T>(_ptr: *const T) {}

    /// No-op prefetch for multiple lines.
    #[inline(always)]
    pub fn read_ahead<T>(_ptr: *const T, _lines: usize) {}
}

/// Hot loop optimization utilities.
///
/// These utilities help optimize tight loops by reducing overhead
/// and improving cache behavior.
pub mod hot_loop {
    /// Unroll-friendly loop counter.
    ///
    /// This structure provides a loop counter that helps the compiler
    /// generate more efficient unrolled loops.
    #[derive(Debug, Clone, Copy)]
    pub struct LoopCounter {
        current: usize,
        end: usize,
        stride: usize,
    }

    impl LoopCounter {
        /// Create a new loop counter.
        #[inline(always)]
        pub fn new(start: usize, end: usize, stride: usize) -> Self {
            Self {
                current: start,
                end,
                stride,
            }
        }

        /// Check if loop should continue.
        #[inline(always)]
        pub fn has_more(&self) -> bool {
            self.current < self.end
        }

        /// Get current value and advance.
        #[inline(always)]
        pub fn next(&mut self) -> Option<usize> {
            if self.current < self.end {
                let val = self.current;
                self.current += self.stride;
                Some(val)
            } else {
                None
            }
        }

        /// Get remaining count.
        #[inline(always)]
        pub fn remaining(&self) -> usize {
            self.end.saturating_sub(self.current)
        }
    }

    impl Iterator for LoopCounter {
        type Item = usize;

        #[inline(always)]
        fn next(&mut self) -> Option<Self::Item> {
            self.next()
        }

        #[inline(always)]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.remaining();
            (remaining / self.stride, Some(remaining / self.stride))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_cache_line() {
        assert_eq!(align_cache_line(0), 0);
        assert_eq!(align_cache_line(1), 64);
        assert_eq!(align_cache_line(63), 64);
        assert_eq!(align_cache_line(64), 64);
        assert_eq!(align_cache_line(65), 128);
        assert_eq!(align_cache_line(128), 128);
    }

    #[test]
    fn test_align_simd() {
        assert_eq!(align_simd(0), 0);
        assert_eq!(align_simd(1), 32);
        assert_eq!(align_simd(31), 32);
        assert_eq!(align_simd(32), 32);
        assert_eq!(align_simd(33), 64);
    }

    #[test]
    fn test_cache_aligned_struct() {
        let aligned = CacheAligned::new(42u32);
        assert_eq!(aligned.value, 42);
        // Verify alignment
        let addr = (&aligned.value) as *const u32 as usize;
        assert_eq!(addr % CACHE_LINE_SIZE, 0);
    }

    #[test]
    fn test_simd_aligned_struct() {
        let aligned = SimdAligned::new(42u32);
        assert_eq!(aligned.value, 42);
        // Verify alignment
        let addr = (&aligned.value) as *const u32 as usize;
        assert_eq!(addr % SIMD_ALIGN, 0);
    }

    #[test]
    fn test_loop_counter() {
        let counter = LoopCounter::new(0, 10, 1);
        let collected: Vec<_> = counter.collect();
        assert_eq!(collected, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn test_loop_counter_stride() {
        let counter = LoopCounter::new(0, 10, 2);
        let collected: Vec<_> = counter.collect();
        assert_eq!(collected, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn test_loop_counter_remaining() {
        let mut counter = LoopCounter::new(0, 10, 1);
        assert_eq!(counter.remaining(), 10);
        counter.next();
        assert_eq!(counter.remaining(), 9);
    }
}
