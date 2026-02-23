#!/usr/bin/env python3
# ─────────────────────────────────────────────────────────────────────────────
# find_duplicates.py — Scan a directory for duplicated blocks of code.
#                      Helps identify areas for refactoring.
#
# Usage:
#   ./scripts/find-duplicates.py [--window-size 15] [--dir src]
# ─────────────────────────────────────────────────────────────────────────────

import os
import argparse
from collections import defaultdict

def get_files(root_dir):
    for dirpath, _, filenames in os.walk(root_dir):
        for f in filenames:
            if f.endswith('.rs'):
                yield os.path.join(dirpath, f)

def get_blocks(file_path, window_size=15):
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    # Normalize lines: strip whitespace, ignore empty or very short lines
    normalized = []
    for i, line in enumerate(lines):
        cl = line.strip()
        if cl and not cl.startswith('//') and not cl.startswith('#[') and len(cl) > 3:
            normalized.append((i+1, cl))
            
    for i in range(len(normalized) - window_size + 1):
        block = tuple(n[1] for n in normalized[i:i+window_size])
        start_line = normalized[i][0]
        yield block, (file_path, start_line)

def main():
    parser = argparse.ArgumentParser(description="Scan a directory for duplicated blocks of code.")
    parser.add_argument(
        '-w', '--window-size',
        type=int,
        default=15,
        help="Number of lines that must match to be considered a duplicate (default: 15)"
    )
    parser.add_argument(
        '-d', '--dir',
        type=str,
        default='src',
        help="Directory to scan (default: 'src')"
    )
    
    args = parser.parse_args()

    print(f"╭────────────────────────────────────────────────────────────────────────────╮")
    print(f"│ CODE DUPLICATION ANALYSIS                                                  │")
    print(f"╰────────────────────────────────────────────────────────────────────────────╯")
    print(f"Scanning directory '{args.dir}' for duplicated blocks of {args.window_size}+ significant lines...\n")
    
    duplicate_blocks = defaultdict(list)
    for f in get_files(args.dir):
        for block, loc in get_blocks(f, window_size=args.window_size):
            duplicate_blocks[block].append(loc)

    # Filter out blocks that are just subsets of larger duplicated blocks
    # To keep the output clean, we will print unique files/line pairs that share a block.
    
    reported_pairs = set()
    matches_found = 0
    
    for block, occurrences in duplicate_blocks.items():
        if len(occurrences) > 1:
            # Check if occurrences are far apart or in different files
            is_valid_duplicate = False
            for i in range(len(occurrences)):
                for j in range(i + 1, len(occurrences)):
                    f1, l1 = occurrences[i]
                    f2, l2 = occurrences[j]
                    if f1 != f2 or abs(l1 - l2) > 20:
                        is_valid_duplicate = True
                        pair_key = tuple(sorted([(f1, l1), (f2, l2)]))
                        if pair_key not in reported_pairs:
                            reported_pairs.add(pair_key)
                            print(f"Match found:")
                            print(f"  - {f1}:{l1}")
                            print(f"  - {f2}:{l2}")
                            print()
                            matches_found += 1

    if matches_found == 0:
        print("No duplicates found!")
    else:
        print(f"Total unique duplicated blocks found: {matches_found}")

if __name__ == '__main__':
    main()
