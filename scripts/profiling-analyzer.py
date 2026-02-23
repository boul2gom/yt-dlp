#!/usr/bin/env python3
# ─────────────────────────────────────────────────────────────────────────────
# profiling-analyzer.py — Parse and analyze dhat-heap.json and profile.json.gz
#                         to identify memory and CPU bottlenecks.
#
# Usage:
#   ./scripts/profiling-analyzer.py
# ─────────────────────────────────────────────────────────────────────────────

import json
import gzip
import os
import argparse

def analyze_dhat(dhat_path="dhat-heap.json"):
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
        
        # Calculate total
        total_bytes = sum(p.get('tb', 0) for p in pps)
        total_blocks = sum(p.get('tbk', 0) for p in pps)
        
        print(f"Total Allocated: {total_bytes:,} bytes in {total_blocks:,} blocks\n")
        
        # Sort by total bytes
        pps.sort(key=lambda x: x.get('tb', 0), reverse=True)
        
        print("Top 10 Allocators (by total bytes):")
        for p in pps[:10]:
            tb = p.get('tb', 0)
            tbk = p.get('tbk', 0)
            fs = p.get('fs', [])
            
            # Reconstruct stack trace
            stack = []
            for f_idx in fs[:15]: 
                if isinstance(f_idx, int) and f_idx < len(frames):
                    name = frames[f_idx]
                    name = name.split("::h")[0] 
                    # Hide basic alloc frames
                    if not any(x in name for x in ["alloc::", "dhat::"]):
                        stack.append(name)
            
            name = stack[0] if stack else "Unknown"
            pct = (tb / total_bytes * 100) if total_bytes > 0 else 0
            print(f"- {tb:12,} bytes ({pct:5.1f}%) | {tbk:8,} allocs | {name}")
            if len(stack) > 1:
                for s in stack[1:4]:
                    print(f"    <- {s}")
                
    except Exception as e:
        print(f"Error analyzing DHAT: {e}")

def analyze_samply(samply_path="profile.json.gz"):
    print("\n╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ SAMPLY CPU ANALYSIS                                                        │")
    print("╰────────────────────────────────────────────────────────────────────────────╯")
    
    if not os.path.exists(samply_path):
        print(f"{samply_path} not found!\n")
        return

    try:
        with gzip.open(samply_path, 'rt') as f:
            prof = json.load(f)
            
        threads = prof.get('threads', [])
        
        for t in threads:
            name = t.get('name', 'Unknown')
            is_main = t.get('isMainThread', False)
            
            samples = t.get('samples', {})
            stack_data = samples.get('stack', [])

            if not stack_data or len(stack_data) < 100:
                continue # Skip idle threads
                
            print(f"\nThread: {name} (Main: {is_main}) - {len(stack_data)} samples")
            
            string_array = t.get('stringArray', [])
            
            frame_table = t.get('frameTable', {})
            f_func = frame_table.get('func', [])
            
            func_table = t.get('funcTable', {})
            fn_name = func_table.get('name', [])
            
            stack_table = t.get('stackTable', {})
            st_frame = stack_table.get('frame', [])
            st_prefix = stack_table.get('prefix', [])
            
            self_counts = {}
            total_counts = {}
            
            for stack_idx in stack_data:
                if stack_idx is None: continue
                
                curr_st = stack_idx
                is_leaf = True
                seen_in_this_sample = set()
                
                while curr_st is not None:
                    try:
                        if curr_st >= len(st_frame): break
                        frame_idx = st_frame[curr_st]
                        prefix_idx = st_prefix[curr_st] if curr_st < len(st_prefix) else None
                        
                        if frame_idx is not None and frame_idx < len(f_func):
                            func_idx = f_func[frame_idx]
                            
                            if func_idx is not None and func_idx < len(fn_name):
                                name_idx = fn_name[func_idx]
                                
                                if name_idx is not None and name_idx < len(string_array):
                                    func_name = string_array[name_idx]
                                    func_name = func_name.split("::h")[0]
                                    
                                    if is_leaf:
                                        self_counts[func_name] = self_counts.get(func_name, 0) + 1
                                        is_leaf = False
                                        
                                    if func_name not in seen_in_this_sample:
                                        total_counts[func_name] = total_counts.get(func_name, 0) + 1
                                        seen_in_this_sample.add(func_name)
                                        
                        curr_st = prefix_idx
                    except IndexError:
                        break
            
            total_samples = len(stack_data)
            
            print("  Top functions by SELF time (where execution was bottlenecked):")
            sorted_self = sorted(self_counts.items(), key=lambda x: x[1], reverse=True)
            for func, count in sorted_self[:10]:
                print(f"  - {count:5} samples ({count/total_samples*100:5.1f}%) : {func}")
            
            print("\n  Top functions by TOTAL time (execution + children):")
            sorted_total = sorted(total_counts.items(), key=lambda x: x[1], reverse=True)
            for func, count in sorted_total[:10]:
                print(f"  - {count:5} samples ({count/total_samples*100:5.1f}%) : {func}")
                
    except Exception as e:
        print(f"Error analyzing profile.json.gz: {e}")

def main():
    parser = argparse.ArgumentParser(
        description="Parse and analyze DHAT and Samply profiles to identify memory and CPU bottlenecks."
    )
    parser.add_argument(
        '--dhat-file',
        type=str,
        default='dhat-heap.json',
        help="Path to DHAT heap profile (default: dhat-heap.json)"
    )
    parser.add_argument(
        '--samply-file',
        type=str,
        default='profile.json.gz',
        help="Path to Samply profile (default: profile.json.gz)"
    )
    parser.add_argument(
        '--no-dhat',
        action='store_true',
        help="Skip DHAT memory analysis"
    )
    parser.add_argument(
        '--no-samply',
        action='store_true',
        help="Skip Samply CPU analysis"
    )

    args = parser.parse_args()

    if not args.no_dhat:
        analyze_dhat(args.dhat_file)
    
    if not args.no_samply:
        analyze_samply(args.samply_file)

if __name__ == '__main__':
    main()