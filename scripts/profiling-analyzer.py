#!/usr/bin/env python3
# ─────────────────────────────────────────────────────────────────────────────
# profiling-analyzer.py — Parse and analyze dhat-heap.json and profile.json.gz
#                         to identify memory and CPU bottlenecks.
#
# Usage:
#   ./scripts/profiling-analyzer.py [--dhat-file <path>] [--samply-file <path>]
# ─────────────────────────────────────────────────────────────────────────────

import argparse
import gzip
import json
import os

# Alloc frames to hide in DHAT stack traces.
ALLOC_NOISE = ("alloc::", "dhat::")


# ── Symbol helpers ────────────────────────────────────────────────────────────

def clean_symbol(name):
    """Strip the Rust hash suffix (::h...) from a symbol name."""
    return name.split("::h")[0]


# ── DHAT Analysis ─────────────────────────────────────────────────────────────

def resolve_allocator_stack(pp, frames):
    """Resolve the stack trace for a single allocation point, filtering noise."""
    stack = []
    for f_idx in pp.get('fs', [])[:15]:
        if not isinstance(f_idx, int) or f_idx >= len(frames):
            continue
        name = clean_symbol(frames[f_idx])
        if not any(noise in name for noise in ALLOC_NOISE):
            stack.append(name)
    return stack


def print_top_allocators(pps, frames, total_bytes, limit=10):
    """Print the top N allocators sorted by total bytes."""
    print(f"Top {limit} Allocators (by total bytes):")

    for pp in pps[:limit]:
        tb = pp.get('tb', 0)
        tbk = pp.get('tbk', 0)
        stack = resolve_allocator_stack(pp, frames)
        name = stack[0] if stack else "Unknown"
        pct = (tb / total_bytes * 100) if total_bytes > 0 else 0

        print(f"- {tb:12,} bytes ({pct:5.1f}%) | {tbk:8,} allocs | {name}")
        for s in stack[1:4]:
            print(f"    <- {s}")


def analyze_dhat(dhat_path="dhat-heap.json"):
    """Parse dhat-heap.json and print the top allocators by total bytes."""
    print("╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ DHAT MEMORY ANALYSIS                                                       │")
    print("╰────────────────────────────────────────────────────────────────────────────╯")

    if not os.path.exists(dhat_path):
        print(f"{dhat_path} not found!\n")
        return

    try:
        with open(dhat_path) as f:
            d = json.load(f)

        frames = d.get('ftbl', [])
        pps = d.get('pps', [])

        total_bytes = sum(p.get('tb', 0) for p in pps)
        total_blocks = sum(p.get('tbk', 0) for p in pps)
        print(f"Total Allocated: {total_bytes:,} bytes in {total_blocks:,} blocks\n")

        pps.sort(key=lambda x: x.get('tb', 0), reverse=True)
        print_top_allocators(pps, frames, total_bytes)

    except Exception as e:
        print(f"Error analyzing DHAT: {e}")


# ── Samply Analysis ───────────────────────────────────────────────────────────

def build_thread_tables(thread):
    """Extract the lookup tables needed for stack resolution from a thread."""
    return {
        "strings": thread.get('stringArray', []),
        "f_func": thread.get('frameTable', {}).get('func', []),
        "fn_name": thread.get('funcTable', {}).get('name', []),
        "st_frame": thread.get('stackTable', {}).get('frame', []),
        "st_prefix": thread.get('stackTable', {}).get('prefix', []),
    }


def resolve_func_name(frame_idx, tables):
    """Resolve a frame index to a cleaned function name, or None."""
    f_func = tables["f_func"]
    fn_name = tables["fn_name"]
    strings = tables["strings"]

    if frame_idx is None or frame_idx >= len(f_func):
        return None

    func_idx = f_func[frame_idx]
    if func_idx is None or func_idx >= len(fn_name):
        return None

    name_idx = fn_name[func_idx]
    if name_idx is None or name_idx >= len(strings):
        return None

    return clean_symbol(strings[name_idx])


def count_samples(stack_data, tables):
    """Walk all stacks and compute self-time and total-time counts per function."""
    st_frame = tables["st_frame"]
    st_prefix = tables["st_prefix"]
    self_counts = {}
    total_counts = {}

    for stack_idx in stack_data:
        if stack_idx is None:
            continue

        curr = stack_idx
        is_leaf = True
        seen = set()

        while curr is not None and curr < len(st_frame):
            func_name = resolve_func_name(st_frame[curr], tables)
            if func_name is not None:
                if is_leaf:
                    self_counts[func_name] = self_counts.get(func_name, 0) + 1
                    is_leaf = False
                if func_name not in seen:
                    total_counts[func_name] = total_counts.get(func_name, 0) + 1
                    seen.add(func_name)

            curr = st_prefix[curr] if curr < len(st_prefix) else None

    return self_counts, total_counts


def print_top_functions(counts, total_samples, header, limit=10):
    """Print the top N functions by sample count."""
    print(header)
    sorted_items = sorted(counts.items(), key=lambda x: x[1], reverse=True)
    for func, count in sorted_items[:limit]:
        print(f"  - {count:5} samples ({count / total_samples * 100:5.1f}%) : {func}")


def analyze_thread(thread):
    """Analyze a single samply thread and print its CPU profile."""
    samples = thread.get('samples', {})
    stack_data = samples.get('stack', [])

    if not stack_data or len(stack_data) < 100:
        return

    name = thread.get('name', 'Unknown')
    is_main = thread.get('isMainThread', False)
    total_samples = len(stack_data)
    print(f"\nThread: {name} (Main: {is_main}) - {total_samples} samples")

    tables = build_thread_tables(thread)
    self_counts, total_counts = count_samples(stack_data, tables)

    print_top_functions(self_counts, total_samples,
                        "  Top functions by SELF time (where execution was bottlenecked):")
    print_top_functions(total_counts, total_samples,
                        "\n  Top functions by TOTAL time (execution + children):")


def analyze_samply(samply_path="profile.json.gz"):
    """Parse profile.json.gz and print per-thread CPU profiles."""
    print("\n╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ SAMPLY CPU ANALYSIS                                                        │")
    print("╰────────────────────────────────────────────────────────────────────────────╯")

    if not os.path.exists(samply_path):
        print(f"{samply_path} not found!\n")
        return

    try:
        with gzip.open(samply_path, 'rt') as f:
            prof = json.load(f)

        for thread in prof.get('threads', []):
            analyze_thread(thread)

    except Exception as e:
        print(f"Error analyzing profile.json.gz: {e}")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(
        description="Parse and analyze DHAT and Samply profiles to identify memory and CPU bottlenecks."
    )
    parser.add_argument('--dhat-file', type=str, default='dhat-heap.json',
                        help="Path to DHAT heap profile (default: dhat-heap.json)")
    parser.add_argument('--samply-file', type=str, default='profile.json.gz',
                        help="Path to Samply profile (default: profile.json.gz)")
    parser.add_argument('--no-dhat', action='store_true', help="Skip DHAT memory analysis")
    parser.add_argument('--no-samply', action='store_true', help="Skip Samply CPU analysis")

    args = parser.parse_args()

    if not args.no_dhat:
        analyze_dhat(args.dhat_file)

    if not args.no_samply:
        analyze_samply(args.samply_file)


if __name__ == '__main__':
    main()
