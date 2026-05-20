# Fix CLI parameter omission
$file = "c:\Users\zhang_i5edc0\OneDrive\Documents\TraeSOLO\BSMAP\bsmap-rs\bsmap\src\cli.rs"
$content = Get-Content $file -Raw

# Find and replace the pattern
$oldPattern = @'
            align_transition,
            digestion_sites,
            num_threads,
            randseed,
            verbose,
        }) => ResolvedCommand::Align(AlignArgs {
            query_a: Some(query_a.clone()),
            query_b: query_b.clone(),
            reference: Some(reference.clone()),
            output: output.clone(),
            seed_size: *seed_size,
            max_mismatch: *max_mismatch,
            gap_size: *gap_size,
            max_hits: *max_hits,
            nt3: *nt3,
            read_start: *read_start,
            read_end: *read_end,
            index_interval: *index_interval,
            kmer_cutoff: *kmer_cutoff,
            qual_threshold: *qual_threshold,
            zero_qual: *zero_qual,
            max_ns: *max_ns,
            adapters: adapters.clone(),
            max_read_len: max_read_len.unwrap_or(0),
            report_repeat: *report_repeat,
            out_ref: *out_ref,
            out_unmap: *out_unmap,
            no_header: *no_header,
            min_insert: *min_insert,
            max_insert: *max_insert,
            chains: *chains,
            align_transition: align_transition.clone(),
            digestion_sites: digestion_sites.clone(),
            num_threads: *num_threads,
            no_prefetch: *no_prefetch,
            randseed: *randseed,
            verbose: *verbose,
        }),
'@

$newPattern = @'
            align_transition,
            digestion_sites,
            num_threads,
            no_prefetch,
            randseed,
            verbose,
        }) => ResolvedCommand::Align(AlignArgs {
            query_a: Some(query_a.clone()),
            query_b: query_b.clone(),
            reference: Some(reference.clone()),
            output: output.clone(),
            seed_size: *seed_size,
            max_mismatch: *max_mismatch,
            gap_size: *gap_size,
            max_hits: *max_hits,
            nt3: *nt3,
            read_start: *read_start,
            read_end: *read_end,
            index_interval: *index_interval,
            kmer_cutoff: *kmer_cutoff,
            qual_threshold: *qual_threshold,
            zero_qual: *zero_qual,
            max_ns: *max_ns,
            adapters: adapters.clone(),
            max_read_len: max_read_len.unwrap_or(0),
            report_repeat: *report_repeat,
            out_ref: *out_ref,
            out_unmap: *out_unmap,
            no_header: *no_header,
            min_insert: *min_insert,
            max_insert: *max_insert,
            chains: *chains,
            align_transition: align_transition.clone(),
            digestion_sites: digestion_sites.clone(),
            num_threads: *num_threads,
            no_prefetch: *no_prefetch,
            randseed: *randseed,
            verbose: *verbose,
        }),
'@

if ($content -match [regex]::Escape($oldPattern)) {
    $content = $content -replace [regex]::Escape($oldPattern), $newPattern
    $content | Set-Content $file -NoNewline
    Write-Host "Fixed: Added no_prefetch to pattern match" -ForegroundColor Green
} else {
    Write-Host "Pattern not found" -ForegroundColor Red
}
