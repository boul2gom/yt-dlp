#!/usr/bin/env python3
# ─────────────────────────────────────────────────────────────────────────────
# bench-table.py — Parse Criterion JSON results and print Markdown tables
#                  matching the PROFILING.md format.
#
# Usage:
#   ./scripts/bench-table.py              # parse existing results only
#   ./scripts/bench-table.py --run        # run cargo bench first, then parse
#   ./scripts/bench-table.py --run-all    # run with all feature-gated groups
# ─────────────────────────────────────────────────────────────────────────────
import os
import sys
import json
import subprocess
import argparse
from pathlib import Path

CRITERION_DIR = Path("target/criterion")

def fmt_ns(ns):
    """Format nanoseconds to human-readable string (ns / µs / ms / s)"""
    if ns < 1000:
        return f"{ns:.1f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1000:.1f} µs"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000:.1f} ms"
    else:
        return f"{ns / 1_000_000_000:.2f} s"

def read_estimate(file_path):
    """Read mean point estimate from estimates.json in ns"""
    if not file_path.is_file():
        return None
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            return data.get('mean', {}).get('point_estimate')
    except Exception:
        return None

def get_result(group, bench_name, param=""):
    """Get formatted timing for a benchmark result"""
    if param:
        path = CRITERION_DIR / group / bench_name / str(param) / "new" / "estimates.json"
    else:
        path = CRITERION_DIR / group / bench_name / "new" / "estimates.json"
        
    ns = read_estimate(path)
    if ns is None:
        return "—"
    return fmt_ns(ns)

def main():
    parser = argparse.ArgumentParser(
        description="Parse Criterion JSON results and print Markdown tables matching the PROFILING.md format."
    )
    parser.add_argument(
        '--run',
        action='store_true',
        help="Run cargo bench first, then parse"
    )
    parser.add_argument(
        '--run-all',
        action='store_true',
        help="Run with all feature-gated groups"
    )
    
    args = parser.parse_args()

    # Run benchmarks if flags are provided
    if args.run_all:
        print("Running cargo bench with all features...")
        subprocess.run(["cargo", "bench", "--features", "webhooks cache-json"], check=True)
        print()
    elif args.run:
        print("Running cargo bench...")
        subprocess.run(["cargo", "bench"], check=True)
        print()

    if not CRITERION_DIR.is_dir():
        print(f"No Criterion results found in {CRITERION_DIR}")
        print("Run 'cargo bench' first, or use './scripts/bench-table.py --run'")
        sys.exit(1)

    print("╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ CRITERION BENCHMARK RESULTS                                                │")
    print("╰────────────────────────────────────────────────────────────────────────────╯\n")

    print("### Format Selection (`cargo bench -- format_selection`)")
    print("| Operation | 10 formats | 50 formats | 100 formats | 500 formats |")
    print("|---|---|---|---|---|")
    ops = ["best_video_format", "best_audio_format", "worst_video_format", "worst_audio_format", "select_video_High_AVC1", "select_audio_Best_Opus"]
    for op in ops:
        label = op
        if op == "select_video_High_AVC1":
            label = "select_video High/AVC1"
        elif op == "select_audio_Best_Opus":
            label = "select_audio Best/Opus"
        
        r10 = get_result("format_selection", op, 10)
        r50 = get_result("format_selection", op, 50)
        r100 = get_result("format_selection", op, 100)
        r500 = get_result("format_selection", op, 500)
        print(f"| `{label}` | {r10} | {r50} | {r100} | {r500} |")

    print("\n### Validation (`cargo bench -- validation`)")
    print("\n| Operation | Time |\n|---|---|")
    ops = [
        "validate_youtube_url/valid_youtube",
        "validate_youtube_url/valid_youtu_be",
        "validate_youtube_url/non_youtube",
        "validate_youtube_url/invalid",
        "sanitize_filename/normal",
        "sanitize_filename/special_chars",
        "sanitize_filename/unicode",
        "sanitize_path/simple",
        "sanitize_path/traversal"
    ]
    for op in ops:
        r = get_result("validation", op)
        label = op.replace("/", " — ")
        print(f"| `{label}` | {r} |")

    print("\n### Model Operations (`cargo bench -- model_operations`)")
    print("\n| Operation | Time |\n|---|---|")
    ops = ["has_chapters", "get_chapters", "has_heatmap", "has_subtitle_language_en", "serialize_to_json", "deserialize_from_json"]
    for op in ops:
        print(f"| `{op}` | {get_result('model_operations', op)} |")

    print("\n### Config Builders (`cargo bench -- config_builders`)")
    print("\n| Operation | Time |\n|---|---|")
    for op in ["ManagerConfig_default", "ManagerConfig_builder_max5", "PostProcessConfig_H264_AAC"]:
        label = op
        if op == "ManagerConfig_default": label = "ManagerConfig::default()"
        elif op == "ManagerConfig_builder_max5": label = "ManagerConfig::builder().max(5).build()"
        elif op == "PostProcessConfig_H264_AAC": label = "PostProcessConfig H264/AAC"
        print(f"| `{label}` | {get_result('config_builders', op)} |")

    print("\n### Chapter Operations (`cargo bench -- chapter_ops`)")
    print("\n| Operation | 5 chapters | 20 chapters | 50 chapters | 100 chapters |\n|---|---|---|---|---|")
    for op in ["find_by_timestamp", "search_by_title", "contains_timestamp", "validate"]:
        print(f"| `{op}` | {get_result('chapter_ops', op, 5)} | {get_result('chapter_ops', op, 20)} | {get_result('chapter_ops', op, 50)} | {get_result('chapter_ops', op, 100)} |")

    print("\n### Heatmap Operations (`cargo bench -- heatmap_ops`)")
    print("\n| Operation | 10 points | 100 points | 1 000 points |\n|---|---|---|---|")
    for op in ["most_engaged_segment", "get_highly_engaged_segments_0_7", "get_point_at_time_42"]:
        label = "get_highly_engaged_segments(0.7)" if "0_7" in op else "get_point_at_time(42.0)" if "42" in op else op
        print(f"| `{label}` | {get_result('heatmap_ops', op, 10)} | {get_result('heatmap_ops', op, 100)} | {get_result('heatmap_ops', op, 1000)} |")

    print("\n### Playlist Operations (`cargo bench -- playlist_ops`)")
    print("\n| Operation | 10 entries | 50 entries | 200 entries |\n|---|---|---|---|")
    for op in ["available_entries", "search_entries_by_title", "filter_by_uploader"]:
        print(f"| `{op}` | {get_result('playlist_ops', op, 10)} | {get_result('playlist_ops', op, 50)} | {get_result('playlist_ops', op, 200)} |")

    print("\n### Format Type Detection (`cargo bench -- format_type`)")
    print("\n| Operation | Time |\n|---|---|")
    for op in ["format_type_video", "format_type_audio", "format_type_muxed", "format_type_manifest", "is_video", "is_audio"]:
        label = f"format_type() — {op.split('_')[-1]}-only" if op.startswith("format_type") and op not in ["format_type_muxed", "format_type_manifest"] else op
        if op == "format_type_muxed": label = "format_type() — muxed"
        if op == "format_type_manifest": label = "format_type() — manifest"
        print(f"| `{label}` | {get_result('format_type', op)} |")

    print("\n### Event Filter (`cargo bench -- event_filter`)")
    print("\n| Operation | Time |\n|---|---|")
    for op in ["event_filter_all_matches", "event_filter_only_terminal_match", "event_filter_only_terminal_no_match"]:
        label = "EventFilter::all().matches() (always true)" if "all" in op else "EventFilter::only_terminal().matches() (terminal event)" if "match" in op and "no_match" not in op else "EventFilter::only_terminal().matches() (progress event, no match)"
        print(f"| `{label}` | {get_result('event_filter', op)} |")

    print("\n### Speed Profiles — Optimal Segments (`cargo bench -- speed_profile`)")
    print("\n| Profile | 1 MB | 50 MB | 500 MB | 2 GB |\n|---|---|---|---|---|")
    for profile in ["Conservative", "Balanced", "Aggressive"]:
        print(f"| {profile} | {get_result('speed_profile', f'{profile}/calculate_optimal_segments', 1_000_000)} | {get_result('speed_profile', f'{profile}/calculate_optimal_segments', 50_000_000)} | {get_result('speed_profile', f'{profile}/calculate_optimal_segments', 500_000_000)} | {get_result('speed_profile', f'{profile}/calculate_optimal_segments', 2_000_000_000)} |")

    print("\n### Retry Strategy (`cargo bench --features webhooks -- retry_strategy`)")
    print("\n| Operation | Time |\n|---|---|")
    for attempt in [0, 1, 3]:
        print(f"| `delay_for_attempt({attempt})` | {get_result('retry_strategy', 'delay_for_attempt', attempt)} |")
    print(f"| `should_retry` (true) | {get_result('retry_strategy', 'should_retry_true')} |")
    print(f"| `should_retry` (false) | {get_result('retry_strategy', 'should_retry_false')} |")

    print("\n### Cache Operations (`cargo bench --features cache -- cache_ops`)")
    print("\n| Operation | Time |\n|---|---|")
    for op in ["cache_put_single", "cache_get_hit", "cache_get_miss", "cache_put_500"]:
        print(f"| `{op}` | {get_result('cache_ops', op)} |")

    print("\nDone! Copy the tables above into PROFILING.md's '📊 Benchmark result tables' section.")

if __name__ == '__main__':
    main()
