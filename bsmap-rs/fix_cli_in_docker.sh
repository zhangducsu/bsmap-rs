#!/bin/bash
# Fix CLI parameter - proper fix
cd /workspace/bsmap-rs

# Add no_prefetch to pattern match in cli.rs
sed -i 's/digestion_sites,$/digestion_sites,\n            no_prefetch,/' bsmap/src/cli.rs

# Verify the fix
grep -A2 "digestion_sites," bsmap/src/cli.rs | head -5
