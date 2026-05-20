#!/usr/bin/env python3
import re

# Read the file
with open('bsmap/src/cli.rs', 'r', encoding='utf-8') as f:
    content = f.read()

# Find and replace pattern
# Looking for:
#   align_transition,
#   digestion_sites,
#   num_threads,
#   randseed,
# And changing to:
#   align_transition,
#   digestion_sites,
#   num_threads,
#   no_prefetch,
#   randseed,

old_pattern = r'(            align_transition,\n            digestion_sites,\n            num_threads,\n)            randseed,'
new_pattern = r'\1            no_prefetch,\n            randseed,'

if re.search(old_pattern, content):
    content = re.sub(old_pattern, new_pattern, content)
    with open('bsmap/src/cli.rs', 'w', encoding='utf-8') as f:
        f.write(content)
    print("Fixed: Added no_prefetch to pattern match")
else:
    print("Pattern not found")
    # Let's find what we have
    match = re.search(r'align_transition,\n            digestion_sites,\n            num_threads,\n\s+randseed,', content)
    if match:
        print(f"Found similar at position {match.start()}")
        print("Context:")
        print(match.group(0))
