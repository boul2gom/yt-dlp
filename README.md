<h2 align="center">🎬️ A Rust library (with auto dependencies installation) for video downloading</h2>

<div align="center">This library is a Rust asynchronous wrapper around the yt-dlp command line tool, a feature-rich audio/video downloader supporting <strong>YouTube, Vimeo, TikTok, Instagram, Twitter, and more.</strong></div>
<div align="center">
  The crate is designed to download audio and video from various websites.
  You don't need to care about dependencies, yt-dlp and ffmpeg will be downloaded automatically.
</div>

<br>
<div align="center">⚠️ The project is still in development, so if you encounter any bugs or have any feature requests, please open an issue or a discussion.</div>
<br>

<div align="center">
  <a href="https://github.com/boul2gom/yt-dlp/issues/new/choose">Report a Bug</a>
  ·
  <a href="https://github.com/boul2gom/yt-dlp/discussions/new?category=ideas">Request a Feature</a>
  ·
  <a href="https://github.com/boul2gom/yt-dlp/discussions/new?category=q-a">Ask a Question</a>
</div>

---

<p align="center">
  <a href="https://github.com/boul2gom/yt-dlp/actions/workflows/ci-dev.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/boul2gom/yt-dlp/ci-dev.yml?label=Develop%20CI&logo=Github" alt="Develop CI"/>
  </a>  
  <a href="https://crates.io/crates/yt-dlp">
    <img src="https://img.shields.io/github/v/release/boul2gom/yt-dlp?label=Release&logo=Rust" alt="Release"/>
  </a>
  <a href="https://crates.io/crates/yt-dlp">
    <img src="https://img.shields.io/crates/d/yt-dlp?label=Downloads&logo=Rust" alt="Downloads"/>
  </a>
  <a href="https://deepwiki.com/boul2gom/yt-dlp">
    <img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki">
  </a>
</p>
<p align="center">
  <a href="https://github.com/boul2gom/yt-dlp/discussions">
    <img src="https://img.shields.io/github/discussions/boul2gom/yt-dlp?label=Discussions&logo=Github" alt="Discussions">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/issues">
    <img src="https://img.shields.io/github/issues-raw/boul2gom/yt-dlp?label=Issues&logo=Github" alt="Issues">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/pulls">
    <img src="https://img.shields.io/github/issues-pr-raw/boul2gom/yt-dlp?label=Pull%20requests&logo=Github" alt="Pull%20requests">
  </a>
</p>
<p align="center">
  <a href="https://github.com/boul2gom/yt-dlp/blob/develop/LICENSE.md">
    <img src="https://img.shields.io/github/license/boul2gom/yt-dlp?label=License&logo=Github" alt="License">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/stargazers">
    <img src="https://img.shields.io/github/stars/boul2gom/yt-dlp?label=Stars&logo=Github" alt="Stars">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/fork">
    <img src="https://img.shields.io/github/forks/boul2gom/yt-dlp?label=Forks&logo=Github" alt="Forks">
  </a>
</p>  

<p align="center">
  <img src="https://repobeats.axiom.co/api/embed/81fed25250909bb618c0180c8092c143feae0616.svg" alt="Statistics" title="Repobeats analytics image" />
</p>

<p align="center">
  <a href="https://app.fossa.com/projects/custom%2B60779%2Fgithub.com%2Fboul2gom%2Fyt-dlp?ref=badge_small" alt="FOSSA Status"><img src="https://app.fossa.com/api/projects/custom%2B60779%2Fgithub.com%2Fboul2gom%2Fyt-dlp.svg?type=small"/></a>
  <a href="https://codecov.io/gh/boul2gom/yt-dlp"><img src="https://img.shields.io/codecov/c/github/boul2gom/yt-dlp?token=M9LN6PRPUU&label=Codecov&logo=codecov" alt="Codecov"></a>
  <a href="https://sonarcloud.io/summary/overall?id=boul2gom_yt-dlp"><img src="https://img.shields.io/sonar/quality_gate/boul2gom_yt-dlp?server=https%3A%2F%2Fsonarcloud.io&label=Quality&logo=sonarqubecloud" alt="Quality"></a>
  <a href="https://securityscorecards.dev/viewer/?uri=github.com/boul2gom/yt-dlp"><img src="https://img.shields.io/ossf-scorecard/github.com/boul2gom/yt-dlp?label=Scorecard&logo=securityscorecard" alt="Scorecard"></a>
</p>

---

## 💭️ Why use an external Python app?

Originally, to download videos from YouTube, I used the [```rustube```](https://crates.io/crates/rustube) crate, written in pure Rust and without any external dependencies.
However, I quickly realized that due to frequent breaking changes on the YouTube website, the crate was outdated and no longer functional.

After a few tests and research, I concluded that the python app [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) was the best compromise, thanks to its regular updates and massive community.
Its standalone binaries and its ability to output the fetched data in JSON format make it a perfect candidate for a Rust wrapper.

Using an external program is not ideal, but it is the most reliable and maintained solution for now.

## 📥 How to get it

Add the following to your `Cargo.toml` file:
```toml
[dependencies]
yt-dlp = "2.7.2"
```

A new release is automatically published every two weeks, to keep up to date with dependencies and features.
Make sure to check the [releases](https://github.com/boul2gom/yt-dlp/releases) page to see the latest version of the crate.

## 🔌 Optional features

This library puts a lot of functionality behind optional features in order to optimize
compile time for the most common use cases. The following features are
available.

- 🪝 **`hooks`** - Enables Rust hooks and callbacks for download events. Allows registering async functions that will be called when events occur.
- 📡 **`webhooks`** - Enables HTTP webhooks delivery for download events. Allows sending events to external HTTP endpoints with retry logic.
- 📊 **`statistics`** - Enables real-time statistics and analytics on downloads and fetches. Exposes aggregate counters, averages, success rates, and a bounded history window.
- ⚡ **`cache-memory`** (enabled by default) — In-memory Moka cache (pulls in `moka`). Fast TTL-based eviction; no persistence.
- 🗃️ **`cache-json`** — JSON file-system backend. One `.json` file per entry.
- 🗄️ **`cache-redb`** — Embedded [```redb```](https://github.com/cberner/redb) backend. Single-file, pure-rust, ACID-compliant.
- 🌐 **`cache-redis`** — Distributed [```Redis```](https://redis.io/) backend. Native TTL via `SETEX`.
- 🔴 **`live-recording`** - Enables live stream recording via HLS segment fetching (reqwest) or FFmpeg fallback. Pulls in `m3u8-rs` for HLS manifest parsing.
- 📡 **`live-streaming`** - Enables live fragment streaming via HLS segment fetching (reqwest). Pulls in `m3u8-rs` for HLS manifest parsing.
- 🔒 **`rustls`** - Enables the `rustls-tls` feature in the [```reqwest```](https://crates.io/crates/reqwest) crate.
  This enables building the application without openssl or other system sourced SSL libraries.
- 🌍 **`hickory-dns`** - Enables async DNS resolution via [```Hickory DNS```](https://github.com/hickory-dns/hickory-dns) (passes `reqwest/hickory-dns`). Replaces the default blocking system resolver with a fully async, pure-Rust resolver.

### 🗄️ Cache backends

The library includes a tiered metadata cache that avoids redundant yt-dlp subprocess calls for
video info, downloaded files, and playlists. The architecture uses an optional L1 in-memory layer
(Moka) and an optional L2 persistent layer, selected exclusively via Cargo features:

| Feature | Backend | Persistence | Notes |
|---|---|---|---|
| `cache-memory` *(default)* | In-memory Moka | ❌ No | TTL-based eviction, async-ready |
| `cache-json` | JSON files on disk | ✅ Yes | One `.json` file per entry in the cache directory |
| `cache-redb` | Embedded redb | ✅ Yes | Single-file, pure-Rust, ACID transactions |
| `cache-redis` | Redis | ✅ Yes | Distributed, native TTL via `SETEX` |

Multiple persistent backends can be compiled in simultaneously. When exactly one is enabled, it is
selected automatically. When several are enabled, `CacheConfig::persistent_backend` must be set
explicitly; otherwise `CacheLayer::from_config` returns an `Error::AmbiguousCacheBackend` at runtime.
The `cache-memory` feature (Moka L1) can always be combined with any persistent backend for a
tiered L1 + L2 setup.

**Default (in-memory Moka)** — no persistence, TTL-based eviction, useful for short-lived processes:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-memory"] }
```

**JSON** — persistent, file-system backed, no extra dependencies:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-json"] }
```

**Redb** — embedded, single-file, ACID-compliant, great for desktop/server apps:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-redb"] }
```

**Redis** — distributed, ideal for multi-node or cloud deployments:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-redis"] }
```

**Tiered (Moka L1 + persistent L2)** — best of both worlds:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-memory", "cache-redb"] }
```

**Multiple backends compiled in** — select one at runtime via `CacheConfig::persistent_backend`:
```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["cache-memory", "cache-json", "cache-redb"] }
```
```rust,ignore
use yt_dlp::prelude::*;

let config = CacheConfig::builder()
    .cache_dir("cache")
    .persistent_backend(PersistentBackendKind::Redb) // required when multiple compiled in
    .build();
```

#### CDN URL expiry and cache invalidation

When a `Video` is cached, its stream format URLs are valid for approximately **6 hours** (YouTube CDN lifetime). The library tracks this automatically via the `available_at` field on each `Format`.

On every `fetch_video_infos` call, the cache checks whether the format URLs are still fresh. If they have expired, the cached entry is **silently invalidated** and the video is re-fetched — so you never end up downloading with stale CDN URLs. The configured TTL (default 24 h) acts as an upper bound; the effective TTL is `min(configured_ttl, cdn_url_lifetime)`.

This behavior is transparent and requires no changes to your code. You can inspect the expiry yourself:
```rust,ignore
if !video.are_format_urls_fresh() {
    // URLs are stale — fetch_video_infos will re-fetch automatically
}
// Or get the earliest available_at timestamp across all downloadable formats:
if let Some(ts) = video.formats_available_at() {
    println!("Format URLs valid until approx. {} (unix)", ts + yt_dlp::model::FORMAT_URL_LIFETIME);
}
```

### 🔍 Observability & Tracing

This crate always includes the <img align="center" width="20" alt="Tracing" src="https://raw.githubusercontent.com/tokio-rs/tracing/refs/heads/master/assets/logo.svg" /> [`tracing`](https://crates.io/crates/tracing) crate. The library emits `debug` and `trace` span events throughout its internal operations (downloads, cache lookups, subprocess execution, etc.).

⚠️ **Important:** `tracing` macros are **pure no-ops** without a configured subscriber. If you don't add one, there is zero runtime overhead.

To capture logs, add a subscriber in your application:
```toml
[dependencies]
tracing-subscriber = "0.3"
```
```rust,ignore
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::TRACE)
        // completes the builder.
        .finish();
tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
```

Refer to the [`tracing-subscriber` documentation](https://docs.rs/tracing-subscriber) for more advanced configuration (JSON output, log levels, targets, etc.).

---

## 📖 Documentation

The documentation is available on [docs.rs](https://docs.rs/yt-dlp).

## 🏗️ Multi-Extractor Architecture

This library now supports downloading from **1,800+ websites** through a flexible extractor system:

- **`Downloader`** - Universal client supporting all sites via the extractors
- **`extractor::Youtube`** - Highly optimized YouTube extractor with platform-specific features:
  - Player client selection (Android, iOS, Web, TvEmbedded) for bypassing restrictions
  - Format presets (Best, Premium, High, Medium, Low, AudioOnly, ModernCodecs)
  - YouTube-specific methods: `search()`, `fetch_channel()`, `fetch_user()`, `fetch_playlist_paginated()`
- **`extractor::Generic`** - Universal extractor for all other sites with authentication support

### 🧩 Usage Patterns

- 🎬️ For YouTube with optimizations:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg")
    );
    let downloader = Downloader::builder(libraries, "output")
        .build()
        .await?;

    // Access YouTube-specific features
    let youtube = downloader.youtube_extractor();
    let results = youtube.search("rust programming", 10).await?;
    let channel = youtube.fetch_channel("UCaYhcUwRBNscFNUKTjgPFiA").await?;

    Ok(())
}
```

- 🌐 For any website (YouTube, Vimeo, TikTok, etc.):
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"), 
        PathBuf::from("libs/ffmpeg")
    );
    let downloader = Downloader::builder(libraries, "output")
        .build()
        .await?;

    // Works with any supported site
    let video = downloader.fetch_video_infos("https://vimeo.com/123456789").await?;
    let video_path = downloader.download_video(
        &video,
        "output.mp4"
    ).await?;
    Ok(())
}
```

## 📚 Examples

- 📦 Installing the [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) and [```ffmpeg```](https://ffmpeg.org/) binaries:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executables_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    // Create fetcher and install binaries
    let downloader = Downloader::with_new_binaries(
        executables_dir,
        output_dir
    ).await?.build().await?;
    
    Ok(())
}
```

- 📦 Installing the [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) binary only:
```rust,no_run
use yt_dlp::client::deps::LibraryInstaller;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = PathBuf::from("libs");
    let installer = LibraryInstaller::new(destination);

    let youtube = installer.install_youtube(None).await.unwrap();
    Ok(())
}
```

- 📦 Installing the [```ffmpeg```](https://ffmpeg.org/) binary only:
```rust,no_run
use yt_dlp::client::deps::LibraryInstaller;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = PathBuf::from("libs");
    let installer = LibraryInstaller::new(destination);
    
    let ffmpeg = installer.install_ffmpeg(None).await.unwrap();
    Ok(())
}
```

- 🔄 Updating the [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) binary:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    downloader.update_downloader().await?;
    Ok(())
}
```

- 📥 Fetching a video (with its audio) and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    let video_path = downloader.download_video(&video, "my-video.mp4").await?;
    Ok(())
}
```

- 📁 Downloading a video to a specific path (ignoring `output_dir`):
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    
    // Download to an absolute path — the file is written directly to the given path,
    // bypassing the configured output_dir.
    let path = PathBuf::from("/Users/me/Videos/my-video.mp4");
    let video_path = downloader.download_video_to_path(&video, path).await?;
    Ok(())
}
```

- ✨ Using the fluent API with custom quality preferences:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::model::selector::{VideoQuality, AudioQuality, VideoCodecPreference};
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Use the fluent download builder API
    let video_path = downloader.download(&video, "my-video.mp4")
        .video_quality(VideoQuality::CustomHeight(1080))
        .video_codec(VideoCodecPreference::AVC1)
        .audio_quality(AudioQuality::Best)
        .execute()
        .await?;

    Ok(())
}
```

- 🎬 Fetching a video (without its audio) and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    downloader.download_video_stream(&video, "video.mp4").await?;
    Ok(())
}
```

- 🎵 Fetching an audio and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    downloader.download_audio_stream(&video, "audio.mp3").await?;
    Ok(())
}
```

- 📜 Fetching a specific format and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;
use yt_dlp::VideoSelection;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    println!("Video title: {}", video.title);

    let video_format = video.best_video_format().unwrap();
    let format_path = downloader.download_format(&video_format, "my-video-stream.mp4").await?;
    
    let audio_format = video.worst_audio_format().unwrap();
    let audio_path = downloader.download_format(&audio_format, "my-audio-stream.mp3").await?;
    
    Ok(())
}
```

- ⚙️ Combining an audio and a video file into a single file:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;
use yt_dlp::VideoSelection;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    let audio_format = video.best_audio_format().unwrap();
    let audio_path = downloader.download_format(&audio_format, "audio-stream.mp3").await?;

    let video_format = video.worst_video_format().unwrap();
    let video_path = downloader.download_format(&video_format, "video-stream.mp4").await?;

    let output_path = downloader.combine_audio_and_video("audio-stream.mp3", "video-stream.mp4", "my-output.mp4").await?;
    Ok(())
}
```

- 📸 Fetching a thumbnail and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;
use yt_dlp::model::selector::ThumbnailQuality;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    let thumbnail_path = downloader.download_thumbnail(&video, ThumbnailQuality::Best, "thumbnail.jpg").await?;
    Ok(())
}
```

- 🖼️ Selecting a thumbnail by minimum resolution and downloading it:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Best thumbnail by area (width × height)
    if let Some(thumb) = video.best_thumbnail() {
        println!("Best thumbnail: {} — {:?}", thumb.url, thumb.resolution);
    }

    // Smallest thumbnail that is at least 1280×720
    if let Some(thumb) = video.thumbnail_for_size(1280, 720) {
        println!("HD thumbnail: {}", thumb.url);
    }

    Ok(())
}
```

- 📝 Downloading subtitles or automatic captions:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Check available languages (merges subtitles + automatic captions)
    let langs = downloader.list_subtitle_languages(&video);
    println!("Available languages: {:?}", langs);

    // Download French subtitles (falls back to automatic captions if no manual ones)
    let sub_path = downloader.download_subtitle(&video, "fr", "subtitles.srt", true).await?;

    // Download all available subtitles/captions
    let paths = downloader.download_all_subtitles(&video, "subtitles/", true).await?;

    Ok(())
}
```

- 🎞️ Downloading storyboard preview frames:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::model::StoryboardQuality;
use yt_dlp::VideoSelection;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Download the best (highest resolution) storyboard into a directory
    let frames = downloader
        .download_storyboard(&video, StoryboardQuality::Best, "storyboard/")
        .await?;
    println!("Downloaded {} MHTML fragment(s)", frames.len());

    // Or pick a specific storyboard format directly
    if let Some(format) = video.best_storyboard_format() {
        let frames = downloader.download_storyboard_format(format, "storyboard/").await?;
    }

    Ok(())
}
```

- 📥 Download with download manager and priority:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::download::manager::{ManagerConfig, DownloadPriority};
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Custom download manager configuration using the typed builder
    let config = ManagerConfig::builder()
        .max_concurrent_downloads(5)        // Maximum 5 concurrent downloads
        .segment_size(1024 * 1024 * 10)    // 10 MB per segment
        .parallel_segments(8)               // 8 parallel segments per download
        .retry_attempts(5)                  // 5 retry attempts on failure
        .max_buffer_size(1024 * 1024 * 20) // 20 MB maximum buffer
        .build();

    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);

    // Create a fetcher with custom configuration
    let downloader = Downloader::with_download_manager_config(libraries, output_dir, config)
        .build()
        .await?;

    // Download a video with high priority
    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    let download_id = downloader.download_video_with_priority(
        &video,
        "video-high-priority.mp4",
        Some(DownloadPriority::High)
    ).await?;

    // Wait for download completion
    let status = downloader.wait_for_download(download_id).await;
    println!("Final download status: {:?}", status);

    Ok(())
}
```

- 📊 Download with progress tracking:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Download with progress callback
    let download_id = downloader.download_video_with_progress(
        &video, 
        "video-with-progress.mp4", 
        |downloaded, total| {
            let percentage = if total > 0 {
                (downloaded as f64 / total as f64 * 100.0) as u64
            } else {
                0
            };
            println!("Progress: {}/{} bytes ({}%)", downloaded, total, percentage);
        }
    ).await?;

    // Wait for download completion
    downloader.wait_for_download(download_id).await;
    
    Ok(())
}
```

- 🛑 Canceling a download:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Start a download
    let download_id = downloader.download_video_with_priority(
        &video, 
        "video-to-cancel.mp4", 
        None
    ).await?;

    // Check status
    let status = downloader.get_download_status(download_id).await;
    println!("Download status: {:?}", status);

    // Cancel the download
    let canceled = downloader.cancel_download(download_id).await;
    println!("Download canceled: {}", canceled);
    
    Ok(())
}
```

---

## 🎛️ Format Selection

The library provides a powerful format selection system that allows you to download videos and audio with specific quality and codec preferences.

### 🎬 Video Quality Options

- `VideoQuality::Best` - Selects the highest quality video format available
- `VideoQuality::High` - Targets 1080p resolution
- `VideoQuality::Medium` - Targets 720p resolution
- `VideoQuality::Low` - Targets 480p resolution
- `VideoQuality::Worst` - Selects the lowest quality video format available
- `VideoQuality::CustomHeight(u32)` - Targets a specific height (e.g., `CustomHeight(1440)` for 1440p)
- `VideoQuality::CustomWidth(u32)` - Targets a specific width (e.g., `CustomWidth(1920)` for 1920px width)

### 🎵 Audio Quality Options

- `AudioQuality::Best` - Selects the highest quality audio format available
- `AudioQuality::High` - Targets 192kbps bitrate
- `AudioQuality::Medium` - Targets 128kbps bitrate
- `AudioQuality::Low` - Targets 96kbps bitrate
- `AudioQuality::Worst` - Selects the lowest quality audio format available
- `AudioQuality::CustomBitrate(u32)` - Targets a specific bitrate in kbps (e.g., `CustomBitrate(256)` for 256kbps)

### 🎞️ Codec Preferences

#### 📹 Video Codecs
- `VideoCodecPreference::VP9` - Prefer VP9 codec
- `VideoCodecPreference::AVC1` - Prefer AVC1/H.264 codec
- `VideoCodecPreference::AV1` - Prefer AV01/AV1 codec
- `VideoCodecPreference::Custom(String)` - Prefer a custom codec
- `VideoCodecPreference::Any` - No codec preference

#### 🔊 Audio Codecs
- `AudioCodecPreference::Opus` - Prefer Opus codec
- `AudioCodecPreference::AAC` - Prefer AAC codec
- `AudioCodecPreference::MP3` - Prefer MP3 codec
- `AudioCodecPreference::Custom(String)` - Prefer a custom codec
- `AudioCodecPreference::Any` - No codec preference

### 🧪 Example: Downloading with Quality and Codec Preferences

```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::model::selector::{VideoQuality, VideoCodecPreference, AudioQuality, AudioCodecPreference};
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Download a high quality video with VP9 codec and high quality audio with Opus codec
    let video_path = downloader.download_video_with_quality(
        &video,
        "complete-video.mp4",
        VideoQuality::High,
        VideoCodecPreference::VP9,
        AudioQuality::High,
        AudioCodecPreference::Opus
    ).await?;
    
    // Download just the video stream with medium quality and AVC1 codec
    let video_stream_path = downloader.download_video_stream_with_quality(
        &video,
        "video-only.mp4",
        VideoQuality::Medium,
        VideoCodecPreference::AVC1
    ).await?;
    
    // Download just the audio stream with high quality and AAC codec
    let audio_stream_path = downloader.download_audio_stream_with_quality(
        &video,
        "audio-only.m4a",
        AudioQuality::High,
        AudioCodecPreference::AAC
    ).await?;
    
    println!("Downloaded files:");
    println!("Complete video: {}", video_path.display());
    println!("Video stream: {}", video_stream_path.display());
    println!("Audio stream: {}", audio_stream_path.display());
    
    Ok(())
}
```

---

## 📋 Metadata
The project supports automatic addition of metadata to downloaded files in several formats:

- **MP3**: Title, artist, comment, genre (from tags), release year
- **M4A**: Title, artist, comment, genre (from tags), release year
- **MP4**: All basic metadata, plus technical information (resolution, FPS, video codec, video bitrate, audio codec, audio bitrate, audio channels, sample rate)
- **WebM**: All basic metadata (via Matroska format), plus technical information as with MP4
- **FLAC**: Title, artist, album, genre, date, description (via Vorbis comments through lofty), thumbnail embedding
- **OGG/Opus**: Title, artist, album, genre, date, description (via Vorbis comments through lofty)
- **WAV**: Title, artist, album, genre (via RIFF INFO through lofty)
- **AAC**: Title, artist, album, genre, date (via ID3v2 through lofty)
- **AIFF**: Title, artist, album, genre, date (via ID3v2 through lofty)
- **AVI/TS/FLV**: Basic metadata via FFmpeg fallback

Metadata is added automatically during download, without requiring any additional action from the user.

### 🧠 Intelligent Metadata Management
The system intelligently manages the application of metadata based on the file type and intended use:

- For standalone files (audio or audio+video), metadata is applied immediately during download.
- For separate audio and video streams that will be combined later, metadata is not applied to individual files to avoid redundant work.
- When combining audio and video streams with `combine_audio_and_video()`, complete metadata is applied to the final file, including information from both streams.

This optimized approach ensures that metadata is always present in the final file, while avoiding unnecessary processing of temporary files.

## 📖 Chapters

Videos may contain chapters that divide the content into logical segments. The library provides easy access to chapter information and **automatically embeds chapters into downloaded video files** (MP4/MKV/WebM):

- 📖 Accessing video chapters:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Check if video has chapters
    if video.has_chapters() {
        println!("Video has {} chapters", video.get_chapters().len());

        // Iterate over all chapters
        for chapter in video.get_chapters() {
            println!(
                "Chapter: {} ({:.2}s - {:.2}s)",
                chapter.title.as_deref().unwrap_or("Untitled"),
                chapter.start_time,
                chapter.end_time
            );
        }
    }

    Ok(())
}
```

- 🕒 Finding a chapter at a specific timestamp:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Find chapter at 120 seconds (2 minutes)
    if let Some(chapter) = video.get_chapter_at_time(120.0) {
        println!(
            "At 2:00, you're in chapter: {}",
            chapter.title.as_deref().unwrap_or("Untitled")
        );
        println!("Chapter duration: {:.2}s", chapter.duration());
    }

    Ok(())
}
```

**Note**: When downloading videos using `download_video()` or `download_video_from_url()`, chapters are **automatically embedded** into the video file metadata. Media players like VLC, MPV, and others will be able to navigate using the chapters!

## 🔥 Heatmap

Heatmap data (also known as "Most Replayed" segments) shows viewer engagement across different parts of a video. This feature allows you to identify which segments are most popular:

- 🔥 Accessing heatmap data:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Check if video has heatmap data
    if video.has_heatmap() {
        if let Some(heatmap) = video.get_heatmap() {
            println!("Video has {} heatmap segments", heatmap.points().len());

            // Find the most replayed segment
            if let Some(most_replayed) = heatmap.most_engaged_segment() {
                println!(
                    "Most replayed segment: {:.2}s - {:.2}s (engagement: {:.2})",
                    most_replayed.start_time,
                    most_replayed.end_time,
                    most_replayed.value
                );
            }
        }
    }

    Ok(())
}
```

- 📊 Analyzing engagement by threshold:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    if let Some(heatmap) = video.get_heatmap() {
        // Get segments with high engagement (> 0.7)
        let highly_engaged = heatmap.get_highly_engaged_segments(0.7);
        println!("Found {} highly engaged segments", highly_engaged.len());

        for segment in highly_engaged {
            println!(
                "High engagement: {:.2}s - {:.2}s (value: {:.2})",
                segment.start_time,
                segment.end_time,
                segment.value
            );
        }

        // Get engagement at specific timestamp
        if let Some(point) = heatmap.get_point_at_time(120.0) {
            println!(
                "Engagement at 2:00 is {:.2}",
                point.value
            );
        }
    }

    Ok(())
}
```

## 📝 Subtitles

The library provides comprehensive subtitle support, including downloading, language selection, and embedding subtitles into videos:

- 📋 Listing available subtitle languages:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // List all available subtitle languages
    let languages = downloader.list_subtitle_languages(&video);
    println!("Available subtitle languages: {:?}", languages);

    // Check if specific language is available
    if downloader.has_subtitle_language(&video, "en") {
        println!("English subtitles are available");
    }

    Ok(())
}
```

- 📥 Downloading a specific subtitle:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Download English subtitles
    let subtitle_path = downloader
        .download_subtitle(&video, "en", "subtitle_en.srt", true)
        .await?;
    println!("Subtitle downloaded to: {:?}", subtitle_path);

    Ok(())
}
```

- 📥 Downloading all available subtitles:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir.clone())
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Download all available subtitles
    let subtitle_paths = downloader
        .download_all_subtitles(&video, &output_dir, true)
        .await?;
    println!("Downloaded {} subtitle files", subtitle_paths.len());

    Ok(())
}
```

- 🎬 Embedding subtitles into a video:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Download video
    let video_path = downloader.download_video(&video, "video.mp4").await?;

    // Download subtitles
    let en_subtitle = downloader
        .download_subtitle(&video, "en", "subtitle_en.srt", true)
        .await?;
    let fr_subtitle = downloader
        .download_subtitle(&video, "fr", "subtitle_fr.srt", true)
        .await?;

    // Embed subtitles into video
    let video_with_subs = downloader
        .embed_subtitles_in_video(
            &video_path,
            &[en_subtitle, fr_subtitle],
            "video_with_subtitles.mp4",
        )
        .await?;
    println!("Video with embedded subtitles: {:?}", video_with_subs);

    Ok(())
}
```

- 🔄 Working with automatic captions:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::model::caption::Subtitle;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // Iterate over subtitles and filter automatic ones
    for (lang_code, subtitles) in &video.subtitles {
        for subtitle in subtitles {
            if subtitle.is_automatic {
                println!(
                    "Auto-generated subtitle: {} ({})",
                    subtitle.language_name
                        .as_deref()
                        .unwrap_or(lang_code),
                    subtitle.file_extension()
                );
            }
        }
    }

    // Convert automatic captions to Subtitle struct
    for (lang_code, auto_captions) in &video.automatic_captions {
        if let Some(caption) = auto_captions.first() {
            let subtitle = Subtitle::from_automatic_caption(
                caption,
                lang_code.clone(),
            );
            println!("Converted: {}", subtitle);
        }
    }

    Ok(())
}
```

---

## 📂 Playlists

The library provides full playlist support, including fetching playlist metadata and downloading videos with various selection options:

- 📋 Fetching playlist information:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = downloader.fetch_playlist_infos(playlist_url).await?;

    println!("Playlist: {}", playlist.title);
    println!("Videos: {}", playlist.entry_count());
    println!("Uploader: {}", playlist.uploader.as_deref().unwrap_or("unknown"));

    // List all videos in the playlist
    for entry in &playlist.entries {
        println!(
            "[{}] {} ({})",
            entry.index.unwrap_or(0),
            entry.title,
            entry.id
        );
    }

    Ok(())
}
```

- 📥 Downloading entire playlist:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = downloader.fetch_playlist_infos(playlist_url).await?;

    // Download all videos with a pattern
    // Use %(playlist_index)s for index, %(title)s for title, %(id)s for video ID
    let video_paths = downloader
        .download_playlist(&playlist, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} videos", video_paths.len());

    Ok(())
}
```

- 🎯 Downloading specific videos by index:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = downloader.fetch_playlist_infos(playlist_url).await?;

    // Download specific videos by index (0-based)
    let indices = vec![0, 2, 5, 10]; // Videos at positions 1, 3, 6, and 11
    let video_paths = downloader
        .download_playlist_items(&playlist, &indices, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} specific videos", video_paths.len());

    Ok(())
}
```

- 📊 Downloading a range of videos:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = downloader.fetch_playlist_infos(playlist_url).await?;

    // Download videos 5-15 (0-based, inclusive)
    let video_paths = downloader
        .download_playlist_range(&playlist, 5, 15, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} videos from range", video_paths.len());

    Ok(())
}
```

- 🔍 Filtering and analyzing playlists:
```rust,no_run
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let downloader = Downloader::builder(libraries, output_dir)
        .build()
        .await?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = downloader.fetch_playlist_infos(playlist_url).await?;

    // Check if playlist is complete
    if playlist.is_complete() {
        println!("All playlist videos have been fetched");
    }

    // Get only available videos
    let available = playlist.available_entries();
    println!("Available videos: {}/{}", available.len(), playlist.entry_count());

    // Get specific entry
    if let Some(first_video) = playlist.get_entry_by_index(0) {
        println!("First video: {}", first_video.title);
        if let Some(duration) = first_video.duration_minutes() {
            println!("Duration: {:.2} minutes", duration);
        }
    }

    // Get entries in a range
    let range = playlist.get_entries_in_range(0, 10);
    println!("First 11 videos: {}", range.len());

    Ok(())
}
```

---

## 🔔 Events, Hooks & Webhooks

The library provides a comprehensive event system to monitor download lifecycle and react to events through Rust hooks or HTTP webhooks.

### ⚡ Event System

All download operations emit events that you can subscribe to:

- 📡 Subscribing to the event stream:
```rust,no_run
use yt_dlp::Downloader;
use tokio_stream::StreamExt;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    let downloader = Downloader::builder(libraries, output_dir).build().await?;
    let mut stream = downloader.event_stream();

    while let Some(Ok(event)) = stream.next().await {
        println!("Event: {} - {:?}", event.event_type(), event);
    }

    Ok(())
}
```

### ✉️ Available Events

The library emits **22 different event types** covering the entire download lifecycle:

**Download Lifecycle:**
- `VideoFetched` - Video metadata retrieved
- `DownloadQueued` - Download added to queue
- `DownloadStarted` - Download begins
- `DownloadProgress` - Progress updates (bytes downloaded, speed, ETA)
- `DownloadPaused` / `DownloadResumed` - Pause/resume events
- `DownloadCompleted` - Download finished successfully
- `DownloadFailed` - Download failed with error
- `DownloadCanceled` - Download was canceled

**Format & Metadata:**
- `FormatSelected` - Video/audio format chosen
- `MetadataApplied` - Metadata tags written
- `ChaptersEmbedded` - Chapters added to file

**Post-Processing:**
- `PostProcessStarted` / `PostProcessCompleted` / `PostProcessFailed` - FFmpeg operations

**Playlist Operations:**
- `PlaylistFetched` - Playlist metadata retrieved
- `PlaylistItemStarted` / `PlaylistItemCompleted` / `PlaylistItemFailed` - Per-item events
- `PlaylistCompleted` - Entire playlist finished

**Advanced:**
- `SegmentStarted` / `SegmentCompleted` - Parallel segment downloads

### 🪝 Rust Hooks (Feature: `hooks`)

Register async functions to be called when events occur:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["hooks"] }
```

- 🎣 Registering a hook for download events:
```rust,ignore
use yt_dlp::events::{EventHook, EventFilter, DownloadEvent, HookResult};
use async_trait::async_trait;
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[derive(Clone)]
struct MyHook;

#[async_trait]
impl EventHook for MyHook {
    async fn on_event(&self, event: &DownloadEvent) -> HookResult {
        match event {
            DownloadEvent::DownloadCompleted { download_id, output_path, .. } => {
                println!("Download {} completed: {:?}", download_id, output_path);
            }
            DownloadEvent::DownloadFailed { download_id, error, .. } => {
                eprintln!("Download {} failed: {}", download_id, error);
            }
            _ => {}
        }
        Ok(())
    }

    fn filter(&self) -> EventFilter {
        // Only receive terminal events (completed, failed, canceled)
        EventFilter::only_terminal()
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    downloader.register_hook(MyHook).await;

    Ok(())
}
```

**Hook Features:**
- Async execution
- Event filtering (by type, download ID, custom predicates)
- Parallel or sequential execution
- Automatic timeout protection (30s)
- Error isolation (hook failures don't stop downloads)

#### 🔍 Event Filters

```rust,no_run
use yt_dlp::events::EventFilter;

// Only completed downloads
EventFilter::only_completed();

// Only failed downloads
EventFilter::only_failed();

// Progress updates only
EventFilter::only_progress();

// Exclude progress events
EventFilter::all().exclude_progress();

// Specific download ID
EventFilter::download_id(123);

// Terminal events (completed, failed, canceled)
EventFilter::only_terminal();

// Chain filters
EventFilter::download_id(123).and_then(|e| e.is_terminal());

// Custom filter
EventFilter::all().and_then(|event| {
    // Your custom logic
    true
});
```

### 📡 HTTP Webhooks (Feature: `webhooks`)

Send events to external HTTP endpoints with automatic retry:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["webhooks"] }
```

- 📡 Registering a webhook:
```rust,ignore
use yt_dlp::events::{WebhookConfig, WebhookMethod, EventFilter};
use std::time::Duration;
use yt_dlp::Downloader;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    let webhook = WebhookConfig::new("https://example.com/webhook")
        .with_method(WebhookMethod::Post)
        .with_header("Authorization", "Bearer your-token")
        .with_filter(EventFilter::only_completed())
        .with_timeout(Duration::from_secs(10));

    let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    downloader.register_webhook(webhook).await;

    Ok(())
}
```

**Webhook Features:**
- HTTP POST/PUT/PATCH methods
- Custom headers (authentication, etc.)
- Event filtering (same as hooks)
- Automatic retry with exponential backoff (3 attempts by default)
- Configurable timeouts
- JSON payload with event data
- Environment variable configuration

**Environment Variables:**
```bash
export YTDLP_WEBHOOK_URL="https://example.com/webhook"
export YTDLP_WEBHOOK_METHOD="POST"  # Optional, default: POST
export YTDLP_WEBHOOK_TIMEOUT="10"   # Optional, default: 10 seconds
```

- 🔧 Loading a webhook from environment variables:
```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use yt_dlp::events::WebhookConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg")
    );
    let mut downloader = Downloader::builder(libraries, PathBuf::from("output")).build().await?;

    // Load webhook from environment variables
    if let Some(webhook) = WebhookConfig::from_env() {
        downloader.register_webhook(webhook).await;
    }

    Ok(())
}
```

**Webhook Payload:**
```json
{
  "event_type": "download_completed",
  "download_id": 123,
  "timestamp": "2025-01-21T10:30:00Z",
  "data": {
    "download_id": 123,
    "output_path": "/path/to/video.mp4",
    "duration": 45.2,
    "total_bytes": 104857600
  }
}
```

#### ♻️ Retry Strategy

- ♻️ Configuring a retry strategy:
```rust,ignore
use yt_dlp::events::RetryStrategy;
use std::time::Duration;

// Exponential backoff (default)
let strategy = RetryStrategy::exponential(
    3,                              // max attempts
    Duration::from_secs(1),         // initial delay
    Duration::from_secs(30)         // max delay
);

// Linear backoff
let strategy = RetryStrategy::linear(
    3,                              // max attempts
    Duration::from_secs(5)          // fixed delay
);

// No retries
let strategy = RetryStrategy::none();
```

### 🔗 Combining Hooks and Webhooks

Use both hooks and webhooks together:

- 🔗 Using hooks and webhooks simultaneously:
```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::events::{EventHook, WebhookConfig, EventFilter};
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[derive(Clone)]
struct MyLocalHook;

#[async_trait::async_trait]
impl EventHook for MyLocalHook {
    async fn on_event(&self, _event: &yt_dlp::events::DownloadEvent) -> yt_dlp::events::HookResult { Ok(()) }
    fn filter(&self) -> EventFilter { EventFilter::all() }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));

    let mut downloader = Downloader::builder(libraries, PathBuf::from("output")).build().await?;

    // Register Rust hook for immediate in-process handling
    downloader.register_hook(MyLocalHook).await;

    // Register webhook for external notifications
    let webhook = WebhookConfig::new("https://example.com/webhook")
        .with_filter(EventFilter::only_completed());
    downloader.register_webhook(webhook).await;

    // Start downloads - both hooks and webhooks will receive events
    let video = downloader.fetch_video_infos("https://youtube.com/watch?v=...".to_string()).await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

## 📊 Statistics & Analytics (Feature: `statistics`)

Enable real-time, aggregate metrics with zero manual bookkeeping:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["statistics"] }
```

The [`StatisticsTracker`](https://docs.rs/yt-dlp/latest/yt_dlp/stats/struct.StatisticsTracker.html) subscribes to the internal event bus in a background task and continuously updates running counters. Call `snapshot()` at any time to obtain an atomic view of all metrics:

```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    let downloader = Downloader::builder(libraries, "output").build().await?;

    // Perform some downloads and fetches ...
    let video = downloader.fetch_video_infos("https://youtube.com/watch?v=...".to_string()).await?;
    downloader.download_video(&video, "video.mp4").await?;

    let snapshot = downloader.statistics().snapshot().await;
    println!("Downloads completed:  {}", snapshot.downloads.completed);
    println!("Total bytes:          {}", snapshot.downloads.total_bytes);
    println!("Avg speed (B/s):      {:?}", snapshot.downloads.avg_speed_bytes_per_sec);
    println!("Download success %:   {:?}", snapshot.downloads.success_rate);
    println!("Fetch success %:      {:?}", snapshot.fetches.success_rate);
    println!("Post-process success: {:?}", snapshot.post_processing.success_rate);

    Ok(())
}
```

The snapshot exposes:
- **`downloads`** — attempted, completed, failed, canceled, total bytes, average speed, peak speed, success rate
- **`fetches`** — attempted, succeeded, failed, average duration, success rate (video + playlist fetches)
- **`post_processing`** — attempted, succeeded, failed, average duration
- **`playlists`** — playlists fetched, failed, per-item success rate
- **`recent_downloads`** — bounded history window of completed downloads with per-download details

## 🚀 Advanced Features

### 🔐 Proxy Support

The library supports HTTP, HTTPS, and SOCKS5 proxies for both `yt-dlp` and `reqwest` downloads:

- 🔐 Configuring a proxy with authentication:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::proxy::{ProxyConfig, ProxyType};
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    // Configure proxy with authentication
    let proxy = ProxyConfig::new(ProxyType::Http, "http://proxy.example.com:8080")
        .with_auth("username", "password")
        .with_no_proxy(vec!["localhost".to_string(), "127.0.0.1".to_string()]);

    // Build Downloader with proxy
    let downloader = Downloader::builder(libraries, output_dir)
        .with_proxy(proxy)
        .build()
        .await?;

    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;

    // All downloads (video, audio, thumbnails) will use the proxy
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

Supported proxy types:
- **HTTP/HTTPS**: Standard HTTP proxies
- **SOCKS5**: SOCKS5 proxies for more flexibility
- **Authentication**: Username/password authentication
- **No-proxy list**: Exclude specific domains from proxying

### 🔑 Authentication & Cookies

Many platforms (YouTube bot-protection, Twitch, age-restricted content, etc.) require authentication.
The library supports three authentication modes that are propagated to both metadata extraction and
all download operations automatically.

#### Cookie file (Netscape format)

```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );

    // Export cookies from your browser with a browser extension (e.g. "Get cookies.txt LOCALLY")
    let downloader = Downloader::builder(libraries, PathBuf::from("output"))
        .with_cookies("cookies.txt")
        .build()
        .await?;

    let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

#### Browser cookies

```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );

    // yt-dlp will read cookies directly from your browser's cookie store
    let downloader = Downloader::builder(libraries, PathBuf::from("output"))
        .with_cookies_from_browser("chrome") // or "firefox", "safari", "edge", …
        .build()
        .await?;

    let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

#### Runtime (after build)

```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );

    let mut downloader = Downloader::builder(libraries, PathBuf::from("output"))
        .build()
        .await?;

    // Apply cookies after build — propagates to both extractors and download args
    downloader.set_cookies("cookies.txt");
    // or: downloader.set_cookies_from_browser("chrome");
    // or: downloader.set_netrc();

    let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

#### .netrc

```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );

    let downloader = Downloader::builder(libraries, PathBuf::from("output"))
        .with_netrc()
        .build()
        .await?;

    let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

### ✂️ Clip extraction & chapter splitting

The library supports downloading a specific time range or a specific chapter from a video without fetching the whole file. Seeking is handled by [`media-seek`](crates/media-seek/README.md) — a pure Rust container index parser that translates timestamps into HTTP Range byte offsets.

- ✂️ Downloading a specific time range (seconds):
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Download seconds [60, 120] only — no re-encoding
    let clip_path = downloader
        .download(&video, "clip.mp4")
        .time_range(60.0, 120.0)?
        .execute()
        .await?;

    Ok(())
}
```

- 📖 Downloading a specific chapter by index range:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Download chapters 0 through 2 (inclusive)
    let clip_path = downloader
        .download(&video, "chapters.mp4")
        .chapters(0, 2)?
        .execute()
        .await?;

    Ok(())
}
```

- 🔪 Splitting a downloaded video into one file per chapter:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let video = downloader.fetch_video_infos(url).await?;

    // Download and split into one file per chapter — FFmpeg stream copy, no re-encoding
    let chapter_files: Vec<PathBuf> = downloader
        .split_by_chapters(&video, "output/chapters/")
        .await?;

    for path in &chapter_files {
        println!("Chapter file: {}", path.display());
    }

    Ok(())
}
```

### 🔴 Live Stream Recording (Feature: `live-recording`)

Record live streams using either the pure-Rust reqwest engine or FFmpeg as a fallback.
Enable the feature in your `Cargo.toml`:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["live-recording"] }
```

#### 📥 Basic live recording (reqwest engine)

```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;

    // Record for 1 hour maximum
    let result = downloader.record_live(&video, "live-recording.ts")
        .with_max_duration(Duration::from_secs(3600))
        .execute()
        .await?;

    println!("Recorded {} bytes in {:.1}s", result.total_bytes, result.total_duration.as_secs_f64());
    Ok(())
}
```

#### 🎬 FFmpeg fallback engine

```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::events::RecordingMethod;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;

    let result = downloader.record_live(&video, "live-recording.ts")
        .with_method(RecordingMethod::Fallback)
        .with_max_duration(Duration::from_secs(600))
        .execute()
        .await?;

    Ok(())
}
```

**Implementation details:**
- **Reqwest engine** (default): Pure-Rust HLS segment fetcher. Polls the media playlist, downloads new segments, and writes them sequentially. Zero-copy `bytes::Bytes`, progress events throttled at 50 ms.
- **FFmpeg engine** (fallback): Spawns `ffmpeg -i <url> -c copy <output>`. Stops gracefully via stdin `q`. Useful for encrypted streams or complex HLS features.
- Recording stops on cancellation token, `#EXT-X-ENDLIST`, or max duration.
- Recording events: `LiveRecordingStarted`, `LiveRecordingProgress`, `LiveRecordingStopped`, `LiveRecordingFailed`.
- Streaming events: `LiveStreamStarted`, `LiveStreamProgress`, `LiveStreamStopped`, `LiveStreamFailed`.

#### 📡 Live fragment streaming (Feature: `live-streaming`)

Enable the feature in your `Cargo.toml`:

```toml
[dependencies]
yt-dlp = { version = "2.7.2", features = ["live-streaming"] }
```

```rust,ignore
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg"),
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;

    let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;

    let mut stream = downloader.stream_live(&video)
        .execute()
        .await?;

    while let Some(fragment) = stream.next().await {
        let fragment = fragment?;
        println!("Fragment {} bytes", fragment.data.len());
    }

    Ok(())
}
```

### 🎨 Post-Processing Options

Apply advanced post-processing to videos using FFmpeg:

#### 🔧 Basic codec conversion

- 🔧 Converting video codec and bitrate:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::download::{PostProcessConfig, VideoCodec, AudioCodec};
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );
    let downloader = Downloader::builder(libraries, output_dir).build().await?;

    // Configure post-processing
    let config = PostProcessConfig::new()
        .with_video_codec(VideoCodec::H264)
        .with_audio_codec(AudioCodec::AAC)
        .with_video_bitrate("2M")
        .with_audio_bitrate("192k");

    // Apply to existing video
    downloader.postprocess_video("input.mp4", "output.mp4", config).await?;

    Ok(())
}
```

#### 🎛️ Advanced post-processing with filters

- 🎛️ Applying resolution, framerate, and visual filters:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::download::{
    PostProcessConfig, VideoCodec, Resolution, EncodingPreset,
    FfmpegFilter, WatermarkPosition
};
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );
    let downloader = Downloader::builder(libraries, output_dir).build().await?;

    // Advanced configuration with filters
    let config = PostProcessConfig::new()
        .with_video_codec(VideoCodec::H265)
        .with_resolution(Resolution::HD)
        .with_framerate(30)
        .with_preset(EncodingPreset::Medium)
        .add_filter(FfmpegFilter::Brightness { value: 0.1 })
        .add_filter(FfmpegFilter::Contrast { value: 1.2 })
        .add_filter(FfmpegFilter::Watermark {
            path: "logo.png".to_string(),
            position: WatermarkPosition::BottomRight,
        });

    downloader.postprocess_video("input.mp4", "processed.mp4", config).await?;

    Ok(())
}
```

#### 📋 Available post-processing options

**Video Codecs:**
- H.264 (libx264) - Most compatible
- H.265 (libx265) - Better compression
- VP9 (libvpx-vp9) - Open format
- AV1 (libaom-av1) - Next-gen codec
- Copy - No re-encoding

**Audio Codecs:**
- AAC - High quality, widely supported
- MP3 (libmp3lame) - Universal compatibility
- Opus - Best quality/size ratio
- Vorbis - Open format
- Copy - No re-encoding

**Resolutions:**
- UHD8K (7680x4320)
- UHD4K (3840x2160)
- QHD (2560x1440)
- FullHD (1920x1080)
- HD (1280x720)
- SD (854x480)
- Low (640x360)
- Custom { width, height }

**Encoding Presets:**
- UltraFast, SuperFast, VeryFast, Fast
- Medium (balanced)
- Slow, Slower, VerySlow (best quality)

**Video Filters:**
- **Crop**: `Crop { width, height, x, y }`
- **Rotate**: `Rotate { angle }` (in degrees)
- **Watermark**: `Watermark { path, position }`
- **Brightness**: `Brightness { value }` (-1.0 to 1.0)
- **Contrast**: `Contrast { value }` (0.0 to 4.0)
- **Saturation**: `Saturation { value }` (0.0 to 3.0)
- **Blur**: `Blur { radius }`
- **FlipHorizontal**, **FlipVertical**
- **Denoise**, **Sharpen**
- **Custom**: `Custom { filter }` - Any FFmpeg filter string

### ⚡ Speed Profiles

The library includes an intelligent speed optimization system that automatically configures download parameters based on your internet connection speed. This feature significantly improves download performance for both individual videos and playlists.

#### 📊 Available Speed Profiles

Three pre-configured profiles are available:

**🐢 Conservative** (for connections < 50 Mbps)
- 3 concurrent downloads
- 4-8 parallel segments per file
- 5 MB segment size
- 10 MB buffer
- 2 concurrent playlist downloads
- Best for: Standard internet, avoiding network congestion, limited bandwidth

**⚖️ Balanced** (for connections 50-500 Mbps) - **Default**
- 4 concurrent downloads
- 5–20 parallel segments per file
- 8 MB segment size
- 20 MB buffer
- 3 concurrent playlist downloads
- Best for: Most modern internet connections, general use

**🚀 Aggressive** (for connections > 500 Mbps)
- 6 concurrent downloads
- 6–24 parallel segments per file
- 10 MB segment size
- 30 MB buffer
- 5 concurrent playlist downloads
- Best for: High-bandwidth connections (fiber, gigabit), maximum speed

#### 🚀 Using Speed Profiles

- 🚀 Selecting a speed profile at build time:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::download::SpeedProfile;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    // Use the Aggressive profile for maximum speed
    let downloader = Downloader::builder(libraries, output_dir)
        .with_speed_profile(SpeedProfile::Aggressive)
        .build()
        .await?;

    // All downloads will now use optimized settings
    let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    let video = downloader.fetch_video_infos(url).await?;
    downloader.download_video(&video, "video.mp4").await?;

    Ok(())
}
```

#### ⚙️ Manual Configuration

- ⚙️ Fine-grained control over download parameters:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::download::ManagerConfig;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let libraries = Libraries::new(
        libraries_dir.join("yt-dlp"),
        libraries_dir.join("ffmpeg")
    );

    // Create a custom configuration
    let config = ManagerConfig::builder()
        .max_concurrent_downloads(10)   // 10 concurrent downloads
        .segment_size(15 * 1024 * 1024) // 15 MB segments
        .parallel_segments(16)          // 16 parallel segments
        .build();

    let downloader = Downloader::builder(libraries, output_dir)
        .with_download_manager_config(config)
        .build()
        .await?;

    Ok(())
}
```

#### 📈 Performance Improvements

The speed optimization system includes several advanced features:

- **HTTP/2 Support**: Automatically enabled for better connection multiplexing
- **Transparent Compression**: Responses are automatically decompressed (gzip, brotli) for faster transfers
- **TCP Nodelay**: Nagle's algorithm is disabled for lower latency on small writes
- **CDN-Friendly Range Probing**: Uses `GET` with `Range: bytes=0-0` instead of `HEAD` for better CDN compatibility
- **Progress Throttling**: Progress callbacks are throttled to 50 ms intervals to reduce overhead
- **Parallel Playlist Downloads**: Playlists are downloaded in parallel by default (previously sequential)
- **Dynamic Segment Allocation**: Automatically adjusts the number of parallel segments based on file size
- **Connection Pooling**: Reuses HTTP connections for better performance
- **Intelligent Buffering**: Optimized buffer sizes based on your profile
- **Async DNS** *(opt-in)*: Enable the `hickory-dns` feature for a fully async, pure-Rust DNS resolver

**Expected Performance Gains:**

For individual videos:
- Conservative: ~30% faster (HTTP/2)
- Balanced: ~100% faster (2x segments + HTTP/2)
- Aggressive: ~200% faster (3x segments + HTTP/2)

For playlists:
- Conservative: ~150% faster (2 videos in parallel)
- Balanced: ~200% faster (3 videos in parallel)
- Aggressive: ~400% faster (5 videos in parallel)

**Note**: Actual performance gains depend on your internet speed, server limitations, and network conditions.

## 🌍 Multi-Site Support

**This library supports all 1,800+ extractors from yt-dlp!**

While the examples focus on YouTube (the most common use case), the library works seamlessly with any site supported by yt-dlp. Simply pass the URL - yt-dlp automatically detects the correct extractor.

### 🗂️ Supported Sites

- **Video platforms**: YouTube, Vimeo, Dailymotion, Twitch
- **Social media**: Instagram, TikTok, Twitter/X, Facebook
- **Streaming services**: Netflix, Disney+, Crunchyroll (may require authentication)
- **Music platforms**: Spotify, SoundCloud, Bandcamp
- **News outlets**: CNN, BBC, Fox News
- **And 1,800+ more...**

For the complete list, see [yt-dlp's supported sites](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md).

### 🧩 Examples

- 🎬 Downloading from Vimeo:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    let downloader = Downloader::builder(libraries, PathBuf::from("output")).build().await?;

    let url = "https://vimeo.com/148751763".to_string();
    let video = downloader.fetch_video_infos(url).await?;
    downloader.download_video(&video, "vimeo-video.mp4").await?;

    Ok(())
}
```

- 📱 Downloading from TikTok:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    let downloader = Downloader::builder(libraries, PathBuf::from("output")).build().await?;

    let url = "https://www.tiktok.com/@user/video/123".to_string();
    let video = downloader.fetch_video_infos(url).await?;
    downloader.download_video(&video, "tiktok-video.mp4").await?;

    Ok(())
}
```

- 📸 Downloading from Instagram (may require cookies for authentication):
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    let downloader = Downloader::builder(libraries, PathBuf::from("output")).build().await?;

    let url = "https://www.instagram.com/p/ABC123/".to_string();
    let video = downloader.fetch_video_infos(url).await?;

    Ok(())
}
```

- 🔍 Detecting which extractor handles a URL:
```rust,no_run
use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries = Libraries::new(
        PathBuf::from("libs/yt-dlp"),
        PathBuf::from("libs/ffmpeg")
    );
    let downloader = Downloader::builder(libraries, "output").build().await?;
    let extractor = downloader.detect_extractor("https://vimeo.com/123").await?;
    println!("This URL uses the '{}' extractor", extractor);
    Ok(())
}
```

For detailed documentation, examples, and authentication instructions, see the [`extractor`](https://docs.rs/yt-dlp/latest/yt_dlp/extractor/) module documentation.

## 🔬 Profiling (Feature: `profiling`)

See [PROFILING.md](PROFILING.md) for the complete guide (flamegraph, samply, dhat-rs, heaptrack, Criterion, and the raw vs library comparison tool).

---

## 🏎️ Performances

The library fetches video metadata via `yt-dlp --dump-single-json`, then **downloads format streams
directly over HTTP using parallel segments** — bypassing yt-dlp's sequential download engine.
Run [`benches/compare.rs`](#-profiling-feature-profiling) with any public YouTube URL to
reproduce these numbers on your own connection:

```bash
cargo bench --bench compare --features profiling -- https://www.youtube.com/watch?v=gXtp6C-3JKo --cookies-from-browser safari --runs 10
```

> Results below are averages over 10 runs on a typical broadband connection.
>
> **Methodology**: raw `yt-dlp` re-fetches metadata on every download. The library fetches metadata
> **once**, caches it, and then downloads each format via parallel HTTP segments — this separation
> is the core optimisation reflected in the results below.

### 🎵 Audio streams

| Scenario | `yt-dlp` | Conservative | Balanced *(default)* | Aggressive |
|---|---|---|---|---|
| Audio 96 kbps (Low) | 8.26s | 1.27s | 1.47s | 1.27s |
| Audio 128 kbps (Medium) | 7.83s | 1.11s | 1.12s | 1.18s |
| Audio 192 kbps (High) | 8.53s | 1.38s | 1.26s | 1.19s |
| Audio best quality | 8.51s | 1.22s | 1.15s | 1.15s |

### 🎬 Video streams (no audio)

| Scenario | `yt-dlp` | Conservative | Balanced *(default)* | Aggressive |
|---|---|---|---|---|
| Video 480p | 8.59s | 2.39s | 2.11s | 2.58s |
| Video 720p | 9.84s | 3.67s | 3.86s | 3.89s |
| Video 1080p | 17.6s | 8.65s | 8.60s | 8.81s |
| Video best quality | 16.6s | 11.7s | 11.8s | 12.0s |

### 📦 Muxed streams — native (YouTube pre-muxed, no ffmpeg)

YouTube serves some formats already containing both video and audio tracks. No post-processing
is needed — the file is downloaded as-is.

| Scenario | `yt-dlp` | Conservative | Balanced *(default)* | Aggressive |
|---|---|---|---|---|
| Native 360p (mp4) | 9.69s | 2.11s | 2.00s | 2.16s |
| Native 720p (mp4) | 20.7s | 2.08s | 2.10s | 2.06s |

### 📦 Muxed streams — combined by ffmpeg

For higher-quality streams, YouTube only provides separate video and audio tracks. The library
downloads both in parallel, then ffmpeg merges them using **stream copy** (no re-encoding) when
the audio container is compatible with the output format (e.g. AAC/m4a → mp4).

| Scenario | `yt-dlp` | Conservative | Balanced *(default)* | Aggressive |
|---|---|---|---|---|
| Muxed 480p | 9.41s | 3.51s | 3.48s | 3.34s |
| Muxed 720p | 10.4s | 4.95s | 4.92s | 4.97s |
| Muxed 1080p | 18.3s | 10.4s | 10.5s | 10.3s |
| Muxed best quality | 18.2s | 13.7s | 13.7s | 13.3s |

### 🚀 Speed profiles

| Profile | Parallel segments | Segment size | Use case |
|---|---|---|---|
| `Conservative` | 1–16 | 5 MB | < 50 Mbps connections |
| `Balanced` *(default)* | 2–20 | 8 MB | Most modern connections |
| `Aggressive` | 3–24 | 10 MB | Fibre / gigabit |

See [PROFILING.md](PROFILING.md) for detailed micro-benchmarks.

---

## 💡Features coming soon
- [ ] Use `rust-ffmpeg` and `ffmpeg-sys-next` as safe bindings instead of commands, and keep Command fallback as a feature flag
- [ ] Bandwidth throttling (limit download speed)
- [ ] Download queue persistence (resume queue across restarts)
- [ ] SponsorBlock integration (skip/mark sponsor segments)

---

## 🔍 The `media-seek` crate

This library uses [`media-seek`](crates/media-seek/README.md), a standalone sub-crate published independently on crates.io, for container index parsing. When you use clip extraction or chapter range downloads, `media-seek` parses the stream header to find the exact byte offsets — no subprocess, no FFmpeg involved for the seek step.

**Supported container formats**: MP4/M4A (fMP4 SIDX), WebM/MKV (EBML Cues), MP3 (Xing/VBRI TOC or CBR), OGG (granule bisection), FLAC (SEEKTABLE), WAV, AIFF, AAC/ADTS, FLV (AMF0 keyframes), AVI (`idx1`), MPEG-TS (PCR binary search).

You can also use `media-seek` directly in your own project:

```toml
[dependencies]
media-seek = "0.4.0"
```

See the [`media-seek` README](crates/media-seek/README.md) for the full API reference and usage examples.

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
Make sure to follow the [Contributing Guidelines](CONTRIBUTING.md).

## 📄 License

This project is licensed under the [GPL-3.0 License](LICENSE.md).
