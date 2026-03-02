You are an expert in Rust, async programming, and concurrent systems.

Key Principles
- All comments and docs must be in english. No french, in any comments or docs.
- The functions documentation must describe it, with arguments, errors and returns.
- Write clear, concise, and idiomatic Rust code with accurate examples.
- Use async programming paradigms effectively, leveraging `tokio` for concurrency.
- Prioritize modularity, clean code organization, and efficient resource management.
- Use expressive variable names that convey intent (e.g., `is_ready`, `has_data`).
- Adhere to Rust's naming conventions: snake_case for variables and functions, PascalCase for types and structs.
- Avoid code duplication; use functions and modules to encapsulate reusable logic.
- All `use` imports must be at the top of the file (module-level), never inside function bodies. Use `#[cfg(...)]` on the import when it is platform-specific. The only exception is inside `macro_rules!` definitions where `$crate::` paths require local imports.
- Write code with safety, concurrency, and performance in mind, embracing Rust's ownership and type system.
- Use `impl Into<String>`, `impl Into<PathBuf>`, `impl AsRef<str>` for public API parameters — not concrete `String` or `&str`.
- Use optimized types in function parameters according to the operations applied (borrowing vs owned, `&str` vs `String`, `&Path` vs `PathBuf`).
- No `#[cfg(test)]` modules in `src/`. Tests are done via doctests (`cargo test --doc`), benchmarks (`benches/benchmarks.rs` with criterion), and integration examples (`examples/`).

Project Architecture

The workspace contains two crates:

```
Desktop/yt-dlp/
├── Cargo.toml               ← workspace manifest ([workspace] + [package] for yt-dlp)
├── src/                     ← yt-dlp crate source
└── crates/
    └── media-seek/          ← standalone container index parsing crate
        ├── Cargo.toml
        └── src/
            ├── lib.rs       — RangeFetcher trait + parse() dispatch
            ├── error.rs     — Error enum + Result<T> alias
            ├── detect.rs    — magic-byte format detection
            ├── index.rs     — ContainerIndex, SegmentEntry, Inner
            ├── audio/       — mp3, ogg, flac, pcm (wav+aiff), adts
            └── video/       — mp4, webm, flv, avi, ts
```

The `yt-dlp` crate source follows a strict module hierarchy:

```
src/
├── lib.rs              # Crate root: `Downloader` struct lives here (not in a submodule)
├── prelude.rs          # Convenience re-exports for `use yt_dlp::prelude::*`
├── macros.rs           # Convenience macros: youtube!, ytdlp_args!, install_libraries!, ternary!, simple_hook!
├── error.rs            # Single unified Error enum + type Result<T> alias
├── client/             # Builder, download builder, proxy, dependency installation, stream orchestration
│   ├── builder.rs      # DownloaderBuilder (fluent builder for Downloader)
│   ├── download_builder.rs  # DownloadBuilder<'a> (fluent API for downloads)
│   ├── pipeline.rs     # Fluent pipeline API (fetch, download_and_continue, pipeline, postprocess, events)
│   ├── proxy.rs        # ProxyConfig, ProxyType
│   ├── deps/           # Dependency auto-installation (yt-dlp, ffmpeg via GitHub releases)
│   └── streams/        # Format selection, quality API, stream orchestration
│       ├── processing.rs    # Stream processing utilities
│       ├── selection.rs     # VideoSelection trait, format selection
│       ├── quality.rs       # Quality-based download API
│       ├── pipeline/        # Download pipeline steps
│       │   ├── fetch.rs     # Video info fetching, cache lookup, extractor selection
│       │   ├── download.rs  # Single-video/format download orchestration
│       │   ├── combine.rs   # Audio+video combining with FFmpeg
│       │   ├── partial.rs   # Partial/clip downloads
│       │   └── playlist.rs  # Playlist iteration and download
│       └── assets/          # Media asset downloads
│           ├── storyboard.rs
│           └── subtitle.rs
├── download/           # DownloadManager, Fetcher, segment-based parallel downloads
│   ├── api.rs          # Downloader download-manager API (priority, progress, status)
│   ├── manager.rs      # DownloadManager core
│   ├── engine/         # Download engine internals
│   │   ├── fetcher.rs  # HTTP fetcher with range support
│   │   ├── segment.rs  # Segment-based parallel download
│   │   ├── range_fetcher.rs  # Range request support
│   │   └── partial.rs  # Partial download support
│   └── config/         # Download configuration and post-processing
│       ├── progress.rs # Progress tracking
│       ├── speed_profile.rs  # Speed profiles
│       └── postprocess.rs    # Post-processing config
├── events/             # EventBus, DownloadEvent, EventFilter, hooks, webhooks
│   ├── bus.rs          # Event bus (broadcast)
│   ├── types.rs        # DownloadEvent enum
│   ├── filters.rs      # EventFilter predicates
│   ├── retry.rs        # Retry event logic
│   └── delivery/       # Event delivery mechanisms
│       ├── hooks.rs    # Rust hooks (feature: hooks)
│       └── webhooks.rs # HTTP webhooks (feature: webhooks)
├── executor/           # Executor (process runner), FfmpegArgs builder, temp-file+rename pattern
├── extractor/          # VideoExtractor trait, Youtube extractor, Generic extractor, URL detection
├── metadata/           # MetadataManager, metadata writing, chapter injection
│   ├── api.rs          # Public metadata API
│   ├── base.rs         # Base metadata operations
│   ├── chapters.rs     # Chapter injection
│   ├── postprocess.rs  # Metadata post-processing
│   └── writers/        # Format-specific metadata writers
│       ├── ffmpeg.rs   # FFmpeg-based metadata writing
│       ├── lofty.rs    # Lofty-based metadata writing
│       ├── mp3.rs      # MP3-specific metadata
│       └── mp4.rs      # MP4-specific metadata
├── model/              # Data types: Video, Format, Chapter, Playlist, Caption, Thumbnail, Heatmap
│   ├── video.rs        # Video struct
│   ├── format.rs       # Format struct and related types
│   ├── selector.rs     # VideoQuality, AudioQuality, codec preference enums
│   ├── utils/          # serde helpers (json_none)
│   └── types/          # Auxiliary model types
│       ├── caption.rs  # Caption/subtitle metadata
│       ├── chapter.rs  # Chapter metadata
│       ├── heatmap.rs  # Heatmap data
│       ├── playlist.rs # Playlist metadata
│       └── thumbnail.rs # Thumbnail metadata
├── cache/              # VideoCache, DownloadCache, PlaylistCache (feature-gated)
│   ├── config.rs       # Cache configuration
│   ├── layer.rs        # CacheLayer (tiered L1+L2)
│   ├── stores/         # Individual cache stores
│   │   ├── files.rs    # Download file cache
│   │   ├── video.rs    # Video metadata cache
│   │   └── playlist.rs # Playlist cache
│   └── backend/        # Backend trait abstractions + implementations
│       ├── memory.rs   # Moka in-memory (L1)
│       ├── json.rs     # JSON file (L2)
│       ├── redb.rs     # Embedded redb (L2)
│       └── redis.rs    # Distributed Redis (L2)
├── live/               # Live stream recording (feature: live-recording)
│   ├── hls.rs          # HLS manifest parsing via m3u8-rs
│   ├── recording.rs    # LiveRecorder — reqwest-based HLS segment recorder (primary)
│   └── ffmpeg_recording.rs  # FfmpegLiveRecorder — FFmpeg-based recorder (fallback)
├── stats/              # StatisticsTracker, GlobalSnapshot (feature: statistics)
└── utils/              # fs, http, platform, validation, subtitle
    ├── fs.rs           # Filesystem utilities
    ├── http.rs         # HTTP utilities
    ├── platform.rs     # Platform detection
    ├── validation.rs   # Input validation
    ├── network/        # Network-related utilities
    │   ├── retry.rs    # Retry logic
    │   └── url_expiry.rs  # URL expiration handling
    └── subtitle/       # Subtitle conversion and validation
```

Module Conventions:
- Each directory has a `mod.rs` that declares submodules and re-exports all public types via `pub use`.
- `lib.rs` re-exports the most-used types to the crate root: `pub use client::{DownloadBuilder, DownloaderBuilder};`.
- `prelude.rs` re-exports everything users need for basic usage, feature-gated with `#[cfg(feature = "...")]`.
- Module-level `//!` doc comments describe the module's purpose and architecture.
- Feature-gated modules declared with `#[cfg(feature = "...")] pub mod cache;` in `lib.rs`.

File & Module Size Constraints:
- **No file may exceed 1000 lines.** When a file approaches the limit, split it into focused submodules.
- **No module directory may contain more than 5 source files** (excluding `mod.rs`). Create submodule directories to group related files.
- **`src/` root may only contain**: `error.rs`, `lib.rs`, `macros.rs`, `prelude.rs`. All other code must live in submodules (`client/`, `download/`, `events/`, etc.).

Visibility Conventions:
- `pub` — For types and methods exposed to library users.
- `pub(crate)` — For all fields of `Downloader` (libraries, output_dir, args, user_agent, timeout, proxy, cache, download_manager, cancellation_token, event_bus, youtube_extractor, generic_extractor, hook_registry, webhook_delivery, statistics) and internal helpers. Public getter methods expose read access (e.g. `libraries()`, `output_dir()`, `args()`, `timeout()`, `proxy()`, `cache()`, `download_manager()`, `event_bus()`, `statistics()`).
- Private — Default for implementation details that don't need crate-wide access.
- Builder struct fields are private; `TypedBuilder` config struct fields are `pub`.

Error Handling

Single unified error type in `src/error.rs`:

```rust
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    // ==================== Runtime & System Errors ====================
    #[error("IO error during {operation}")]
    IO { operation: String, path: Option<PathBuf>, #[source] source: std::io::Error },
    // ... more variants grouped by category with comment banners
}
```

Rules:
- **One `Error` enum** for the whole crate. Variants grouped by category with `// ===  ===` comment banners.
- **Type alias** `pub type Result<T>` in `error.rs`, imported as `use crate::error::Result;`.
- **Structured named fields** on every variant — never just a string. Use fields like `operation`, `context`, `url`, `reason`, `path`, `source`.
- **`#[source]`** on the inner error field for proper error chaining.
- **Helper constructors** with embedded tracing: `Error::io(operation, source)`, `Error::http(url, context, source)`, etc. Each logs `tracing::warn!` or `tracing::error!` with structured fields before constructing.
- **`From` impls** for common error types (`std::io::Error`, `reqwest::Error`, `serde_json::Error`, `JoinError`, `ZipError`, conditionally `redb::Error`, `redis::RedisError`). Each `From` impl also logs a tracing message with `"(automatic conversion)"` suffix.
- **Constructor parameter style**: `impl Into<String>` — not generics with trait bounds.
- **Feature-gated variants**: `#[cfg(feature = "cache-redb")] Database { ... }`, `#[cfg(feature = "cache-redis")] Redis { ... }`.
- **Only other error type**: `HookError` in `src/events/hooks.rs` (for hook execution failures).
- Never use `anyhow` — always the crate's own `Error` / `Result`.

Builder Patterns

Two builder styles coexist in the codebase:

**A) Manual builder (consuming `mut self`)** — For `DownloaderBuilder`, `DownloadBuilder`, `WebhookConfig`, `FfmpegArgs`:
```rust
pub fn with_timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
}
// Terminal method:
pub async fn build(self) -> Result<Downloader> { ... }
```
- Builder methods prefixed with `with_` (e.g. `with_args`, `with_timeout`, `with_proxy`, `with_cache`, `with_cookies`).
- Always `mut self` (consuming), **never `&mut self`**.
- Terminal method: `.build()` (async for `DownloaderBuilder`, sync for `FfmpegArgs`) or `.execute()` for `DownloadBuilder`.
- `DownloadBuilder<'a>` holds a reference `&'a Downloader`.
- Builder struct fields are private.

**Post-build mutation methods on `Downloader`** — After `.build()`, use `set_*`/`add_*` methods (not `with_*`) to mutate the instance:
```rust
downloader.set_user_agent("my-agent");
downloader.set_timeout(Duration::from_secs(30));
downloader.set_args(vec!["--no-playlist".into()]);
downloader.add_arg("--flat-playlist");
downloader.set_cookies("cookies.txt");
downloader.set_cookies_from_browser("chrome");
downloader.set_netrc();
```
- These take `&mut self` (borrowing), return `&mut Self` for chaining.
- Prefix: `set_` for replacing a value, `add_` for appending.

**B) `TypedBuilder` derive** — For config structs (`ManagerConfig`, `RetryPolicy`, `ExpiryConfig`):
```rust
#[derive(Debug, Clone, TypedBuilder)]
pub struct ManagerConfig {
    #[builder(default = SpeedProfile::default().max_concurrent_downloads())]
    pub max_concurrent_downloads: usize,
    // ...
}
```
- All fields are `pub`.
- Uses `#[builder(default = ...)]` for defaults.

Model & Data Types

Standard derive sets:
- **Simple enums**: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]` + `Default` with `#[default]` on a variant.
- **Complex structs** (with `f64` fields): `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` — `Eq`/`Hash` implemented manually.
- **Simple structs** (no floats): `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`.

Serde patterns:
- `#[serde(flatten)]` for struct composition: `Format` flattens `CodecInfo`, `VideoResolution`, `DownloadInfo`, `QualityInfo`, `FileInfo`, `StoryboardInfo`, `RatesInfo`.
- `#[serde(rename = "...")]` on fields: `#[serde(rename = "timestamp")]`, `#[serde(rename = "acodec")]`.
- `#[serde(rename_all = "snake_case")]` or `#[serde(rename_all = "PascalCase")]` on enums.
- `#[serde(default)]` on optional collections and fields.
- `#[serde(other)]` on `Unknown` variant for forward compatibility.
- `#[serde(skip)]` for derived/internal fields (e.g. `video_id` on `Format`).
- Custom deserializer `json_none` in `model/utils/serde.rs` — turns `"none"` strings to `Option::None`.
- `#[serde_as(deserialize_as = "DefaultOnNull")]` from `serde_with` (e.g. on `Video.chapters`).
- Custom `Deserialize` impl with visitor for polymorphic types (e.g. `DrmStatus` accepts both bool and string).
- `ordered_float::OrderedFloat<f64>` is used only when floating-point values need `Hash`/`Eq`.

Display format — **always** `TypeName(key=value, key=value)`:
```rust
impl fmt::Display for Video {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Video(id={}, title={:?}, channel={:?}, formats={})",
            self.id, self.title, self.channel.as_deref().unwrap_or("Unknown"), self.formats.len())
    }
}
```
- Only include essential identifying fields — never full serialization.
- Use `as_deref().unwrap_or("none")` or `unwrap_or("unknown")` for `Option` fields.
- Enum variants in Display: `f.write_str("VariantName")` for constant strings, `write!(f, "Variant(key={})", val)` with fields.

Custom `Hash` impls — hash only identity fields:
```rust
impl Hash for Video {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.title.hash(state);
        self.channel.hash(state);
        self.channel_id.hash(state);
    }
}
```

Trait Design Patterns

**`#[async_trait]`** — For traits used as `dyn Trait` (trait objects):
```rust
#[async_trait]
pub trait VideoExtractor: Downcast + Send + Sync + fmt::Debug {
    async fn fetch_video(&self, url: &str) -> Result<Video>;
    fn name(&self) -> ExtractorName;
    fn supports_url(&self, url: &str) -> bool;
}
impl_downcast!(VideoExtractor);
```
- `downcast_rs::Downcast` + `impl_downcast!` for runtime downcasting.
- Always `Send + Sync + fmt::Debug` bounds.

**RPITIT (Return Position Impl Trait In Trait)** — For traits dispatched via concrete enum (not `dyn`):
```rust
pub trait VideoBackend: Send + Sync + std::fmt::Debug {
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Video>>> + Send;
    fn put(&self, url: String, video: Video) -> impl Future<Output = Result<()>> + Send;
}
```
- Used for cache backend traits because dispatch is via enum (`VideoBackendEnum`), not `Box<dyn>`.

**Clonable trait objects** — `dyn_clone::DynClone` + `clone_trait_object!`:
```rust
pub trait EventHook: DynClone + Send + Sync { ... }
dyn_clone::clone_trait_object!(EventHook);
```

**When to use each pattern**:
- `async_trait` → trait will be used behind `Box<dyn Trait>` or `Arc<dyn Trait>`.
- RPITIT → trait dispatched via concrete enum or generic, never `dyn`.
- Trait method declarations carry full rustdoc; impls may add only a brief clarifying comment.

Shared State & Concurrency

Primitives used:
- `Arc<reqwest::Client>` — Shared HTTP client with connection pooling.
- `Arc<Mutex<...>>` — Mutable shared state: download queues, task maps, next_id counter.
- `Arc<Semaphore>` — Concurrency limit for parallel downloads.
- `Arc<AtomicU64>` / `Arc<AtomicBool>` — Lock-free counters and flags.
- `Arc<RwLock<...>>` — Read-heavy shared state: hook registry, stats, webhooks.
- `Arc<DownloadEvent>` — Events in broadcast channel (efficient cloning).
- `Arc<dyn Fn(...) + Send + Sync>` — Callbacks and filter predicates.
- `tokio_util::sync::CancellationToken` — Graceful shutdown.

Rules:
- `tokio::sync::Mutex` and `tokio::sync::RwLock` for async contexts.
- `std::sync::Mutex` only for `ProgressCounters` and other non-async contexts (progress callbacks from sync closures).
- Never hold a `tokio` lock across `.await` points.
- Prefer `Arc<AtomicU64>` over `Arc<Mutex<u64>>` for simple counters.
- Caches stored as `Option<Arc<CacheLayer>>` on `Downloader`.

Async Programming
- Use `tokio` as the async runtime for handling asynchronous tasks and I/O.
- Implement async functions using `async fn` syntax.
- Leverage `tokio::spawn` for task spawning and concurrency.
- Use `tokio::select!` for managing multiple async tasks and cancellations.
- Favor structured concurrency: prefer scoped tasks and clean cancellation paths.
- Implement timeouts, retries, and backoff strategies for robust async operations.
- Avoid blocking inside async functions; offload to `tokio::task::spawn_blocking` (used for `serde_json::from_reader` and CPU-intensive parsing).
- Use `tokio::time::sleep` and `tokio::time::interval` for time-based operations.

Channels and Concurrency
- `tokio::sync::mpsc` for async multi-producer, single-consumer channels (webhook delivery queue).
- `tokio::sync::broadcast` for event broadcasting to multiple subscribers.
- `tokio::sync::oneshot` for one-time communication between tasks.
- Prefer bounded channels for backpressure; handle capacity limits gracefully.

Event System

Architecture in `src/events/`:
- `EventBus` wraps `broadcast::Sender<Arc<DownloadEvent>>`. Events wrapped in `Arc` for efficient cloning.
- `DownloadEvent`: Large enum with `#[allow(clippy::large_enum_variant)]`. **All variants use named fields** (no tuple variants).
- `EventFilter`: Predicate-based with `Vec<Arc<dyn Fn(&DownloadEvent) -> bool + Send + Sync>>`. Builder-style with `and_then()`. Factory methods: `all()`, `only_terminal()`, `only_completed()`, `download_id(id)`.
- `HookRegistry`: `Arc<RwLock<Vec<Box<dyn EventHook>>>>` — supports parallel and sequential execution.
- `simple_hook!` macro for creating hooks from closures.

Event emission pattern — three-phase delivery in `Downloader::emit_event()`:
1. Hooks (with timeout, `#[cfg(feature = "hooks")]`)
2. Webhooks (non-blocking, `#[cfg(feature = "webhooks")]`)
3. Broadcast bus (always)

Feature Flags & Conditional Compilation

Features in `Cargo.toml`:
- `default = ["reqwest/default", "cache-memory"]`
- `hooks`, `webhooks`, `statistics` — zero-dependency feature flags.
- **Cache hierarchy**: `cache-memory` (Moka in-memory), `cache-json` (JSON files), `cache-redb` (embedded redb), `cache-redis` (distributed Redis). The `cache` cfg is emitted by `build.rs` when any of these is enabled.
- `live-recording` — live stream recording via HLS (pulls `m3u8-rs`).
- `rustls` — optional TLS backend.
- `hickory-dns` — optional async DNS resolver (passes `reqwest/hickory-dns`).
- `profiling` — optional `dhat` heap profiler.

Cache backend selection via `build.rs`:
- Emits `cache` when any cache backend (`cache-memory`, `cache-json`, `cache-redb`, `cache-redis`) is enabled.
- Emits `persistent_cache` when any of `cache-json`, `cache-redb`, or `cache-redis` is enabled.
- Emits `multiple_persistent_backends` (triggers `compile_error!`) if more than one persistent backend is active.
- Architecture: tiered L1 (Moka, `#[cfg(feature = "cache-memory")]`) + L2 (persistent, `#[cfg(persistent_cache)]`).

Usage patterns:
- `#[cfg(cache)]` — single guard for all cache code (emitted by `build.rs`, not a Cargo feature).
- `#[cfg(feature = "cache-json")]` — backend-specific module declarations and imports.
- `#[cfg(persistent_cache)]` — guard for any persistent backend code.
- `#[cfg(feature = "hooks")]` — module declarations, struct fields, `pub use` exports.
- `#[cfg(feature = "live-recording")]` — live recording module, error variants, event variants, executor streaming.

HTTP Client Configuration

- `reqwest` features: `json`, `stream`, `http2`, `charset`, `gzip`, `brotli` — all enabled unconditionally.
- `tcp_nodelay(true)` on the client builder to disable Nagle's algorithm.
- Range support probing uses `GET` with `Range: bytes=0-0` header (not `HEAD`) for CDN compatibility; response is validated via `Content-Range` header.
- Progress callbacks are throttled to 50 ms intervals via `AtomicU64` timestamp comparison (`PROGRESS_THROTTLE_NANOS`), bypassed only for the final update.

Process Execution

- `Executor` in `src/executor/mod.rs`: Wraps `tokio::process::Command` with piped stdout/stderr and timeout.
- `ProcessOutput`: Struct with `stdout: String`, `stderr: String`, `code: i32`.
- Timeout pattern: `tokio::time::timeout` + `process.kill()` on timeout.
- Windows-specific: `#[cfg(target_os = "windows")]` with `command.creation_flags(0x08000000)` (CREATE_NO_WINDOW).
- `FfmpegArgs` builder: Fluent API — `.input()`, `.codec_copy()`, `.args()`, `.overwrite()`, `.output()`, `.build()`.
- **Temp file + rename pattern**: FFmpeg operations write to a temp file then rename atomically via `run_ffmpeg_with_tempfile()`.
- **CPU-intensive JSON parsing** via `tokio::task::spawn_blocking` to avoid blocking the async runtime.

Constants & Magic Numbers

- **No magic numbers or magic byte patterns in logic.** Every literal number (sizes, offsets, masks, multipliers, thresholds, flags) must be extracted to a named `const` at the top of the file with a brief doc comment.
- Module-private constants at file top: `const DEFAULT_RETRY_ATTEMPTS: usize = 3;`, `const BALANCED_SEGMENT_SIZE: usize = 8 * 1024 * 1024;`.
- Naming: `SCREAMING_SNAKE_CASE`, often prefixed with context: `DEFAULT_`, `CONSERVATIVE_`, `BALANCED_`, `AGGRESSIVE_`.
- Public constants: `pub const FORMAT_URL_LIFETIME: i64 = 6 * 3600;`.
- Configuration structs via `TypedBuilder` with `#[builder(default = ...)]`.
- Lookup tables (e.g., bitrate tables, sample rate tables) are `const` arrays at file top, never inline in match arms.
- Magic byte sequences for format detection use named constants: `const EBML_MAGIC: &[u8] = &[0x1A, 0x45, 0xDF, 0xA5];` — never raw `&[0x1A, ...]` in conditionals.

Return Types

- **Never return tuples from functions.** Use a named struct instead.
- Even for two-field returns, create a small struct with descriptive field names.
- The struct can be module-private if only used internally.
- Example:
```rust
// ❌ BAD — Opaque meaning at call site
fn find_range(&self, time: f64) -> Option<(u64, u64)> { ... }

// ✅ GOOD — Clear field semantics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}
fn find_range(&self, time: f64) -> Option<ByteRange> { ... }
```

Function Call & Type Qualification

- **Function calls**: qualify with at most one `::` — use `Self::method()`, `module::function()`, or `Type::method()`. Never `module::submodule::function()` — import the submodule or function instead.
- **Type paths**: frequently-used types must be imported directly. Avoid double-qualified paths like `reqwest::header::HeaderMap` — instead import `use reqwest::header::{self, HeaderMap, HeaderValue};` and use `HeaderMap` or `header::CONTENT_TYPE`.
- **Self qualification**: prefer `Self::` for calling associated functions and methods within an `impl` block.
- **Crate paths**: use `crate::module::Type` in imports, then use the short name in code.

Tracing & Logging Guidelines
- Tracing is an unconditional dependency (no feature flag). Every important function must have tracing.
- Always use fully-qualified macros: `tracing::debug!(...)`, `tracing::info!(...)`, etc. Never import the macros. Never use `#[instrument]`.
- Always use structured fields, never format!()-style interpolation in messages:
  - GOOD: `tracing::debug!(url = %url, timeout = ?timeout, "📥 Starting download")`
  - BAD: `tracing::debug!("Starting download for {}", url)`
- Field syntax: `key = value` for Display, `key = ?value` for Debug, `key = %value` for explicit Display.

Log Level Rules:
- `trace`: Hot paths, comparisons, pure data transforms (should be rare — prefer deleting over trace)
- `debug`: Function entry/exit, parameters, config steps, internal operations
- `info`: Key workflow milestones only (download start/end, fetch, install, combine, postprocess, playlist, shutdown)
- `warn`: Recoverable failures, retries, fallbacks — NO emoji prefix on warn
- `error`: Unrecoverable per-item failures — NO emoji prefix on error

Emoji Prefixes (mandatory on all trace/debug/info messages):
Every tracing message string must start with one domain emoji followed by a space:
| Emoji | Domain                        |
|-------|-------------------------------|
| 📦    | Install / dependencies        |
| 📡    | Fetch / extract               |
| 📥    | Download                      |
| 🎬    | Combine / mux                 |
| ✂️    | Postprocess / ffmpeg          |
| 🏷️    | Metadata                      |
| 💬    | Subtitle                      |
| 🖼️    | Thumbnail                     |
| 📋    | Playlist                      |
| ✅    | Success / completion          |
| 🔄    | Retry / update                |
| 🔧    | Config / setup / builder      |
| 🔍    | Cache / lookup                |
| ⚙️    | Internal / utility            |
| 📊    | Statistics                    |
| 🔔    | Events                        |
| 🧩    | Format selection              |
| 🛑    | Shutdown                      |

What NOT to trace (delete tracing from these):
- Trivial getters/setters that just return or set a field
- Pure transforms (e.g., `to_ffmpeg_name`, `is_empty`, enum-to-string conversions)
- Simple constant lookups / match on enum returning a value

Rustdoc Guidelines
Every public function, method, and trait method must have a rustdoc comment following this format:

```rust
/// Brief one-line description of what the function does.
///
/// Optional extended description with more details, context, or behavior notes.
///
/// # Arguments
///
/// * `param_name` - Description of the parameter
/// * `other_param` - Description of the other parameter
///
/// # Errors
///
/// Returns an error if the file cannot be created or written.
///
/// # Returns
///
/// Description of the return value.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::prelude::*;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let downloader = Downloader::new("yt-dlp", "ffmpeg").await?;
/// let result = downloader.some_method().await?;
/// # Ok(())
/// # }
/// ```
```

Section rules:
- `# Arguments` — Include only when the function has parameters beyond `&self`/`&mut self`. List each parameter with `* \`name\` - description`.
- `# Errors` — Include only when the function returns `Result`. Describe what conditions cause errors.
- `# Returns` — Include only when the function returns a value (not `-> ()` or no return type). Describe what is returned, including `None`/`Ok`/`Err` semantics.
- `# Examples` — Include on main public API entry points (the functions users call first: `Downloader::new`, `download`, `fetch`, `combine`, `pipeline`, `postprocess`, etc.). Use `no_run` or `ignore` for examples requiring network/binaries. Follow the patterns in `lib.rs` and `README.md`.
- Trait method declarations must have full rustdoc in the trait definition. Implementations may add a brief clarifying comment but should not duplicate the trait docs.
- Simple getters/setters still need at minimum a one-liner description + `# Returns` (for getters) or `# Arguments` (for setters with params).
- Builder methods need at minimum a one-liner + `# Arguments` for their parameter.

Macros

Defined in `src/macros.rs` and `src/events/hooks.rs`:
- `youtube!($yt_dlp, $ffmpeg, $output)` — Convenience constructor for `Downloader`.
- `ytdlp_args![...]` — Args builder (string list or key-value pairs).
- `install_libraries!($dir)` — Async binary installation.
- `ternary!($cond, $true, $false)` — Ternary operator.
- `simple_hook!` — Create an `EventHook` from a closure.

All macros use `$crate::` fully-qualified paths for robustness. Local `use` inside `macro_rules!` is the only exception to the top-level import rule.

media-seek Conventions

`crates/media-seek/` is a standalone crate with its own `Cargo.toml`. It is pure parsing — no `async_trait`, no `serde`, no `reqwest`.

- All tracing follows the same rules as `yt-dlp` above (⚙️ for internal/utility, ✅ for success, structured fields, fully-qualified macros).
- Error constructors in `error.rs` embed tracing (same pattern as `yt-dlp`): `parse()` logs `warn!`, `fetch()` logs `warn!`.
- `Result<T>` alias defined in `error.rs` and re-exported from `lib.rs` as `pub use error::{Error, Result}`.
- Parser modules live under `src/audio/` (mp3, ogg, flac, pcm, adts) and `src/video/` (mp4, webm, flv, avi, ts). Each is `pub(crate)`.
- `RangeFetcher` trait uses RPITIT (not `#[async_trait]`) because dispatch is via concrete type, never `dyn`.
- No feature flags in `media-seek`. All formats are always compiled in.
- `ByteRange { start, end }` returned by `ContainerIndex::find_byte_range()` — never tuples.
- All magic numbers (sync bytes, header sizes, sample rates, bitrate tables) are documented `const` at file top.
- `dedup_by_key` must be applied after sorting by the **same key** used for dedup. If downstream needs a different sort order, sort again after dedup.

Verification

All edits must pass these checks:
```bash
# Lint each feature in isolation (workspace-wide, covers both yt-dlp and media-seek)
cargo hack clippy --workspace --each-feature --exclude-all-features -- -D warnings

# Lint tiered cache combinations (L1 Moka + L2 persistent)
cargo clippy --workspace --features cache-memory,cache-json -- -D warnings
cargo clippy --workspace --features cache-memory,cache-redb -- -D warnings
cargo clippy --workspace --features cache-memory,cache-redis -- -D warnings

# Check formatting (requires nightly)
cargo +nightly fmt --all -- --check

# Run all doc-tests (workspace-wide)
cargo test --doc --workspace

# Check dependencies (licenses, advisories, bans)
cargo deny check

# Check for unused dependencies
cargo machete
```