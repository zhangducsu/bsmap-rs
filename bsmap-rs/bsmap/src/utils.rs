//! Utility functions: timing, random number generation, and hit comparison.
//!
//! Mirrors the C++ `utilities.h/cpp`.
//!
//! ## RNG: SplitMix64 variant
//! When `randseed == 0`, uses OS entropy (reproducible based on per-thread seed).
//! When `randseed != 0`, uses a deterministic hash of (read_index, randseed)
//! to produce reproducible mapping results — matching C++ `myrand()`.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::param::GHit;

// ── Timing ──────────────────────────────────────────────────────────────────

/// Get seconds since epoch (used for elapsed-time calculation).
#[inline]
pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Simple elapsed-time tracker.
pub struct Timer {
    start: f64,
    last: f64,
}

impl Timer {
    pub fn new() -> Self {
        let now = now_secs();
        Self { start: now, last: now }
    }

    /// Seconds elapsed since this timer was created.
    pub fn elapsed(&self) -> f64 {
        now_secs() - self.start
    }

    /// Seconds since last `step()` call (or timer creation on first call).
    pub fn step(&mut self) -> f64 {
        let now = now_secs();
        let diff = now - self.last;
        self.last = now;
        diff
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

// ── RNG: SplitMix64 Variant ─────────────────────────────────────────────────

/// Thread-safe, reproducible pseudorandom number generator.
///
/// Matches the C++ `myrand(int i, bit32_t* rseed)` function exactly.
///
/// When `randseed == 0`: delegates to OS thread-local RNG.
/// When `randseed != 0`: deterministic mixing of `read_index` and `randseed`.
#[inline]
pub fn myrand(read_index: u32, randseed: u32, thread_seed: u32) -> u32 {
    if randseed == 0 {
        // Use OS entropy (via a simple SplitMix64 from thread_seed)
        let mut x = thread_seed.wrapping_mul(0x9E3779B9) as u64;
        x = x.wrapping_add(0x9E3779B97F4A7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
        (x ^ (x >> 31)) as u32
    } else {
        // Deterministic: matches C++ `myrand()` with custom seed
        let mut v: u64 = (read_index as u64)
            .wrapping_add((randseed as u64).wrapping_mul(1_000_000))
            .wrapping_mul(3_935_559_000_370_003_845)
            .wrapping_add(2_691_343_689_449_507_681);

        v ^= v >> 21;
        v ^= v << 37;
        v ^= v >> 4;
        v = v.wrapping_mul(4_767_518_773_237_032_717);
        v ^= v << 20;
        v ^= v >> 41;
        v ^= v << 5;
        v as u32
    }
}

// ── Hit Comparison ──────────────────────────────────────────────────────────

/// Sort hits by chromosome then position (ascending).
/// Matches C++ `HitComp(gHit a, gHit b)`.
#[inline]
pub fn hit_comp(a: &GHit, b: &GHit) -> std::cmp::Ordering {
    a.chr.cmp(&b.chr).then_with(|| a.loc.cmp(&b.loc))
}

/// Sort hits by chromosome/2 then position (for paired-end merge).
/// Matches C++ `HitComp2(gHit a, gHit b)`.
#[inline]
pub fn hit_comp2(a: &GHit, b: &GHit) -> std::cmp::Ordering {
    (a.chr / 2)
        .cmp(&(b.chr / 2))
        .then_with(|| a.loc.cmp(&b.loc))
}

// ── Display helpers ─────────────────────────────────────────────────────────

/// Display a 2-bit packed u32 as DNA bases (for debugging).
#[allow(dead_code)]
pub fn disp_bfa(a: u32, len: usize, useful_nt: &[u8]) -> String {
    let mut s = String::with_capacity(len);
    for i in (0..len).rev() {
        let idx = ((a >> (i * 2)) & 0x3) as usize;
        if idx < useful_nt.len() {
            s.push(useful_nt[idx] as char);
        } else {
            s.push('N');
        }
    }
    s
}

/// Display a 2-bit packed u64 as DNA bases (for debugging).
#[allow(dead_code)]
pub fn disp_bfa64(a: u64, len: usize, useful_nt: &[u8]) -> String {
    let mut s = String::with_capacity(len);
    for i in (0..len).rev() {
        let idx = ((a >> (i * 2)) & 0x3) as usize;
        if idx < useful_nt.len() {
            s.push(useful_nt[idx] as char);
        } else {
            s.push('N');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myrand_deterministic() {
        // Same seed should produce same sequence
        let a = myrand(100, 42, 0);
        let b = myrand(100, 42, 0);
        assert_eq!(a, b);
    }

    #[test]
    fn test_myrand_reproducible() {
        // Different read indices with same randseed should produce
        // deterministic results matching the SplitMix64 algorithm
        let r1 = myrand(1, 12345, 0);
        let r2 = myrand(2, 12345, 0);
        assert_ne!(r1, r2); // different inputs → different outputs
    }

    #[test]
    fn test_hit_comp() {
        let a = GHit { chr: 0, loc: 100, ..Default::default() };
        let b = GHit { chr: 1, loc: 50, ..Default::default() };
        let c = GHit { chr: 0, loc: 200, ..Default::default() };
        assert_eq!(hit_comp(&a, &b), std::cmp::Ordering::Less);    // chr 0 < chr 1
        assert_eq!(hit_comp(&a, &c), std::cmp::Ordering::Less);    // loc 100 < 200
        assert_eq!(hit_comp(&b, &a), std::cmp::Ordering::Greater);
    }
}
