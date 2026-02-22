#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# bench-table.sh — Parse Criterion JSON results and print Markdown tables
#                  matching the PROFILING.md format.
#
# Usage:
#   ./benches/bench-table.sh              # parse existing results only
#   ./benches/bench-table.sh --run        # run cargo bench first, then parse
#   ./benches/bench-table.sh --run-all    # run with all feature-gated groups
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

CRITERION_DIR="target/criterion"

# ─── Helpers ──────────────────────────────────────────────────────────────────

# fmt_ns <nanoseconds>  →  human-readable string (ns / µs / ms / s)
fmt_ns() {
    local ns="$1"
    # Use awk for floating-point comparison
    awk -v ns="$ns" 'BEGIN {
        if (ns < 1000)          printf "%.1f ns\n", ns
        else if (ns < 1000000)  printf "%.1f µs\n", ns / 1000
        else if (ns < 1e9)      printf "%.1f ms\n", ns / 1000000
        else                    printf "%.2f s\n",  ns / 1e9
    }'
}

# read_estimate <path/to/estimates.json>  →  mean point_estimate in ns
read_estimate() {
    local file="$1"
    if [[ ! -f "$file" ]]; then
        echo ""
        return
    fi
    # Extract mean.point_estimate using python (available on macOS & Linux)
    python3 -c "
import json, sys
with open(sys.argv[1]) as f:
    data = json.load(f)
print(data['mean']['point_estimate'])
" "$file"
}

# get_result <group> <bench_name> [<param>]  →  formatted timing or empty
get_result() {
    local group="$1" bench="$2" param="${3:-}"
    local path

    if [[ -n "$param" ]]; then
        path="$CRITERION_DIR/$group/$bench/$param/new/estimates.json"
    else
        path="$CRITERION_DIR/$group/$bench/new/estimates.json"
    fi

    local ns
    ns=$(read_estimate "$path")
    if [[ -z "$ns" ]]; then
        echo ""
    else
        fmt_ns "$ns"
    fi
}

# ─── Run benchmarks (optional) ───────────────────────────────────────────────

if [[ "${1:-}" == "--run" ]]; then
    echo "Running cargo bench..."
    cargo bench
    echo
elif [[ "${1:-}" == "--run-all" ]]; then
    echo "Running cargo bench with all features..."
    cargo bench --features "webhooks cache-json"
    echo
fi

if [[ ! -d "$CRITERION_DIR" ]]; then
    echo "No Criterion results found in $CRITERION_DIR"
    echo "Run 'cargo bench' first, or use './benches/bench-table.sh --run'"
    exit 1
fi

# ─── Tables ──────────────────────────────────────────────────────────────────

echo "### Format Selection (\`cargo bench -- format_selection\`)"
echo ""
echo "| Operation | 10 formats | 50 formats | 100 formats | 500 formats |"
echo "|---|---|---|---|---|"
for op in best_video_format best_audio_format worst_video_format worst_audio_format select_video_High_AVC1 select_audio_Best_Opus; do
    label="$op"
    # prettier labels
    case "$op" in
        select_video_High_AVC1) label="select_video High/AVC1" ;;
        select_audio_Best_Opus) label="select_audio Best/Opus" ;;
    esac
    r10=$(get_result format_selection "$op" 10)
    r50=$(get_result format_selection "$op" 50)
    r100=$(get_result format_selection "$op" 100)
    r500=$(get_result format_selection "$op" 500)
    echo "| \`$label\` | ${r10:-—} | ${r50:-—} | ${r100:-—} | ${r500:-—} |"
done

echo ""
echo "### Validation (\`cargo bench -- validation\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in \
    "validate_youtube_url/valid_youtube" \
    "validate_youtube_url/valid_youtu_be" \
    "validate_youtube_url/non_youtube" \
    "validate_youtube_url/invalid" \
    "sanitize_filename/normal" \
    "sanitize_filename/special_chars" \
    "sanitize_filename/unicode" \
    "sanitize_path/simple" \
    "sanitize_path/traversal"; do
    r=$(get_result validation "$op")
    label=$(echo "$op" | sed 's|/| — |')
    echo "| \`$label\` | ${r:-—} |"
done

echo ""
echo "### Model Operations (\`cargo bench -- model_operations\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in has_chapters get_chapters has_heatmap has_subtitle_language_en serialize_to_json deserialize_from_json; do
    r=$(get_result model_operations "$op")
    echo "| \`$op\` | ${r:-—} |"
done

echo ""
echo "### Config Builders (\`cargo bench -- config_builders\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in ManagerConfig_default ManagerConfig_builder_max5 PostProcessConfig_H264_AAC; do
    label="$op"
    case "$op" in
        ManagerConfig_default)      label="ManagerConfig::default()" ;;
        ManagerConfig_builder_max5) label="ManagerConfig::builder().max(5).build()" ;;
        PostProcessConfig_H264_AAC) label="PostProcessConfig H264/AAC" ;;
    esac
    r=$(get_result config_builders "$op")
    echo "| \`$label\` | ${r:-—} |"
done

echo ""
echo "### Chapter Operations (\`cargo bench -- chapter_ops\`)"
echo ""
echo "| Operation | 5 chapters | 20 chapters | 50 chapters | 100 chapters |"
echo "|---|---|---|---|---|"
for op in find_by_timestamp search_by_title contains_timestamp validate; do
    r5=$(get_result chapter_ops "$op" 5)
    r20=$(get_result chapter_ops "$op" 20)
    r50=$(get_result chapter_ops "$op" 50)
    r100=$(get_result chapter_ops "$op" 100)
    echo "| \`$op\` | ${r5:-—} | ${r20:-—} | ${r50:-—} | ${r100:-—} |"
done

echo ""
echo "### Heatmap Operations (\`cargo bench -- heatmap_ops\`)"
echo ""
echo "| Operation | 10 points | 100 points | 1 000 points |"
echo "|---|---|---|---|"
for op in most_engaged_segment get_highly_engaged_segments_0_7 get_point_at_time_42; do
    label="$op"
    case "$op" in
        get_highly_engaged_segments_0_7) label="get_highly_engaged_segments(0.7)" ;;
        get_point_at_time_42)            label="get_point_at_time(42.0)" ;;
    esac
    r10=$(get_result heatmap_ops "$op" 10)
    r100=$(get_result heatmap_ops "$op" 100)
    r1000=$(get_result heatmap_ops "$op" 1000)
    echo "| \`$label\` | ${r10:-—} | ${r100:-—} | ${r1000:-—} |"
done

echo ""
echo "### Playlist Operations (\`cargo bench -- playlist_ops\`)"
echo ""
echo "| Operation | 10 entries | 50 entries | 200 entries |"
echo "|---|---|---|---|"
for op in available_entries search_entries_by_title filter_by_uploader; do
    r10=$(get_result playlist_ops "$op" 10)
    r50=$(get_result playlist_ops "$op" 50)
    r200=$(get_result playlist_ops "$op" 200)
    echo "| \`$op\` | ${r10:-—} | ${r50:-—} | ${r200:-—} |"
done

echo ""
echo "### Format Type Detection (\`cargo bench -- format_type\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in format_type_video format_type_audio format_type_muxed format_type_manifest is_video is_audio; do
    label="$op"
    case "$op" in
        format_type_video)    label="format_type() — video-only" ;;
        format_type_audio)    label="format_type() — audio-only" ;;
        format_type_muxed)    label="format_type() — muxed" ;;
        format_type_manifest) label="format_type() — manifest" ;;
    esac
    r=$(get_result format_type "$op")
    echo "| \`$label\` | ${r:-—} |"
done

echo ""
echo "### Event Filter (\`cargo bench -- event_filter\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in event_filter_all_matches event_filter_only_terminal_match event_filter_only_terminal_no_match; do
    label="$op"
    case "$op" in
        event_filter_all_matches)             label="EventFilter::all().matches() (always true)" ;;
        event_filter_only_terminal_match)     label="EventFilter::only_terminal().matches() (terminal event)" ;;
        event_filter_only_terminal_no_match)  label="EventFilter::only_terminal().matches() (progress event, no match)" ;;
    esac
    r=$(get_result event_filter "$op")
    echo "| \`$label\` | ${r:-—} |"
done

echo ""
echo "### Speed Profiles — Optimal Segments (\`cargo bench -- speed_profile\`)"
echo ""
echo "| Profile | 1 MB | 50 MB | 500 MB | 2 GB |"
echo "|---|---|---|---|---|"
for profile in Conservative Balanced Aggressive; do
    r1=$(get_result speed_profile "$profile/calculate_optimal_segments" 1000000)
    r50=$(get_result speed_profile "$profile/calculate_optimal_segments" 50000000)
    r500=$(get_result speed_profile "$profile/calculate_optimal_segments" 500000000)
    r2g=$(get_result speed_profile "$profile/calculate_optimal_segments" 2000000000)
    echo "| $profile | ${r1:-—} | ${r50:-—} | ${r500:-—} | ${r2g:-—} |"
done

echo ""
echo "### Retry Strategy (\`cargo bench --features webhooks -- retry_strategy\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for attempt in 0 1 3; do
    r=$(get_result retry_strategy "delay_for_attempt" "$attempt")
    echo "| \`delay_for_attempt($attempt)\` | ${r:-—} |"
done
r_true=$(get_result retry_strategy "should_retry_true")
r_false=$(get_result retry_strategy "should_retry_false")
echo "| \`should_retry\` (true) | ${r_true:-—} |"
echo "| \`should_retry\` (false) | ${r_false:-—} |"

echo ""
echo "### Cache Operations (\`cargo bench --features cache -- cache_ops\`)"
echo ""
echo "| Operation | Time |"
echo "|---|---|"
for op in cache_put_single cache_get_hit cache_get_miss cache_put_500; do
    r=$(get_result cache_ops "$op")
    echo "| \`$op\` | ${r:-—} |"
done

echo ""
echo "Done! Copy the tables above into PROFILING.md's '📊 Benchmark result tables' section."
