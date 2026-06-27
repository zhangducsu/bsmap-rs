//! Opt-in profiling support for RRBS server benchmarks.

use std::ffi::OsStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RrbsProfileMode {
    Off,
    Stage,
    Counts,
}

pub struct RrbsProfile {
    mode: RrbsProfileMode,
    pub segment_calls: AtomicU64,
    pub nonempty_seed_buckets: AtomicU64,
    pub raw_bucket_candidates: AtomicU64,
    pub logical_bucket_candidates: AtomicU64,
    pub mode_matched_candidates: AtomicU64,
    pub mismatch_calls: AtomicU64,
    pub accepted_hits: AtomicU64,
    pub gap_attempts: AtomicU64,
    pub gap_accepted_hits: AtomicU64,
    pub early_stops: AtomicU64,
}

impl RrbsProfile {
    fn new(mode: RrbsProfileMode) -> Self {
        Self {
            mode,
            segment_calls: AtomicU64::new(0),
            nonempty_seed_buckets: AtomicU64::new(0),
            raw_bucket_candidates: AtomicU64::new(0),
            logical_bucket_candidates: AtomicU64::new(0),
            mode_matched_candidates: AtomicU64::new(0),
            mismatch_calls: AtomicU64::new(0),
            accepted_hits: AtomicU64::new(0),
            gap_attempts: AtomicU64::new(0),
            gap_accepted_hits: AtomicU64::new(0),
            early_stops: AtomicU64::new(0),
        }
    }

    #[cfg(test)]
    fn new_enabled_for_test() -> Self {
        Self::new(RrbsProfileMode::Counts)
    }

    #[inline]
    pub fn add(&self, counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    fn get(counter: &AtomicU64) -> u64 {
        counter.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> RrbsProfileSnapshot {
        RrbsProfileSnapshot {
            segment_calls: Self::get(&self.segment_calls),
            nonempty_seed_buckets: Self::get(&self.nonempty_seed_buckets),
            raw_bucket_candidates: Self::get(&self.raw_bucket_candidates),
            logical_bucket_candidates: Self::get(&self.logical_bucket_candidates),
            mode_matched_candidates: Self::get(&self.mode_matched_candidates),
            mismatch_calls: Self::get(&self.mismatch_calls),
            accepted_hits: Self::get(&self.accepted_hits),
            gap_attempts: Self::get(&self.gap_attempts),
            gap_accepted_hits: Self::get(&self.gap_accepted_hits),
            early_stops: Self::get(&self.early_stops),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RrbsProfileSnapshot {
    pub segment_calls: u64,
    pub nonempty_seed_buckets: u64,
    pub raw_bucket_candidates: u64,
    pub logical_bucket_candidates: u64,
    pub mode_matched_candidates: u64,
    pub mismatch_calls: u64,
    pub accepted_hits: u64,
    pub gap_attempts: u64,
    pub gap_accepted_hits: u64,
    pub early_stops: u64,
}

static RRBS_PROFILE: OnceLock<RrbsProfile> = OnceLock::new();

fn parse_rrbs_profile_mode(value: Option<&OsStr>) -> RrbsProfileMode {
    let Some(value) = value else {
        return RrbsProfileMode::Off;
    };
    let Some(value) = value.to_str() else {
        return RrbsProfileMode::Off;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "0" | "false" | "off" | "none" => RrbsProfileMode::Off,
        "stage" | "stages" | "timing" | "timings" => RrbsProfileMode::Stage,
        _ => RrbsProfileMode::Counts,
    }
}

fn rrbs_profile_state() -> &'static RrbsProfile {
    RRBS_PROFILE.get_or_init(|| {
        let mode = parse_rrbs_profile_mode(std::env::var_os("BSMAP_PROFILE_RRBS").as_deref());
        RrbsProfile::new(mode)
    })
}

pub fn rrbs_stage_profile_enabled() -> bool {
    !matches!(rrbs_profile_state().mode, RrbsProfileMode::Off)
}

pub fn rrbs_profile() -> Option<&'static RrbsProfile> {
    let profile = rrbs_profile_state();
    matches!(profile.mode, RrbsProfileMode::Counts).then_some(profile)
}

pub fn print_rrbs_profile_if_enabled() {
    let Some(profile) = rrbs_profile() else {
        return;
    };
    let snapshot = profile.snapshot();
    eprintln!("BSMAP_PROFILE_RRBS segment_calls={}", snapshot.segment_calls);
    eprintln!(
        "BSMAP_PROFILE_RRBS nonempty_seed_buckets={}",
        snapshot.nonempty_seed_buckets
    );
    eprintln!(
        "BSMAP_PROFILE_RRBS raw_bucket_candidates={}",
        snapshot.raw_bucket_candidates
    );
    eprintln!(
        "BSMAP_PROFILE_RRBS logical_bucket_candidates={}",
        snapshot.logical_bucket_candidates
    );
    eprintln!(
        "BSMAP_PROFILE_RRBS mode_matched_candidates={}",
        snapshot.mode_matched_candidates
    );
    eprintln!("BSMAP_PROFILE_RRBS mismatch_calls={}", snapshot.mismatch_calls);
    eprintln!("BSMAP_PROFILE_RRBS accepted_hits={}", snapshot.accepted_hits);
    eprintln!("BSMAP_PROFILE_RRBS gap_attempts={}", snapshot.gap_attempts);
    eprintln!(
        "BSMAP_PROFILE_RRBS gap_accepted_hits={}",
        snapshot.gap_accepted_hits
    );
    eprintln!("BSMAP_PROFILE_RRBS early_stops={}", snapshot.early_stops);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reports_relaxed_counter_values() {
        let profile = RrbsProfile::new_enabled_for_test();
        profile.add(&profile.segment_calls, 2);
        profile.add(&profile.mismatch_calls, 7);
        profile.add(&profile.accepted_hits, 3);

        let snapshot = profile.snapshot();

        assert_eq!(snapshot.segment_calls, 2);
        assert_eq!(snapshot.mismatch_calls, 7);
        assert_eq!(snapshot.accepted_hits, 3);
        assert_eq!(snapshot.raw_bucket_candidates, 0);
    }

    #[test]
    fn profile_mode_parsing_separates_stage_from_counts() {
        assert_eq!(parse_rrbs_profile_mode(None), RrbsProfileMode::Off);
        assert_eq!(parse_rrbs_profile_mode(Some(OsStr::new(""))), RrbsProfileMode::Off);
        assert_eq!(parse_rrbs_profile_mode(Some(OsStr::new("0"))), RrbsProfileMode::Off);
        assert_eq!(
            parse_rrbs_profile_mode(Some(OsStr::new("stage"))),
            RrbsProfileMode::Stage
        );
        assert_eq!(
            parse_rrbs_profile_mode(Some(OsStr::new("timing"))),
            RrbsProfileMode::Stage
        );
        assert_eq!(
            parse_rrbs_profile_mode(Some(OsStr::new("1"))),
            RrbsProfileMode::Counts
        );
        assert_eq!(
            parse_rrbs_profile_mode(Some(OsStr::new("counts"))),
            RrbsProfileMode::Counts
        );
    }
}
