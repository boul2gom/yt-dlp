# 🤝 Contributing to yt-dlp

Thank you for your interest in contributing! This guide will help you understand our codebase conventions and write code that feels like it belongs here. Every rule exists because it's already applied consistently across the entire codebase — please follow them to keep things uniform.

---

## 📋 Table of Contents

- [🚀 Getting Started](#-getting-started)
- [🏗️ Project Architecture](#️-project-architecture)
- [✍️ Code Style](#️-code-style)
  - [🪆 Nesting depth](#-nesting-depth)
- [🚨 Error Handling](#-error-handling)
- [🔧 Builder Patterns](#-builder-patterns)
- [📦 Model & Data Types](#-model--data-types)
- [🧬 Trait Design](#-trait-design)
- [🔒 Shared State & Concurrency](#-shared-state--concurrency)
- [⚡ Async Programming](#-async-programming)
- [🔔 Event System](#-event-system)
- [🎯 Feature Flags](#-feature-flags)
- [📝 Tracing & Logging](#-tracing--logging)
- [📖 Documentation](#-documentation)
- [🔍 Contributing to media-seek](#-contributing-to-media-seek)
- [✅ Verification Checklist](#-verification-checklist)

---

## 🚀 Getting Started

### Prerequisites

- **Rust** (edition 2024) — install via [rustup](https://rustup.rs/)
- **Rust nightly** (for rustfmt) — `rustup toolchain install nightly --component rustfmt`
- **cargo-hack** — `cargo install cargo-hack`
- **cargo-deny** — `cargo install cargo-deny`

### Running the checks

Every PR must pass these commands:
```bash
# Lint all features combined (all backends in a single pass)
cargo clippy --workspace --all-features -- -D warnings

# Check formatting (requires nightly)
cargo +nightly fmt --all -- --check

# Run all doc-tests (workspace-wide)
cargo test --doc --workspace --all-features

# Check dependencies (licenses, advisories, bans)
cargo deny check

# Check for unused dependencies
cargo machete
```

### Branch workflow

1. Fork the repository and create a branch from `develop`
2. Make your changes following the guidelines below
3. Run the verification checks above
4. Open a PR against `develop`

---

## 🏗️ Project Architecture

The codebase is a Cargo workspace with two crates. Understanding the layout is essential before making changes:

```
yt-dlp/
├── Cargo.toml               ← workspace manifest ([workspace] + [package])
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

The `yt-dlp` crate module hierarchy:

```
src/
├── lib.rs              # 🏠 Crate root — Downloader struct lives here (NOT in a submodule)
├── prelude.rs          # 📤 Convenience re-exports for `use yt_dlp::prelude::*`
├── macros.rs           # 🧩 Macros: youtube!, ytdlp_args!, install_libraries!, ternary!
├── error.rs            # 🚨 Single unified Error enum + type Result<T>
│
├── client/             # 🔧 Builder, download builder, proxy, deps, stream orchestration
│   ├── builder.rs      #    DownloaderBuilder (fluent builder)
│   ├── download_builder.rs  # DownloadBuilder<'a> (fluent download API)
│   ├── proxy.rs        #    ProxyConfig, ProxyType
│   ├── deps/           #    📦 Auto-installation of yt-dlp & ffmpeg from GitHub releases
│   └── streams/        #    🧩 Format selection (VideoSelection trait), orchestration
│
├── download/           # 📥 DownloadManager, Fetcher, segment-based parallel downloads
├── events/             # 🔔 EventBus, DownloadEvent, EventFilter, hooks, webhooks
├── executor/           # ⚙️ Process runner, FfmpegArgs builder, temp-file+rename
├── extractor/          # 📡 VideoExtractor trait, Youtube & Generic extractors
├── metadata/           # 🏷️ MP3/MP4/FFmpeg/Lofty metadata writing, chapter injection
├── model/              # 📊 Data types: Video, Format, Chapter, Playlist, Caption, etc.
│   ├── utils/          #    Serde helpers
│   └── selector.rs     #    VideoQuality, AudioQuality, StoryboardQuality enums
├── cache/              # 🔍 VideoCache, DownloadCache, PlaylistCache (feature-gated)
│   └── backend/        #    Backend trait + implementations (memory/moka, json, redb, redis)
├── live/               # 🔴 Live recording/streaming (features: live-recording, live-streaming)
│   ├── hls.rs          #    HLS manifest parsing via m3u8-rs
│   ├── recording.rs    #    Reqwest-based HLS segment recorder (primary)
│   └── ffmpeg_recording.rs  # FFmpeg-based recorder (fallback)
├── stats/              # 📊 StatisticsTracker, GlobalSnapshot (feature: statistics)
└── utils/              # 🛠️ fs, http, platform, retry, validation, url_expiry, subtitle
```

### 📁 Module conventions

| Rule | Example |
|------|---------|
| Each directory has a `mod.rs` that declares submodules and re-exports public types | `pub use video::VideoCache;` in `cache/mod.rs` |
| `lib.rs` re-exports the most-used types to crate root | `pub use client::{DownloadBuilder, DownloaderBuilder};` |
| `prelude.rs` re-exports everything for basic usage | Feature-gated with `#[cfg(feature = "...")]` |
| Module-level `//!` doc comments on every `mod.rs` | Describes the module's purpose and architecture |
| Feature-gated modules in `lib.rs` | `#[cfg(feature = "statistics")] pub mod stats;` |

### 👁️ Visibility rules

| Visibility | When to use | Example |
|-----------|-------------|---------|
| `pub` | Types and methods exposed to library users | `pub fn fetch_video_infos(...)` |
| `pub(crate)` | All fields of `Downloader`, internal helpers | `pub(crate) youtube_extractor: Youtube` |
| Private | Implementation details | `fn audio_codec_for_mux(...)` |

> 💡 Builder struct fields are always **private**. `TypedBuilder` config struct fields are always **`pub`**.

---

## ✍️ Code Style

### 🌍 Language

All comments, docs, variable names, error messages, and log messages must be in **English**. No exceptions.

### 📥 Imports

```rust
// ✅ GOOD — All imports at the top of the file
use crate::error::Result;
use crate::model::Video;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ❌ BAD — Never import inside function bodies
fn my_function() {
    use std::collections::HashMap; // WRONG
}
```

> 🧩 **Exception**: inside `macro_rules!` definitions, `$crate::` paths may require local imports.

### 🏷️ Naming conventions

| Item | Convention | Example |
|------|-----------|---------|
| Variables & functions | `snake_case` | `download_video`, `is_ready` |
| Types & structs | `PascalCase` | `DownloaderBuilder`, `VideoQuality` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_RETRY_ATTEMPTS`, `FORMAT_URL_LIFETIME` |
| Constants prefix | Context prefix | `DEFAULT_`, `CONSERVATIVE_`, `BALANCED_`, `AGGRESSIVE_` |
| Booleans | Intent-driven | `is_ready`, `has_data`, `include_full_data` |

### 🔀 Conditional logic

**No more than two raw conditions directly in an `if` (or `while`) guard.** When three or more sub-expressions are combined with `&&` or `||`, each sub-expression must first be bound to a short, descriptively-named `let` boolean before the guard. Boolean variable names must be short and intent-revealing: `is_year`, `is_endlist`, `is_timeout`, etc.

```rust
// ✅ single condition — OK
if probe.len() < 4 { … }

// ✅ two raw conditions combined — OK
if e.starts_with("HTTP 4") && !e.starts_with("HTTP 429") { … }

// ✅ named booleans combined — OK (required when ≥ 3 conditions)
let is_timeout = error.is_timeout();
let is_connect = error.is_connect();
let is_request = error.is_request();
if is_timeout || is_connect || is_request { … }

// ❌ three or more raw expressions inline — NOT OK
if error.is_timeout() || error.is_connect() || error.is_request() { … }
```

### 🚫 Lint suppressions

`#[allow(…)]` attributes are **forbidden** in this codebase, with one explicit exception:

- `#[allow(clippy::large_enum_variant)]` on `DownloadEvent` — boxing all variants for one large variant would add unnecessary indirection throughout the event system.

**Fix the root cause instead of suppressing the lint:**

| Lint | Preferred fix |
|---|---|
| `dead_code` | Remove the item, or gate with `#[cfg(feature = "…")]` |
| `unreachable_code` | Use `unreachable!("…")` or gate the fallback with `#[cfg(not(…))]` |
| `clippy::too_many_arguments` | Group related parameters into a dedicated struct |
| `unused_*` | Remove unused imports/variables, or prefix with `_` for intentional non-use |

### 🪆 Nesting depth

**Maximum two levels of nesting inside any function body.** Each loop (`for`, `while`, `loop`), conditional (`if`, `else if`, `match`), or closure that contains control flow counts as one level. Exceeding two levels raises the [SonarCloud Cognitive Complexity](https://www.sonarsource.com/docs/CognitiveComplexity.pdf) above the enforced threshold of 15 and will block your PR.

When a third level is needed, **extract the inner logic into a private helper function** that returns an `Option`, `Result`, or a dedicated struct.

```rust
// ❌ BAD — three levels of nesting (loop → if → if)
fn scan_tags(probe: &[u8]) {
    while let Some(tag) = next_tag(probe) {           // level 1
        if tag.kind == TagKind::Video {               // level 2
            if tag.frame_type == FrameType::Key {     // level 3 ← NOT allowed
                keyframes.push(tag.offset);
            }
        }
    }
}

// ✅ GOOD — max two levels; the inner predicate is extracted
fn is_video_keyframe(tag: &Tag) -> bool {
    tag.kind == TagKind::Video && tag.frame_type == FrameType::Key
}

fn scan_tags(probe: &[u8]) {
    while let Some(tag) = next_tag(probe) {           // level 1
        if is_video_keyframe(&tag) {                  // level 2
            keyframes.push(tag.offset);
        }
    }
}
```

The same rule applies to `match` arms that contain their own `if`/`loop`/`match`:

```rust
// ❌ BAD — match arm body itself opens a new level
match block_type {
    BlockType::StreamInfo => {
        if block_len >= MIN_SIZE {    // level 3 when already inside a loop + match
            parse_stream_info(block);
        }
    }
}

// ✅ GOOD — delegate to a helper that handles the guard internally
match block_type {
    BlockType::StreamInfo => parse_stream_info(block), // helper does its own guard
}
```

| Rule | Detail |
|------|--------|
| Hard limit | 2 nesting levels per function |
| What counts | `for`, `while`, `loop`, `if`/`else if`/`else`, `match`, closures with control flow |
| Remedy | Extract inner body into a private `fn`, or use early-return / guard-clause patterns |
| SonarCloud | Max Cognitive Complexity per function: **15** |

### 🎯 Parameter types

Use the most appropriate type for public API parameters:

```rust
// ✅ GOOD — Flexible public API
pub fn new(url: impl Into<String>) -> Self { ... }
pub fn with_cookies(mut self, path: impl Into<PathBuf>) -> Self { ... }
pub fn input(mut self, path: impl AsRef<str>) -> Self { ... }

// ❌ BAD — Too restrictive
pub fn new(url: String) -> Self { ... }
pub fn new(url: &str) -> Self { ... }
```

For internal functions, use the most optimized type for the operations applied:
- `&str` if you only read the string
- `String` if you need ownership
- `&Path` if you only read the path
- `PathBuf` if you need ownership

### 🧪 Testing

There are **no `#[cfg(test)]` modules** in `src/`. No tests live in `tests/common/` (only shared helpers).

**Test harnesses** — three separate binaries under `tests/`:

| Harness | Command | Scope |
|---------|---------|-------|
| Unit | `cargo test --test unit --all-features` | Pure logic, no I/O, no network |
| Integration | `cargo test --test integration --all-features` | wiremock servers, tempdir I/O, async flows |
| E2E | `cargo test --test e2e --all-features -- --test-threads=1` | Full download pipeline with wiremock |
| Doctests | `cargo test --doc --workspace` | Code examples in rustdoc |

**Directory conventions** — test directories mirror `src/` module hierarchy:
```
tests/unit/model/       ← matches src/model/
tests/unit/download/    ← matches src/download/
tests/integration/cache/ ← matches src/cache/
```
Create a subdirectory when a domain has ≥ 2 test files.

**Adding a new test:**
1. Create the test file in the appropriate subdirectory (e.g. `tests/unit/download/new_test.rs`)
2. Register it in the harness entry point (`tests/unit.rs`) with `#[path = "unit/download/new_test.rs"] mod new_test;`
3. Feature-gated tests use `#[cfg(feature = "...")]` on the module declaration in the entry point

**Conventions:**
- Test names follow `fn verb_noun_condition()` (e.g. `fn parse_format_returns_video_type()`)
- All test output goes to `tempfile::tempdir()`, never to project root
- Use `assert_matches!` for error variant checks, `pretty_assertions` for struct comparisons
- Mock servers use `wiremock::MockServer` (dev-dependency)
- Fixtures: JSON in `tests/fixtures/json/`, media in `tests/fixtures/media/`
- 📊 **Benchmarks** — `benches/benchmarks.rs` with [criterion](https://crates.io/crates/criterion)
- 🧪 **Integration examples** — `examples/` directory

### 🔢 Magic Numbers & Constants

**Never use raw numeric or byte literals in logic.** Every literal must be extracted to a named `const` at the top of the file.

```rust
// ✅ GOOD — Named constants with clear intent
/// ID3v2 header fixed size in bytes.
const ID3V2_HEADER_SIZE: usize = 10;
/// Maximum bytes to scan for the first sync word.
const SYNC_SEARCH_LIMIT: usize = 8192;

fn skip_id3(data: &[u8]) -> usize {
    if data.len() < ID3V2_HEADER_SIZE { return 0; }
    // ...
}

// ❌ BAD — What does 10 mean? What about 8192?
fn skip_id3(data: &[u8]) -> usize {
    if data.len() < 10 { return 0; }
    // ...
}
```

| Rule | Detail |
|------|--------|
| Location | File top, before any `fn` or `impl` |
| Naming | `SCREAMING_SNAKE_CASE` with context prefix (`DEFAULT_`, `BALANCED_`, etc.) |
| Lookup tables | Bitrate tables, sample rate tables → `const` arrays at file top |
| Magic bytes | `const EBML_MAGIC: &[u8] = &[0x1A, 0x45, 0xDF, 0xA5];` — never raw in conditionals |

### 📦 Return Types (No Tuples)

**Never return tuples from functions.** Use a named struct instead — even for two fields.

```rust
// ✅ GOOD — Clear field semantics at call site
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

fn find_range(&self, time: f64) -> Option<ByteRange> {
    // ...
}

// ❌ BAD — Opaque meaning, easy to swap fields
fn find_range(&self, time: f64) -> Option<(u64, u64)> {
    // ...
}
```

| Rule | Detail |
|------|--------|
| Scope | Module-private structs are fine if only used internally |
| Derives | At minimum `Debug, Clone` — add `Copy, PartialEq, Eq` when applicable |
| Fields | Descriptive names that convey semantics |

### 🔗 Function Call & Type Qualification

Qualify function calls with **at most one `::`** — import deeper paths at the top of the file.

```rust
// ✅ GOOD — Import then use short paths
use reqwest::header::{self, HeaderMap, HeaderValue};

let mut headers = HeaderMap::new();
headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));

// ❌ BAD — Double-qualified paths
let mut headers = reqwest::header::HeaderMap::new();
headers.insert(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static("text/plain"));
```

| Rule | Example |
|------|---------|
| `Self::` for associated fns in `impl` | `Self::new()`, `Self::parse_header(data)` |
| `module::function()` | `detect::probe(data)` |
| `Type::method()` | `String::from("hello")` |
| Import heavily-used types directly | `use std::collections::HashMap;` then `HashMap::new()` |

---

## 🚨 Error Handling

We use a **single unified error type** in `src/error.rs`. Never introduce new error enums (except `HookError` which already exists for hook-specific failures).

### Rules

| Rule | Detail |
|------|--------|
| **One `Error` enum** | All variants in one enum, grouped by `// === Category ===` comment banners |
| **Type alias** | `pub type Result<T> = std::result::Result<T, Error>;` — import as `use crate::error::Result;` |
| **Structured fields** | Every variant uses named fields (`operation`, `url`, `reason`, `path`, `source`) — never just a string |
| **`#[source]`** | Always on the inner error field for proper chaining |
| **Helper constructors** | `Error::io(...)`, `Error::http(...)` — each logs `tracing::warn!`/`tracing::error!` before constructing |
| **`From` impls** | For `std::io::Error`, `reqwest::Error`, `serde_json::Error`, `JoinError`, `ZipError` — each logs with `"(automatic conversion)"` suffix |
| **Parameter style** | `impl Into<String>` — not concrete types |
| **Feature-gated** | `#[cfg(feature = "cache-redb")] Database { ... }`, `#[cfg(feature = "cache-redis")] Redis { ... }` |
| **No `anyhow`** | Always use the crate's own `Error` / `Result` |

### Example: Adding a new error variant

```rust
// In src/error.rs, add to the appropriate category section:

// ==================== Video & Format Errors ====================

/// My new error description.
#[error("Something failed for {video_id}: {reason}")]
MyNewError {
    video_id: String,
    reason: String,
},
```

And add a helper constructor:
```rust
pub fn my_new_error(video_id: impl Into<String>, reason: impl Into<String>) -> Self {
    let video_id = video_id.into();
    let reason = reason.into();

    tracing::warn!(video_id = video_id, reason = reason, "Something failed");

    Self::MyNewError { video_id, reason }
}
```

---

## 🔧 Builder Patterns

Two builder styles coexist — use the right one for the right job:

### A) Manual builder (consuming `mut self`)

Used for: `DownloaderBuilder`, `DownloadBuilder`, `WebhookConfig`, `FfmpegArgs`

```rust
// ✅ Builder methods prefixed with `with_` and consuming `mut self`
pub fn with_timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
}

// ✅ Terminal method
pub async fn build(self) -> Result<Downloader> { ... }
```

| Rule | Detail |
|------|--------|
| Method prefix | `with_` (e.g. `with_args`, `with_timeout`, `with_proxy`, `with_cache`) |
| Self parameter | Always `mut self` (consuming) — **never `&mut self`** |
| Terminal method | `.build()` or `.execute()` |
| Field visibility | Private |

### B) `TypedBuilder` derive

Used for: config structs (`ManagerConfig`, `RetryPolicy`, `ExpiryConfig`)

```rust
#[derive(Debug, Clone, TypedBuilder)]
pub struct ManagerConfig {
    #[builder(default = SpeedProfile::default().max_concurrent_downloads())]
    pub max_concurrent_downloads: usize,
}
```

| Rule | Detail |
|------|--------|
| Field visibility | `pub` |
| Defaults | `#[builder(default = ...)]` |

### C) Post-build mutation on `Downloader`

After `.build()`, use `set_*`/`add_*` methods (not `with_*`) to mutate the `Downloader` instance:

```rust
downloader.set_user_agent("my-agent");
downloader.set_timeout(Duration::from_secs(30));
downloader.set_args(vec!["--no-playlist".into()]);
downloader.add_arg("--flat-playlist");
downloader.set_cookies("cookies.txt");
downloader.set_cookies_from_browser("chrome");
downloader.set_netrc();
```

| Rule | Detail |
|------|--------|
| Self parameter | `&mut self` (borrowing) — returns `&mut Self` for chaining |
| Prefix for replacing | `set_` (e.g. `set_cookies`, `set_user_agent`, `set_timeout`) |
| Prefix for appending | `add_` (e.g. `add_arg`) |

> 💡 **Don't confuse** builder `with_*` methods (consuming `mut self`, used before `.build()`) with post-build `set_*`/`add_*` methods (borrowing `&mut self`, used after `.build()`).

---

## 📦 Model & Data Types

### Standard derive sets

| Type | Derives |
|------|---------|
| **Simple enums** | `Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize` + `Default` with `#[default]` |
| **Complex structs** (with `f64`) | `Debug, Clone, PartialEq, Serialize, Deserialize` — manual `Eq`/`Hash` |
| **Simple structs** (no floats) | `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` |

### Serde patterns

| Pattern | Usage |
|---------|-------|
| `#[serde(flatten)]` | Struct composition (e.g. `Format` flattens `CodecInfo`, `VideoResolution`, etc.) |
| `#[serde(rename = "...")]` | Field name mapping from JSON (`"timestamp"`, `"acodec"`) |
| `#[serde(rename_all = "snake_case")]` | Enum variant renaming |
| `#[serde(default)]` | Optional collections and fields |
| `#[serde(other)]` | `Unknown` variant for forward compatibility |
| `#[serde(skip)]` | Derived/internal fields (e.g. `video_id` on `Format`) |
| `json_none` deserializer | Turns `"none"` strings to `Option::None` (in `model/utils/serde.rs`) |
| `#[serde_as(deserialize_as = "DefaultOnNull")]` | From `serde_with`, for nullable JSON fields |
| Custom `Deserialize` visitor | Polymorphic types (e.g. `DrmStatus` accepts bool or string) |
| `ordered_float::OrderedFloat<f64>` | Only when `f64` needs `Hash`/`Eq` |

### 🖨️ Display format

**Always** use the format `TypeName(key=value, key=value)`:

```rust
impl fmt::Display for Video {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Video(id={}, title={:?}, channel={:?}, formats={})",
            self.id, self.title, self.channel.as_deref().unwrap_or("Unknown"), self.formats.len())
    }
}
```

| Rule | Detail |
|------|--------|
| Only essential fields | Never full serialization |
| `Option` fields | `as_deref().unwrap_or("none")` or `unwrap_or("unknown")` |
| Enum constant variants | `f.write_str("VariantName")` |
| Enum variants with fields | `write!(f, "Variant(key={})", val)` |

### 🔑 Custom `Hash` implementations

Hash **only identity fields** — not all struct fields:

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

---

## 🧬 Trait Design

### Which pattern to use?

| Pattern | When | Example |
|---------|------|---------|
| `#[async_trait]` | Trait used as `dyn Trait` (trait objects) | `VideoExtractor`, `EventHook` |
| RPITIT (`impl Future + Send`) | Dispatched via concrete enum, never `dyn` | Cache backend traits |
| `DynClone + clone_trait_object!` | Need to clone trait objects | `EventHook` |
| `Downcast + impl_downcast!` | Runtime downcasting of trait objects | `VideoExtractor` |

### `#[async_trait]` example

```rust
#[async_trait]
pub trait VideoExtractor: Downcast + Send + Sync + fmt::Debug {
    async fn fetch_video(&self, url: &str) -> Result<Video>;
    fn name(&self) -> ExtractorName;
    fn supports_url(&self, url: &str) -> bool;
}
impl_downcast!(VideoExtractor);
```

### RPITIT example

```rust
pub trait VideoBackend: Send + Sync + std::fmt::Debug {
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Video>>> + Send;
    fn put(&self, url: String, video: Video) -> impl Future<Output = Result<()>> + Send;
}
```

> 📖 Trait method declarations carry **full rustdoc**; implementations may add only a brief clarifying comment.

---

## 🔒 Shared State & Concurrency

### Primitives used

| Primitive | Purpose |
|-----------|---------|
| `Arc<reqwest::Client>` | Shared HTTP client with connection pooling |
| `Arc<Mutex<...>>` | Mutable shared state (download queues, task maps, next_id counter) |
| `Arc<Semaphore>` | Concurrency limit for parallel downloads |
| `Arc<AtomicU64>` / `Arc<AtomicBool>` | Lock-free counters and flags |
| `Arc<RwLock<...>>` | Read-heavy shared state (hook registry, stats, webhooks) |
| `Arc<DownloadEvent>` | Events in broadcast channel (efficient cloning) |
| `Arc<dyn Fn(...) + Send + Sync>` | Callbacks and filter predicates |
| `tokio_util::sync::CancellationToken` | Graceful shutdown |

### ⚠️ Important rules

| Rule | Detail |
|------|--------|
| Async locks | Use `tokio::sync::Mutex` and `tokio::sync::RwLock` |
| Sync locks | `std::sync::Mutex` **only** for progress counters and non-async contexts |
| Lock safety | **Never** hold a `tokio` lock across `.await` points |
| Simple counters | Prefer `Arc<AtomicU64>` over `Arc<Mutex<u64>>` |
| Caches on Downloader | `Option<Arc<VideoCache>>` |

---

## ⚡ Async Programming

| Rule | Detail |
|------|--------|
| Runtime | `tokio` (multi-threaded) |
| Task spawning | `tokio::spawn` for concurrency |
| Multiple tasks | `tokio::select!` for managing cancellations |
| Structured concurrency | Prefer scoped tasks and clean cancellation paths |
| Timeouts | `tokio::time::timeout` with kill on timeout |
| Blocking work | Offload to `tokio::task::spawn_blocking` (used for `serde_json::from_reader`, CPU-intensive parsing) |
| Time operations | `tokio::time::sleep` and `tokio::time::interval` |
| HTTP | `reqwest` with `Arc<Client>` connection pooling |

### Channels

| Channel | Usage |
|---------|-------|
| `tokio::sync::mpsc` | Webhook delivery queue (bounded, backpressure) |
| `tokio::sync::broadcast` | Event broadcasting to multiple subscribers |
| `tokio::sync::oneshot` | One-time task communication |

---

## 🔔 Event System

The event system lives in `src/events/` and follows a three-phase delivery pattern:

### Architecture

| Component | Role |
|-----------|------|
| `EventBus` | Wraps `broadcast::Sender<Arc<DownloadEvent>>` |
| `DownloadEvent` | Large enum — **all variants use named fields** (no tuple variants) |
| `EventFilter` | Predicate-based with `Vec<Arc<dyn Fn(&DownloadEvent) -> bool + Send + Sync>>` |
| `HookRegistry` | `Arc<RwLock<Vec<Box<dyn EventHook>>>>` |
| `simple_hook!` | Macro to create hooks from closures |

### Event emission order (in `Downloader::emit_event()`)

1. 🪝 **Hooks** — with timeout (`#[cfg(feature = "hooks")]`)
2. 📡 **Webhooks** — non-blocking (`#[cfg(feature = "webhooks")]`)
3. 📢 **Broadcast bus** — always

### Adding a new event variant

```rust
// In DownloadEvent — always use named fields:
// ✅ GOOD
MyNewEvent {
    download_id: u64,
    reason: String,
},

// ❌ BAD — No tuple variants
MyNewEvent(u64, String),
```

---

## 🎯 Feature Flags

### Available features

| Feature | Purpose | Dependencies |
|---------|---------|-------------|
| `hooks` | Rust event callbacks | None |
| `webhooks` | HTTP event delivery | None |
| `statistics` | Real-time analytics | None |
| `cache-memory` *(default)* | In-memory Moka cache | `moka` |
| `cache-json` | JSON file backend | None |
| `cache-redb` | Embedded redb backend | `redb` |
| `cache-redis` | Distributed Redis backend | `redis` |
| `live-recording` | Live stream recording (HLS) | `m3u8-rs` |
| `live-streaming` | Live fragment streaming (HLS) | `m3u8-rs` |
| `rustls` | TLS backend | `reqwest/rustls` |
| `hickory-dns` | Async DNS resolver | `reqwest/hickory-dns` |
| `profiling` | Heap profiler | `dhat` |

### ⚙️ `cache` cfg is emitted by `build.rs`

The `cache` cfg is **not** a Cargo feature — it is a custom `cfg` emitted by `build.rs` when any cache backend
(`cache-memory`, `cache-json`, `cache-redb`, or `cache-redis`) is enabled. Users cannot activate it directly,
and it is invisible in `Cargo.toml`. Use `#[cfg(cache)]` to guard code that requires any cache backend.

### Backend selection

`build.rs` emits `persistent_cache` when any of `cache-json`, `cache-redb`, or `cache-redis` is enabled. Multiple persistent features may be active simultaneously — the `multiple_persistent_backends` cfg and its associated `compile_error!` have been removed.

When exactly one persistent feature is compiled in, `CacheConfig::persistent_backend` is auto-deduced and may be left as `None`. When more than one is compiled in, `persistent_backend` **must** be set explicitly to a `PersistentBackendKind` variant; leaving it `None` causes `CacheLayer::from_config` to return `Error::AmbiguousCacheBackend` at runtime.

```rust
use yt_dlp::prelude::*;

// Multiple backends compiled in — pick one at runtime:
let config = CacheConfig::builder()
    .cache_dir("cache")
    .persistent_backend(PersistentBackendKind::Redb) // required when multiple compiled in
    .build();
```

### Conditional compilation patterns

```rust
// Module-level guard for all cache code (cfg emitted by build.rs)
#[cfg(cache)]

// Backend-specific modules
#[cfg(feature = "cache-json")]
pub mod json;

// Persistent backend guard (any of json/redb/redis)
#[cfg(persistent_cache)]

// Feature-gated struct fields
#[cfg(feature = "hooks")]
pub(crate) hook_registry: Option<events::HookRegistry>,
```

### ❌ Forbidden patterns

- **Never use `#[cfg(...)]` on function parameters.** It makes function signatures unreadable and call sites overly complex. If a parameter is feature-dependent, either feature-gate the entire function, or use a config struct / builder pattern where the specific field is feature-gated.

---

## 📝 Tracing & Logging

Tracing is an **unconditional dependency** — every important function must have tracing.

### Rules at a glance

| Rule | Detail |
|------|--------|
| Macro style | Always fully-qualified: `tracing::debug!(...)` — **never import the macros** |
| No `#[instrument]` | Never use the `#[instrument]` attribute |
| Structured fields | `key = value`, `key = ?value` (Debug), `key = %value` (Display) |
| No interpolation | Never `tracing::debug!("msg {}", var)` — always structured fields |

### Log levels

| Level | Usage | Emoji? |
|-------|-------|--------|
| `trace` | Hot paths, data transforms (rare — prefer deleting) | ✅ Yes |
| `debug` | Function entry/exit, parameters, config, internal ops | ✅ Yes |
| `info` | Key milestones (download start/end, fetch, install, shutdown) | ✅ Yes |
| `warn` | Recoverable failures, retries, fallbacks | ❌ No emoji |
| `error` | Unrecoverable per-item failures | ❌ No emoji |

### 🎨 Emoji prefixes

Every `trace`/`debug`/`info` message **must** start with one domain emoji:

| Emoji | Domain |
|-------|--------|
| 📦 | Install / dependencies |
| 📡 | Fetch / extract |
| 📥 | Download |
| 🎬 | Combine / mux |
| ✂️ | Postprocess / ffmpeg |
| 🏷️ | Metadata |
| 💬 | Subtitle |
| 🖼️ | Thumbnail |
| 📋 | Playlist |
| ✅ | Success / completion |
| 🔄 | Retry / update |
| 🔧 | Config / setup / builder |
| 🔍 | Cache / lookup |
| ⚙️ | Internal / utility |
| 📊 | Statistics |
| 🔔 | Events |
| 🧩 | Format selection |
| 🛑 | Shutdown |

### Example

```rust
// ✅ GOOD
tracing::debug!(url = %url, timeout = ?timeout, "📥 Starting download");
tracing::info!(video_id = video_id, formats = formats.len(), "📡 Video fetched");
tracing::warn!(url = %url, attempt = attempt, "Retry after failure");

// ❌ BAD
tracing::debug!("Starting download for {}", url);  // No interpolation
tracing::info!("Video fetched");                     // No structured fields
tracing::warn!("⚠️ Retry");                          // No emoji on warn
```

### What NOT to trace

- ❌ Trivial getters/setters that just return or set a field
- ❌ Pure transforms (`to_ffmpeg_name`, `is_empty`, enum-to-string)
- ❌ Simple constant lookups / match on enum returning a value

---

## 📖 Documentation

Every public function, method, and trait method must have a **rustdoc comment**:

### Template

```rust
/// Brief one-line description.
///
/// Optional extended description.
///
/// # Arguments
///
/// * `param` - Description
///
/// # Errors
///
/// Returns an error if ...
///
/// # Returns
///
/// Description of return value.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::prelude::*;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let downloader = Downloader::builder(libraries, "output").build().await?;
/// # Ok(())
/// # }
/// ```
```

### Section rules

| Section | When to include |
|---------|----------------|
| `# Arguments` | Only if params beyond `&self`/`&mut self` |
| `# Errors` | Only if returns `Result` |
| `# Returns` | Only if returns a value (not `()`) |
| `# Examples` | Main public API entry points (`Downloader::new`, `download`, `fetch`, etc.) |

### Additional rules

| Rule | Detail |
|------|--------|
| Trait methods | Full rustdoc on the **trait declaration**; impls may add only a brief comment |
| Getters | Minimum one-liner + `# Returns` |
| Setters | Minimum one-liner + `# Arguments` |
| Builder methods | Minimum one-liner + `# Arguments` |
| Examples | Use `no_run` or `ignore` for network/binary-dependent code |

---

## ⚙️ Process Execution

The crate runs external processes (`yt-dlp`, `ffmpeg`) through a controlled abstraction:

| Component | Location | Purpose |
|-----------|----------|---------|
| `Executor` | `src/executor/mod.rs` | Wraps `tokio::process::Command` with piped I/O and timeout |
| `ProcessOutput` | `src/executor/process.rs` | `{ stdout, stderr, code }` |
| `FfmpegArgs` | `src/executor/ffmpeg.rs` | Fluent builder: `.input()`, `.codec_copy()`, `.args()`, `.output()`, `.build()` |
| `run_ffmpeg_with_tempfile()` | `src/executor/ffmpeg.rs` | Temp file + rename pattern for atomic writes |

### Key patterns

- ⏱️ **Timeout**: `tokio::time::timeout` + `process.kill()` on timeout
- 🪟 **Windows**: `command.creation_flags(0x08000000)` (CREATE_NO_WINDOW) behind `#[cfg(target_os = "windows")]`
- 🔄 **Temp + rename**: FFmpeg writes to a temp file, then renames atomically — never write directly to the final output
- 🧵 **CPU-heavy parsing**: `tokio::task::spawn_blocking` for `serde_json::from_reader` and other CPU-intensive work

---

## 🧩 Macros

Defined in `src/macros.rs` and `src/events/hooks.rs`:

| Macro | Purpose |
|-------|---------|
| `youtube!($yt_dlp, $ffmpeg, $output)` | Convenience `Downloader` constructor |
| `ytdlp_args![...]` | Args builder (string list or key-value pairs) |
| `install_libraries!($dir)` | Async binary installation |
| `ternary!($cond, $true, $false)` | Ternary operator |
| `simple_hook!` | Create an `EventHook` from a closure |

All macros must use `$crate::` fully-qualified paths for robustness. The `use` inside `macro_rules!` bodies is the **only** exception to the "imports at module top" rule.

---

## 🔍 Contributing to media-seek

`crates/media-seek/` is a standalone crate published independently to [crates.io](https://crates.io/crates/media-seek). Changes to it follow the same code conventions as the main crate, with a few important constraints.

### Constraints

| Rule | Detail |
|------|--------|
| **No feature flags** | All formats are always compiled in — no conditional compilation inside `media-seek` |
| **No `reqwest`** | The crate is transport-agnostic. Callers implement `RangeFetcher`. |
| **No `serde`** | No serialization — pure parsing only |
| **No `async_trait`** | `RangeFetcher` uses RPITIT (`impl Future + Send`), not `#[async_trait]` |
| **No tuples** | `ByteRange { start, end }` instead of `(u64, u64)` |
| **Named constants** | All magic numbers (sync bytes, header sizes, bitrate tables) as `const` at file top |
| **dedup safety** | `dedup_by_key` only after sorting by the **same key**; re-sort after dedup if needed |

### Where to make changes

| Change | Location |
|--------|---------|
| Audio format parser | `crates/media-seek/src/audio/` (`mp3.rs`, `ogg.rs`, `flac.rs`, `pcm.rs`, `adts.rs`) |
| Video format parser | `crates/media-seek/src/video/` (`mp4.rs`, `webm.rs`, `flv.rs`, `avi.rs`, `ts.rs`) |
| Format detection | `crates/media-seek/src/detect.rs` |
| Index data types | `crates/media-seek/src/index.rs` |
| Error handling | `crates/media-seek/src/error.rs` |
| Public API | `crates/media-seek/src/lib.rs` |

### Tracing conventions

Every `pub(crate) fn parse()` / `pub(crate) async fn parse()` must have entry and success tracing:

```rust
// At function start:
tracing::debug!(probe_len = probe.len(), "⚙️ Parsing <Format> stream");

// Just before each successful return:
tracing::debug!(segments = result.len(), "✅ <Format> index parsed");
```

Use `⚙️` for internal operations and `✅` for success — same as the main crate. No emoji on `warn!` or `error!`.

### Checking your changes

```bash
# media-seek standalone lint
cargo clippy -p media-seek -- -D warnings

# Run media-seek unit + integration tests
cargo test --test unit --all-features -- media_seek
cargo test --test integration --all-features -- media_seek

# Doc-tests (both crates)
cargo test --doc --workspace
```

---

## ✅ Verification Checklist

Before submitting your PR, make sure:

- [ ] 🔍 `cargo clippy --workspace --all-features -- -D warnings` — zero warnings
- [ ] 💄 `cargo +nightly fmt --all -- --check` — properly formatted
- [ ] 🧪 `cargo test --test unit --all-features` — all unit tests pass
- [ ] 🧪 `cargo test --test integration --all-features` — all integration tests pass
- [ ] 🧪 `cargo test --test e2e --all-features -- --test-threads=1` — all E2E tests pass
- [ ] 🧪 `cargo test --doc --workspace --all-features` — all doc-tests pass
- [ ] 🔐 `cargo deny check` — no dependency issues
- [ ] 🧹 `cargo machete` — no unused dependencies
- [ ] 📝 All new public items have rustdoc following the template
- [ ] 🎨 All tracing uses structured fields + emoji prefix
- [ ] 🚨 Errors use the existing `Error` enum with structured fields
- [ ] 📥 All `use` imports are at the top of the file
- [ ] 🔢 No magic numbers — all literals extracted to named `const` at file top
- [ ] 📦 No tuple return types — use named structs instead
- [ ] 🔗 No double-qualified paths — import types and use short names
- [ ] 🌍 All text (comments, docs, logs) is in English
- [ ] 🪆 No function exceeds 2 nesting levels — extract deeper logic into private helpers

---

<div align="center">
  <strong>Thank you for contributing! 🎉</strong>
  <br>
  <sub>If you have questions, open a <a href="https://github.com/boul2gom/yt-dlp/discussions">Discussion</a> — we're happy to help.</sub>
</div>
