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

SIMPLE_TABLE_HEADER = "| Operation | Time |\n|---|---|"

# ── Benchmark section definitions ────────────────────────────────────────────
# Each section is a dict with: title, kind ("simple" / "parameterized" / "mixed"),
# group (Criterion group name), and ops list.
# For "simple" ops: list of (label, bench_name) or just bench_name (label = bench_name).
# For "parameterized" ops: params list and ops list of bench_name or (label, bench_name).
# For "mixed" ops: list of dicts with label, bench, and optional param.

BENCHMARK_SECTIONS = [
    {
        "title": "Format Selection (`cargo bench -- format_selection`)",
        "group": "format_selection",
        "kind": "parameterized",
        "header": "| Operation | 10 formats | 50 formats | 100 formats | 500 formats |",
        "params": [10, 50, 100, 500],
        "ops": [
            "best_video_format",
            "best_audio_format",
            "worst_video_format",
            "worst_audio_format",
            ("select_video High/AVC1", "select_video_High_AVC1"),
            ("select_audio Best/Opus", "select_audio_Best_Opus"),
        ],
    },
    {
        "title": "Validation (`cargo bench -- validation`)",
        "group": "validation",
        "kind": "simple",
        "ops": [
            "validate_youtube_url/valid_youtube",
            "validate_youtube_url/valid_youtu_be",
            "validate_youtube_url/non_youtube",
            "validate_youtube_url/invalid",
            "sanitize_filename/normal",
            "sanitize_filename/special_chars",
            "sanitize_filename/unicode",
            "sanitize_path/simple",
            "sanitize_path/traversal",
        ],
        "label_fn": lambda op: op.replace("/", " — "),
    },
    {
        "title": "Model Operations (`cargo bench -- model_operations`)",
        "group": "model_operations",
        "kind": "simple",
        "ops": ["has_chapters", "get_chapters", "has_heatmap", "has_subtitle_language_en",
                "serialize_to_json", "deserialize_from_json"],
    },
    {
        "title": "Config Builders (`cargo bench -- config_builders`)",
        "group": "config_builders",
        "kind": "simple",
        "ops": [
            ("ManagerConfig::default()", "ManagerConfig_default"),
            ("ManagerConfig::builder().max(5).build()", "ManagerConfig_builder_max5"),
            ("PostProcessConfig H264/AAC", "PostProcessConfig_H264_AAC"),
        ],
    },
    {
        "title": "Chapter Operations (`cargo bench -- chapter_ops`)",
        "group": "chapter_ops",
        "kind": "parameterized",
        "header": "| Operation | 5 chapters | 20 chapters | 50 chapters | 100 chapters |",
        "params": [5, 20, 50, 100],
        "ops": ["find_by_timestamp", "search_by_title", "contains_timestamp", "validate"],
    },
    {
        "title": "Heatmap Operations (`cargo bench -- heatmap_ops`)",
        "group": "heatmap_ops",
        "kind": "parameterized",
        "header": "| Operation | 10 points | 100 points | 1 000 points |",
        "params": [10, 100, 1000],
        "ops": [
            "most_engaged_segment",
            ("get_highly_engaged_segments(0.7)", "get_highly_engaged_segments_0_7"),
            ("get_point_at_time(42.0)", "get_point_at_time_42"),
        ],
    },
    {
        "title": "Playlist Operations (`cargo bench -- playlist_ops`)",
        "group": "playlist_ops",
        "kind": "parameterized",
        "header": "| Operation | 10 entries | 50 entries | 200 entries |",
        "params": [10, 50, 200],
        "ops": ["available_entries", "search_entries_by_title", "filter_by_uploader"],
    },
    {
        "title": "Format Type Detection (`cargo bench -- format_type`)",
        "group": "format_type",
        "kind": "simple",
        "ops": [
            ("format_type() — video-only", "format_type_video"),
            ("format_type() — audio-only", "format_type_audio"),
            ("format_type() — muxed", "format_type_muxed"),
            ("format_type() — manifest", "format_type_manifest"),
            "is_video",
            "is_audio",
        ],
    },
    {
        "title": "Event Filter (`cargo bench -- event_filter`)",
        "group": "event_filter",
        "kind": "simple",
        "ops": [
            ("EventFilter::all().matches() (always true)", "event_filter_all_matches"),
            ("EventFilter::only_terminal().matches() (terminal event)", "event_filter_only_terminal_match"),
            ("EventFilter::only_terminal().matches() (progress event, no match)", "event_filter_only_terminal_no_match"),
        ],
    },
    {
        "title": "Speed Profiles — Optimal Segments (`cargo bench -- speed_profile`)",
        "group": "speed_profile",
        "kind": "parameterized",
        "header": "| Profile | 1 MB | 50 MB | 500 MB | 2 GB |",
        "params": [1_000_000, 50_000_000, 500_000_000, 2_000_000_000],
        "ops": [
            ("Conservative", "Conservative/calculate_optimal_segments"),
            ("Balanced", "Balanced/calculate_optimal_segments"),
            ("Aggressive", "Aggressive/calculate_optimal_segments"),
        ],
    },
    {
        "title": "Retry Strategy (`cargo bench --features webhooks -- retry_strategy`)",
        "group": "retry_strategy",
        "kind": "mixed",
        "ops": [
            {"label": "delay_for_attempt(0)", "bench": "delay_for_attempt", "param": 0},
            {"label": "delay_for_attempt(1)", "bench": "delay_for_attempt", "param": 1},
            {"label": "delay_for_attempt(3)", "bench": "delay_for_attempt", "param": 3},
            {"label": "should_retry (true)", "bench": "should_retry_true"},
            {"label": "should_retry (false)", "bench": "should_retry_false"},
        ],
    },
    {
        "title": "Cache Operations (`cargo bench --features cache -- cache_ops`)",
        "group": "cache_ops",
        "kind": "mixed",
        "ops": [
            {"label": "cache_put_single", "bench": "cache_put_single"},
            {"label": "cache_get_hit", "bench": "cache_get_hit"},
            {"label": "cache_get_miss", "bench": "cache_get_miss"},
            {"label": "cache_put_500", "bench": "cache_put_500"},
        ],
    },
]


def fmt_ns(ns):
    """Format nanoseconds to human-readable string (ns / µs / ms / s)"""
    if ns < 1000:
        return f"{ns:.1f} ns"
    if ns < 1_000_000:
        return f"{ns / 1000:.1f} µs"
    if ns < 1_000_000_000:
        return f"{ns / 1_000_000:.1f} ms"
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
    if param != "":
        path = CRITERION_DIR / group / bench_name / str(param) / "new" / "estimates.json"
    else:
        path = CRITERION_DIR / group / bench_name / "new" / "estimates.json"

    ns = read_estimate(path)
    return "—" if ns is None else fmt_ns(ns)


def resolve_label(op, label_fn=None):
    """Extract label and bench name from an op entry."""
    if isinstance(op, tuple):
        return op[0], op[1]
    if label_fn:
        return label_fn(op), op
    return op, op


def render_simple_section(section):
    """Render a simple (non-parameterized) benchmark section."""
    group = section["group"]
    label_fn = section.get("label_fn")

    print(f"\n{SIMPLE_TABLE_HEADER}")
    for op in section["ops"]:
        label, bench_name = resolve_label(op, label_fn)
        print(f"| `{label}` | {get_result(group, bench_name)} |")


def render_parameterized_section(section):
    """Render a parameterized benchmark section with multiple columns."""
    group = section["group"]
    params = section["params"]
    separator = "|---" * (len(params) + 1) + "|"

    print(f"\n{section['header']}")
    print(separator)
    for op in section["ops"]:
        label, bench_name = resolve_label(op)
        cells = " | ".join(get_result(group, bench_name, p) for p in params)
        print(f"| `{label}` | {cells} |")


def render_mixed_section(section):
    """Render a section with explicit per-op bench names and optional params."""
    group = section["group"]

    print(f"\n{SIMPLE_TABLE_HEADER}")
    for op in section["ops"]:
        param = op.get("param", "")
        print(f"| `{op['label']}` | {get_result(group, op['bench'], param)} |")


SECTION_RENDERERS = {
    "simple": render_simple_section,
    "parameterized": render_parameterized_section,
    "mixed": render_mixed_section,
}


def render_section(section):
    """Render a complete benchmark section with title and table."""
    print(f"\n### {section['title']}")
    renderer = SECTION_RENDERERS[section["kind"]]
    renderer(section)


def run_benchmarks(args):
    """Run cargo bench if flags are provided."""
    if args.run_all:
        print("Running cargo bench with all features...")
        subprocess.run(["cargo", "bench", "--features", "webhooks cache-json"], check=True)
        print()
    elif args.run:
        print("Running cargo bench...")
        subprocess.run(["cargo", "bench"], check=True)
        print()


def main():
    parser = argparse.ArgumentParser(
        description="Parse Criterion JSON results and print Markdown tables matching the PROFILING.md format."
    )
    parser.add_argument('--run', action='store_true', help="Run cargo bench first, then parse")
    parser.add_argument('--run-all', action='store_true', help="Run with all feature-gated groups")
    args = parser.parse_args()

    run_benchmarks(args)

    if not CRITERION_DIR.is_dir():
        print(f"No Criterion results found in {CRITERION_DIR}")
        print("Run 'cargo bench' first, or use './scripts/bench-table.py --run'")
        sys.exit(1)

    print("╭────────────────────────────────────────────────────────────────────────────╮")
    print("│ CRITERION BENCHMARK RESULTS                                                │")
    print("╰────────────────────────────────────────────────────────────────────────────╯\n")

    for section in BENCHMARK_SECTIONS:
        render_section(section)

    print("\nDone! Copy the tables above into PROFILING.md's '📊 Benchmark result tables' section.")


if __name__ == '__main__':
    main()
