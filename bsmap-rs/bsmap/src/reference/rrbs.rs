//! RRBS (Reduced Representation Bisulfite Sequencing) digestion site handling.
//!
//! Parses enzyme digestion sites (e.g., `C-CGG` for MspI), expands IUPAC
//! ambiguity codes, and finds all cut positions in reference sequences.
//! Mirrors C++ `Param::SetDigestionSite()` and `RefSeq::find_CCGG()`.

use crate::param::SEGLEN;

/// A parsed digestion site with all IUPAC expansions.
#[derive(Debug, Clone)]
pub struct DigestionSite {
    /// All possible sequence strings after IUPAC expansion.
    pub sequences: Vec<String>,
    /// Cut position (index of '-' in original input).
    pub cut_pos: u32,
}

/// IUPAC ambiguity code expansion table.
const IUPAC_CODES: &[(u8, &[u8])] = &[
    (b'A', b"A"),
    (b'C', b"C"),
    (b'G', b"G"),
    (b'T', b"T"),
    (b'N', b"ACGT"),
    (b'R', b"AG"),
    (b'Y', b"CT"),
    (b'S', b"CG"),
    (b'W', b"AT"),
    (b'K', b"GT"),
    (b'M', b"AC"),
    (b'B', b"CGT"),
    (b'D', b"AGT"),
    (b'H', b"ACT"),
    (b'V', b"ACG"),
];

impl DigestionSite {
    /// Parse a digestion site specification like "C-CGG".
    ///
    /// The '-' marks the cut position. IUPAC ambiguity codes in the
    /// sequence are expanded to all possible concrete sequences.
    pub fn parse(spec: &str) -> Option<Self> {
        let dash_pos = spec.find('-')?;
        let mut cleaned = spec.to_string();
        cleaned.remove(dash_pos);
        let cleaned = cleaned.to_uppercase();
        let cut_pos = dash_pos as u32;

        let sequences = expand_iupac(&cleaned);

        Some(Self {
            sequences,
            cut_pos,
        })
    }
}

/// Expand IUPAC ambiguity codes to all possible concrete nucleotide strings.
fn expand_iupac(pattern: &str) -> Vec<String> {
    let bytes = pattern.as_bytes();
    let mut results: Vec<String> = vec![String::new()];

    for &b in bytes {
        let expansions = IUPAC_CODES
            .iter()
            .find(|(code, _)| *code == b.to_ascii_uppercase())
            .map(|(_, exp)| *exp)
            .unwrap_or(b"N"); // Unknown chars → N (ACGT)

        let mut new_results = Vec::with_capacity(results.len() * expansions.len());
        for existing in &results {
            for &nt in expansions {
                let mut s = existing.clone();
                s.push(nt as char);
                new_results.push(s);
            }
        }
        results = new_results;
    }

    results
}

/// Find all digestion sites in a reference sequence.
///
/// Returns vector of (position, rev_offset) for each site found,
/// sorted by position. `position` is the cut site location,
/// `rev_offset` = site_len - 2*min(cut_pos, site_len-cut_pos).
pub fn find_sites(seq: &[u8], sites: &[DigestionSite]) -> Vec<(u32, u32)> {
    let mut results: Vec<(u32, u32)> = Vec::new();

    for site in sites {
        for pattern in &site.sequences {
            let min_offset = site.cut_pos.min(pattern.len() as u32 - site.cut_pos);
            let rev_offset = pattern.len() as u32 - 2 * min_offset;

            // Naive string search — could use KMP/Boyer-Moore for large refs
            let pattern_bytes = pattern.as_bytes();
            let mut search_start = 0;
            while let Some(pos) = seq[search_start..]
                .windows(pattern_bytes.len())
                .position(|w| w.eq_ignore_ascii_case(pattern_bytes))
            {
                let abs_pos = search_start + pos;
                results.push((abs_pos as u32 + min_offset, rev_offset));
                search_start = abs_pos + 1;
            }
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Build RRBS seed index positions per chromosome and seed segment.
///
/// Returns `ccgg_index[max_seed_seg][chr_id]` = Vec of positions.
/// Mirrors C++ `RefSeq::find_CCGG()` CCGG_index creation.
pub fn build_rrbs_index(
    seq: &[u8],
    chr_id: u32,
    chr_size: u32,
    rc_offset: u32,
    seed_size: u32,
    max_seed_seg: usize,
    sites: &[DigestionSite],
    min_insert: u32,
    max_insert: u32,
) -> Vec<Vec<Vec<u32>>> {
    let all_sites = find_sites(seq, sites);
    if all_sites.is_empty() {
        return vec![Vec::new(); max_seed_seg];
    }

    let max_pos = chr_size.saturating_sub(seed_size);

    // Per-seed-segment, per-chain: BSW (chain 0) and BSC (chain 1)
    let mut bsw: Vec<Vec<u32>> = vec![Vec::new(); max_seed_seg];
    let mut bsc: Vec<Vec<u32>> = vec![Vec::new(); max_seed_seg];

    // Forward strand (BSW): iterate j..j+1 pairs
    for j in 0..all_sites.len().saturating_sub(1) {
        let (pos_j, _) = all_sites[j];

        // Find next site that satisfies insert size constraints
        let mut seglen: i64 = 0;
        let mut found = false;
        for i in j + 1..all_sites.len() {
            seglen = (all_sites[i].0 + all_sites[i].1) as i64 - pos_j as i64;
            if seglen >= min_insert as i64 {
                found = true;
                break;
            }
        }
        if !found || seglen < min_insert as i64 || seglen > max_insert as i64 {
            continue;
        }

        for seg in 0..max_seed_seg {
            let mut seedloc = pos_j;
            while seedloc <= max_pos {
                bsw[seg].push(seedloc);
                seedloc += seed_size;
                if seedloc > max_pos || bsw[seg].len() >= 1000 {
                    break;
                }
            }
        }
    }

    // Reverse strand (BSC): iterate j..j-1 pairs
    for j in 1..all_sites.len() {
        let mut seglen: i64 = 0;
        let mut found = false;
        for i in (0..j).rev() {
            seglen =
                (all_sites[j].0 + all_sites[j].1) as i64 - all_sites[i].0 as i64;
            if seglen >= min_insert as i64 {
                found = true;
                break;
            }
        }
        if !found || seglen > max_insert as i64 {
            continue;
        }

        let site_end = all_sites[j].0 + all_sites[j].1;
        for seg in 0..max_seed_seg {
            let mut seedloc = site_end.saturating_sub(seed_size) as i64;
            while seedloc >= 0 {
                if rc_offset < seedloc as u32 {
                    seedloc -= seed_size as i64;
                    continue;
                }
                bsc[seg].push(rc_offset - seedloc as u32);
                seedloc -= seed_size as i64;
                if seedloc < 0 {
                    break;
                }
            }
        }
    }

    // Merge BSW and BSC interleaved per chromosome
    let mut result: Vec<Vec<Vec<u32>>> = vec![Vec::new(); max_seed_seg];
    for seg in 0..max_seed_seg {
        result[seg] = vec![bsw[seg].clone(), bsc[seg].clone()];
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_digestion_site() {
        let site = DigestionSite::parse("C-CGG").unwrap();
        assert_eq!(site.cut_pos, 1);
        assert_eq!(site.sequences, vec!["CCGG"]);
    }

    #[test]
    fn test_parse_with_iupac() {
        // Y = C/T
        let site = DigestionSite::parse("C-YG").unwrap();
        assert_eq!(site.cut_pos, 1);
        assert_eq!(site.sequences.len(), 2); // CCG and CTG
    }

    #[test]
    fn test_expand_iupac_simple() {
        let result = expand_iupac("CCGG");
        assert_eq!(result, vec!["CCGG"]);
    }

    #[test]
    fn test_expand_iupac_ambiguous() {
        let result = expand_iupac("YG");
        // Y→C/T, G fixed → 2 results
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"CG".to_string()));
        assert!(result.contains(&"TG".to_string()));
    }

    #[test]
    fn test_find_sites() {
        let seq = b"ACGTCCGGACGTCCGGCCGG";
        let site = DigestionSite::parse("C-CGG").unwrap();
        let results = find_sites(seq, &[site]);
        // "CCGG" appears at positions 4 and 12, cut at C|CGG → +1
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 5); // first CCGG at pos 4 + cut_pos 1
    }
}
