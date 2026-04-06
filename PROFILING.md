# 🔬 Profiling Guide

This document covers all profiling and benchmarking tools available in this library.

---

## Enable the feature

Add the `profiling` feature to your `Cargo.toml`:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["profiling"] }
```

This enables the `dhat` global allocator hook so heap profiling data is collected automatically when running the profiling example.

---

## 🔥 CPU Profiling with flamegraph

```bash
cargo install flamegraph

# Profile a real URL
cargo flamegraph --bench profiling --features profiling --release -- <URL>

# Profile without network (dry-run mode)
cargo flamegraph --bench profiling --features profiling --release -- --dry-run

# Output is written to flamegraph.svg in the current directory
```

Open `flamegraph.svg` in any browser to explore the interactive call-graph. Artefact path: `flamegraph.svg` (excluded by `.gitignore`).

---

## 🎯 CPU Profiling with samply (macOS/Linux)

```bash
cargo install samply

# Build first, then profile
cargo build --bench profiling --features profiling --release
samply record ./target/release/deps/profiling-* <URL>

# samply opens the Firefox profiler UI automatically
```

samply captures kernel-level samples with minimal overhead. Use `--dry-run` to profile pure-Rust paths without network.

---

## 🧠 Heap Profiling with dhat-rs

```bash
# Run the bench — dhat-heap.json is written on exit
cargo bench --bench profiling --features profiling -- <URL>

# Dry-run (no network)
cargo bench --bench profiling --features profiling -- --dry-run

# Open the viewer at:
# https://nnethercote.github.io/dh_view/dh_view.html
# then load dhat-heap.json
```

The `dhat-heap.json` file is written to the working directory and excluded by `.gitignore`. The online viewer shows total bytes allocated, live bytes at peak, and per-call-site allocation traces.

---

## 📊 Heap Profiling with heaptrack (Linux only)

```bash
# Build without the profiling feature (heaptrack intercepts malloc at the OS level)
cargo build --bench profiling --release

heaptrack ./target/release/deps/profiling-* <URL>

# Analyse via CLI
heaptrack --analyze heaptrack.profiling.*

# Or open the GUI
heaptrack_gui heaptrack.profiling.*
```

heaptrack output files match `heaptrack.profiling.*` and are excluded by `.gitignore`. Output files are written to `profiling-output/` if you redirect there, or to the current directory.

---

## 📈 Criterion Micro-benchmarks

All benchmarks are pure Rust with no network calls.

```bash
# Run all benchmark groups
cargo bench

# Run a specific group
cargo bench -- format_selection
cargo bench -- validation
cargo bench -- model_operations
cargo bench -- config_builders
cargo bench -- chapter_ops
cargo bench -- heatmap_ops
cargo bench -- playlist_ops
cargo bench -- format_type
cargo bench -- event_filter
cargo bench -- speed_profile

# Feature-gated groups
cargo bench --features webhooks -- retry_strategy
cargo bench --features cache -- cache_ops

# HTML reports are written to:
# target/criterion/<group>/<benchmark>/report/index.html
```

Open any `index.html` to see Criterion's interactive charts with violin plots and regression analysis.

---

## 🎬 FFmpeg Operations Profiling

FFmpeg is used for several operations in the library: combining separate audio and video streams,
re-encoding with post-processing filters, embedding metadata and chapters, and extracting
partial ranges. These are the most CPU-intensive operations and are measured through the
profiling harness.

### Available scenarios in `benches/profiling.rs`

| Scenario | FFmpeg operation |
|---|---|
| `download_video` | Full pipeline — includes combine (video + audio → mp4) |
| `postprocess` | Re-encode with H.264/AAC codec (`PostProcessConfig`) |

```bash
# Profile the combine step (inside download_video)
cargo flamegraph --bench profiling --features profiling --release -- <URL> --scenario download_video

# Profile post-processing only
cargo flamegraph --bench profiling --features profiling --release -- <URL> --scenario postprocess

# Or use samply for a more detailed macOS/Linux profile
cargo build --bench profiling --features profiling --release
samply record ./target/release/deps/profiling-* <URL> --scenario postprocess
```

### Heap allocation during FFmpeg arg construction

The `config_builders` Criterion benchmark (`cargo bench -- config_builders`) measures the pure-Rust
cost of building `PostProcessConfig` argument vectors, which is useful for confirming zero-copy
argument construction before FFmpeg is even invoked.

### Reading flamegraph results

When profiling `download_video`, look for these call stacks:
- `execute_command` → FFmpeg subprocess spawn (combine step)
- `metadata::postprocess::apply` → metadata injection
- `download::fetcher` → parallel HTTP segment downloads

The `postprocess` scenario isolates the FFmpeg re-encode step from the download phase.

---

## ⚡ Compare: raw yt-dlp vs library

`benches/compare.rs` benchmarks **every scenario from the README performance tables**. For each
scenario it measures raw `yt-dlp` (metadata + download in one subprocess) and the three library
speed profiles (**Conservative**, **Balanced**, **Aggressive**), then prints the results as
Markdown tables ready to paste into the README.

### Quick start

```bash
# Default: 3 runs per scenario
cargo bench --bench compare --features profiling -- <URL> --cookies-from-browser safari

# Custom run count for better statistical accuracy
cargo bench --bench compare --features profiling -- <URL> --cookies-from-browser safari --runs 5
```

### What it measures

The tool iterates over **14 scenarios** grouped into 4 sections:

| Section | Scenarios | yt-dlp `-f` | Library API |
|---|---|---|---|
| **🎵 Audio streams** | 96 kbps (Low), 128 kbps (Medium), 192 kbps (High), best | `bestaudio[abr<=N]` | `download_audio_stream_with_quality` |
| **🎬 Video streams** | 480p, 720p, 1080p, best | `bestvideo[height<=N]` | `download_video_stream_with_quality` |
| **📦 Native muxed** | 360p (mp4), 720p (mp4) | `best[height<=N][ext=mp4]` | `download_format` on `AudioVideo` formats |
| **📦 FFmpeg muxed** | 480p, 720p, 1080p, best | `bestvideo[height<=N]+bestaudio` | `download_video_with_quality` |

---

## 💡 Dry-run mode

All scenarios that do not require network access run fine with `--dry-run`. This lets you measure pure-Rust code paths (format selection, validation, config builders, event bus, etc.) without any yt-dlp or ffmpeg binaries:

```bash
cargo bench --bench profiling --features profiling -- --dry-run
```

You can focus on a single scenario:

```bash
cargo bench --bench profiling --features profiling -- --dry-run --scenario format_selection
cargo bench --bench profiling --features profiling -- --dry-run --scenario event_bus
cargo bench --bench profiling --features profiling -- --dry-run --scenario validation
```

---

## 📊 Benchmark result tables

Run the benchmarks then auto-generate the Markdown tables:

```bash
# Option 1: run benchmarks + print tables in one go
./scripts/bench-table.py --run

# Option 2: run with all feature-gated groups (webhooks, cache)
./scripts/bench-table.py --run-all

# Option 3: run benchmarks manually, then parse existing results
cargo bench --features "webhooks cache-json" --release
./scripts/bench-table.py
```

The script reads `target/criterion/*/new/estimates.json` and outputs tables ready to paste below.

### Format Selection (`cargo bench -- format_selection`)

| Operation | 10 formats | 50 formats | 100 formats | 500 formats |
|---|---|---|---|---|
| `best_video_format` | | | | |
| `best_audio_format` | | | | |
| `worst_video_format` | | | | |
| `worst_audio_format` | | | | |
| `select_video High/AVC1` | | | | |
| `select_audio Best/Opus` | | | | |

### Validation (`cargo bench -- validation`)

| Operation | Time |
|---|---|
| `validate_youtube_url` (valid) | |
| `validate_youtube_url` (youtu.be) | |
| `validate_youtube_url` (non-YT) | |
| `validate_youtube_url` (invalid) | |
| `sanitize_filename` (normal) | |
| `sanitize_filename` (special chars) | |
| `sanitize_filename` (unicode) | |
| `sanitize_path` (simple) | |
| `sanitize_path` (traversal) | |

### Model Operations (`cargo bench -- model_operations`)

| Operation | Time |
|---|---|
| `has_chapters` | |
| `get_chapters` | |
| `has_heatmap` | |
| `has_subtitle_language_en` | |
| `serialize_to_json` | |
| `deserialize_from_json` | |

### Config Builders (`cargo bench -- config_builders`)

| Operation | Time |
|---|---|
| `ManagerConfig::default()` | |
| `ManagerConfig::builder().max(5).build()` | |
| `PostProcessConfig H264/AAC` | |

### Chapter Operations (`cargo bench -- chapter_ops`)

| Operation | 5 chapters | 20 chapters | 50 chapters | 100 chapters |
|---|---|---|---|---|
| `find_by_timestamp` | | | | |
| `search_by_title` | | | | |
| `contains_timestamp` | | | | |
| `validate` | | | | |

### Heatmap Operations (`cargo bench -- heatmap_ops`)

| Operation | 10 points | 100 points | 1 000 points |
|---|---|---|---|
| `most_engaged_segment` | | | |
| `get_highly_engaged_segments(0.7)` | | | |
| `get_point_at_time(42.0)` | | | |

### Playlist Operations (`cargo bench -- playlist_ops`)

| Operation | 10 entries | 50 entries | 200 entries |
|---|---|---|---|
| `available_entries` | | | |
| `search_entries_by_title` | | | |
| `filter_by_uploader` | | | |

### Format Type Detection (`cargo bench -- format_type`)

| Operation | Time |
|---|---|
| `format_type()` — video-only | |
| `format_type()` — audio-only | |
| `format_type()` — muxed | |
| `format_type()` — manifest | |
| `is_video()` | |
| `is_audio()` | |

### Event Filter (`cargo bench -- event_filter`)

| Operation | Time |
|---|---|
| `EventFilter::all().matches()` (always true) | |
| `EventFilter::only_terminal().matches()` (terminal event) | |
| `EventFilter::only_terminal().matches()` (progress event, no match) | |

### Speed Profiles — Optimal Segments (`cargo bench -- speed_profile`)

| Profile | 1 MB | 50 MB | 500 MB | 2 GB |
|---|---|---|---|---|
| Conservative | | | | |
| Balanced | | | | |
| Aggressive | | | | |

### Retry Strategy (`cargo bench --features webhooks -- retry_strategy`)

| Operation | Time |
|---|---|
| `delay_for_attempt(0)` | |
| `delay_for_attempt(1)` | |
| `delay_for_attempt(3)` | |
| `should_retry` (true) | |
| `should_retry` (false) | |

### Cache Operations (`cargo bench --features cache -- cache_ops`)

| Operation | Time |
|---|---|
| `cache_put_single` | |
| `cache_get_hit` | |
| `cache_get_miss` | |
| `cache_put_500` | |

---

## 📁 Directory layout

| Path | Purpose |
|---|---|
| `profiling-libs/` | yt-dlp / ffmpeg binaries downloaded by the profiling example |
| `profiling-output/` | Video / audio files written during profiling runs |
| `dhat-heap.json` | dhat-rs heap profile output |
| `heaptrack.profiling.*` | heaptrack output files |
| `flamegraph.svg` | flamegraph CPU profile |

All of the above are excluded by `.gitignore`.