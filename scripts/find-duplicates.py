#!/usr/bin/env python3
# ─────────────────────────────────────────────────────────────────────────────
# find-duplicates.py — Scan a directory for duplicated blocks of code.
#                      Helps identify areas for refactoring.
#
# Usage:
#   ./scripts/find-duplicates.py [--window-size 15] [--dir src]
# ─────────────────────────────────────────────────────────────────────────────

import argparse
import os
from collections import defaultdict


# ── Helper functions ──────────────────────────────────────────────────────────

def get_files(root_dir):
    """Recursively yield paths to all .rs files under root_dir."""
    for dirpath, _, filenames in os.walk(root_dir):
        for f in filenames:
            if f.endswith('.rs'):
                yield os.path.join(dirpath, f)


def normalize_lines(file_path):
    """Read a file and return significant (non-empty, non-comment, non-attribute) lines."""
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    result = []
    for i, line in enumerate(lines):
        cl = line.strip()
        if cl and not cl.startswith('//') and not cl.startswith('#[') and len(cl) > 3:
            result.append((i + 1, cl))
    return result


def get_blocks(file_path, window_size=15):
    """Yield (block, location) tuples for all sliding windows in the file."""
    normalized = normalize_lines(file_path)

    for i in range(len(normalized) - window_size + 1):
        block = tuple(n[1] for n in normalized[i:i + window_size])
        start_line = normalized[i][0]
        yield block, (file_path, start_line)


# ── Duplicate detection ───────────────────────────────────────────────────────

def is_genuine_duplicate(loc_a, loc_b):
    """Check if two locations represent a genuine duplicate (different files or far apart)."""
    f1, l1 = loc_a
    f2, l2 = loc_b
    return f1 != f2 or abs(l1 - l2) > 20


def find_duplicate_pairs(duplicate_blocks):
    """Extract unique pairs of genuine duplicates from the block map."""
    reported_pairs = set()
    pairs = []

    for occurrences in duplicate_blocks.values():
        if len(occurrences) <= 1:
            continue

        for i in range(len(occurrences)):
            for j in range(i + 1, len(occurrences)):
                if not is_genuine_duplicate(occurrences[i], occurrences[j]):
                    continue

                pair_key = tuple(sorted([occurrences[i], occurrences[j]]))
                if pair_key in reported_pairs:
                    continue

                reported_pairs.add(pair_key)
                pairs.append((occurrences[i], occurrences[j]))

    return pairs


# ── Main ──────────────────────────────────────────────────────────────────────

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

    print("╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ CODE DUPLICATION ANALYSIS                                                  │")
    print("╰────────────────────────────────────────────────────────────────────────────╯")
    print(f"Scanning directory '{args.dir}' for duplicated blocks of {args.window_size}+ significant lines...\n")

    duplicate_blocks = defaultdict(list)
    for f in get_files(args.dir):
        for block, loc in get_blocks(f, window_size=args.window_size):
            duplicate_blocks[block].append(loc)

    pairs = find_duplicate_pairs(duplicate_blocks)

    for (f1, l1), (f2, l2) in pairs:
        print("Match found:")
        print(f"  - {f1}:{l1}")
        print(f"  - {f2}:{l2}")
        print()

    if not pairs:
        print("No duplicates found!")
    else:
        print(f"Total unique duplicated blocks found: {len(pairs)}")


if __name__ == '__main__':
    main()
