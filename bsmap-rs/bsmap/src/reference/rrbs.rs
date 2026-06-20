//! RRBS digestion site handling.
//!
//! Parses enzyme digestion sites, expands IUPAC ambiguity codes, and builds
//! C++-compatible RRBS seed position buckets.

use crate::param::SEGLEN;

/// A parsed digestion site with all IUPAC expansions.
#[derive(Debug, Clone)]
pub struct DigestionSite {
    /// All possible sequence strings after IUPAC expansion.
    pub sequences: Vec<String>,
    /// Cut position (index of '-' in original input).
    pub cut_pos: u32,
}

/// RRBS digestion positions grouped as `mode -> chain -> positions`.
pub type RrbsModeIndex = Vec<Vec<Vec<u32>>>;

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
    /// The '-' marks the cut position. IUPAC ambiguity codes in the sequence
    /// are expanded to all possible concrete sequences.
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
            .unwrap_or(b"N");

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
/// Returns `(cut_position, reverse_offset)` sorted by cut position.
pub fn find_sites(seq: &[u8], sites: &[DigestionSite]) -> Vec<(u32, u32)> {
    let mut results: Vec<(u32, u32)> = Vec::new();

    for site in sites {
        for pattern in &site.sequences {
            let min_offset = site.cut_pos.min(pattern.len() as u32 - site.cut_pos);
            let rev_offset = pattern.len() as u32 - 2 * min_offset;
            let pattern_bytes = pattern.as_bytes();

            // C++ RefSeq::find_CCGG starts searching at offset 1.
            let mut search_start = 1.min(seq.len());
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

/// Build C++-compatible RRBS seed positions for one chromosome.
///
/// Returns `mode -> chain -> positions`, where chain 0 is BSW and chain 1 is
/// BSC. Each fragment contributes at most one seed position to each mode
/// bucket, matching C++ `CCGG_index[mode][chain]`.
pub fn build_rrbs_index(
    seq: &[u8],
    chr_size: u32,
    rc_offset: u32,
    seed_size: u32,
    sites: &[DigestionSite],
    min_insert: u32,
    max_insert: u32,
) -> RrbsModeIndex {
    use crate::param::FIXELEMENT;

    let max_seedseg_num = ((FIXELEMENT - 1) * SEGLEN) as u32 / seed_size;
    let mut by_mode = vec![vec![Vec::new(), Vec::new()]; max_seedseg_num as usize];

    let all_sites = find_sites(seq, sites);
    if all_sites.is_empty() {
        return by_mode;
    }

    let max_pos = chr_size.saturating_sub(seed_size);
    let tmp_offset = rc_offset.saturating_sub(seed_size);

    for j in 0..all_sites.len().saturating_sub(1) {
        let (pos_j, _) = all_sites[j];

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

        let mut seedloc = pos_j;
        for mode in 0..max_seedseg_num {
            if seedloc > max_pos {
                break;
            }
            by_mode[mode as usize][0].push(seedloc);
            seedloc += seed_size;
        }
    }

    for j in 1..all_sites.len() {
        let mut seglen: i64 = 0;
        let mut found = false;
        for i in (0..j).rev() {
            seglen = (all_sites[j].0 + all_sites[j].1) as i64 - all_sites[i].0 as i64;
            if seglen >= min_insert as i64 {
                found = true;
                break;
            }
        }
        if !found || seglen < min_insert as i64 || seglen > max_insert as i64 {
            continue;
        }

        let site_end = all_sites[j].0 + all_sites[j].1;
        // C++ uses unsigned bit32_t here, so the reverse seed walk wraps.
        let mut seedloc = site_end.wrapping_sub(seed_size);
        for mode in 0..max_seedseg_num {
            by_mode[mode as usize][1].push(tmp_offset.wrapping_sub(seedloc));
            seedloc = seedloc.wrapping_sub(seed_size);
        }
    }

    by_mode
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
        let site = DigestionSite::parse("C-YG").unwrap();
        assert_eq!(site.cut_pos, 1);
        assert_eq!(site.sequences.len(), 2);
    }

    #[test]
    fn test_expand_iupac_simple() {
        let result = expand_iupac("CCGG");
        assert_eq!(result, vec!["CCGG"]);
    }

    #[test]
    fn test_expand_iupac_ambiguous() {
        let result = expand_iupac("YG");
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"CG".to_string()));
        assert!(result.contains(&"TG".to_string()));
    }

    #[test]
    fn test_find_sites() {
        let seq = b"ACGTCCGGACGTCCGGCCGG";
        let site = DigestionSite::parse("C-CGG").unwrap();
        let results = find_sites(seq, &[site]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 5);
    }

    #[test]
    fn test_build_rrbs_index_is_grouped_by_mode() {
        let seq = b"ACGTCCGGAAAAAAAAAAAAAAAAAAAAAAACCGGTTTTTTTTTTTTTTTTTTTTTTTTCCGG";
        let site = DigestionSite::parse("C-CGG").unwrap();
        let index = build_rrbs_index(seq, seq.len() as u32, 128, 12, &[site], 20, 1000);

        assert!(index.len() > 1);
        assert!(!index[0][0].is_empty());
        assert!(!index[1][0].is_empty());
        assert_ne!(index[0][0], index[1][0]);
    }

    #[test]
    fn test_build_rrbs_index_wraps_reverse_seed_walk_like_cpp() {
        let seq = b"ACGTCCGGAAAAAAAAAAAAAAAAAAAAAAACCGGTTTTTTTTTTTTTTTTTTTTTTTTCCGG";
        let site = DigestionSite::parse("C-CGG").unwrap();
        let index = build_rrbs_index(seq, seq.len() as u32, 128, 12, &[site], 20, 1000);

        assert!(index.iter().all(|mode| !mode[1].is_empty()));
    }
}
