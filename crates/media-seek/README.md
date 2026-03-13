<h2 align="center">🔍 A transport-agnostic container format index parser for Rust</h2>

<div align="center">Parse seek indices from MP4, WebM, MP3, OGG, FLAC, WAV, AIFF, AAC, FLV, AVI, and MPEG-TS streams.<br>
Translate any <code>[start_secs, end_secs]</code> window into the exact byte ranges needed for partial HTTP downloads — no subprocess, no HTTP client bundled.</div>

<br>
<div align="center">⚠️ Part of the <a href="https://github.com/boul2gom/yt-dlp">yt-dlp</a> workspace. Published independently on crates.io.</div>
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
  <a href="https://crates.io/crates/media-seek">
    <img src="https://img.shields.io/crates/v/media-seek?label=Release&logo=Rust" alt="Release"/>
  </a>
  <a href="https://crates.io/crates/media-seek">
    <img src="https://img.shields.io/crates/d/media-seek?label=Downloads&logo=Rust" alt="Downloads"/>
  </a>
  <a href="https://docs.rs/media-seek">
    <img src="https://img.shields.io/docsrs/media-seek?label=docs.rs&logo=Rust" alt="docs.rs"/>
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
    <img src="https://img.shields.io/github/issues-pr-raw/boul2gom/yt-dlp?label=Pull%20requests&logo=Github" alt="Pull requests">
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
</p>

---

## 💭 Why media-seek?

Downloading only a time clip from a streaming URL requires knowing *which bytes* to request. Every container format stores this information differently — MP4 uses a `sidx` box, WebM has EBML Cues, MP3 uses the Xing TOC, and so on.

`media-seek` abstracts all of that. You feed it the leading bytes of a stream and it returns a `ContainerIndex` that answers one question: **given a time window `[start, end]`, which bytes do I need to fetch?**

There is no HTTP client bundled and no subprocess spawned. Callers implement the two-line `RangeFetcher` trait to supply bytes from whatever transport they use. Formats whose index lies outside the probe window (WebM Cues, OGG page bisection, AVI `idx1`, MPEG-TS PCR search) request additional ranges through that trait automatically.

## 📥 How to get it

Add the following to your `Cargo.toml` file:
```toml
[dependencies]
media-seek = "0.4.0"
```

Check the [releases](https://github.com/boul2gom/yt-dlp/releases) page for the latest version.

### 🔍 Observability & Tracing

This crate always includes the <img align="center" width="20" alt="Tracing" src="https://raw.githubusercontent.com/tokio-rs/tracing/refs/heads/master/assets/logo.svg" /> [`tracing`](https://crates.io/crates/tracing) crate. It emits `debug` events for each format detection, parse step, and extra fetch.

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
    .with_max_level(Level::DEBUG)
    .finish();
tracing::subscriber::set_global_default(subscriber)
    .expect("setting default subscriber failed");
```

Refer to the [`tracing-subscriber` documentation](https://docs.rs/tracing-subscriber) for more advanced configuration (JSON output, log levels, targets, etc.).

---

## 🎯 Supported formats

| Extension | Index type | Precision | Extra fetches |
|-----------|-----------|-----------|---------------|
| `mp4`, `m4a` | fMP4 SIDX box | Fragment boundary | No |
| `webm` | EBML Cues element | Cluster boundary | Maybe |
| `mp3` | Xing/VBRI TOC or CBR avg | Frame / 1 % TOC entry | No |
| `ogg` | OGG page granule bisection | Page boundary | Yes (up to 64) |
| `flac` | SEEKTABLE metadata block | Seek point | No |
| `wav` | PCM formula | BlockAlign-exact | No |
| `aiff` | PCM formula | Sample-exact | No |
| `aac` | ADTS frame scan average | ~21 ms frame | No |
| `flv` | AMF0 onMetaData keyframes | Keyframe | No |
| `avi` | `idx1` chunk at EOF | Frame | Yes (1 fetch) |
| `ts` | PCR binary search | ~11 ms TS packet | Yes (up to 64) |

MHTML (storyboard segments), `None`, and unrecognized magic bytes return `Err(Error::UnsupportedFormat)`.

## 🚀 Quick start

### 1. Implement `RangeFetcher`

```rust,ignore
use media_seek::RangeFetcher;

struct HttpFetcher {
    client: reqwest::Client,
    url: String,
}

impl RangeFetcher for HttpFetcher {
    type Error = reqwest::Error;

    async fn fetch(&self, start: u64, end: u64) -> std::result::Result<Vec<u8>, Self::Error> {
        let range = format!("bytes={}-{}", start, end);
        self.client
            .get(&self.url)
            .header("Range", range)
            .send()
            .await?
            .bytes()
            .await
            .map(|b| b.to_vec())
    }
}
```

### 2. Parse the container index

```rust,no_run
use media_seek::{parse, RangeFetcher};

struct FileFetcher(Vec<u8>);

impl RangeFetcher for FileFetcher {
    type Error = std::io::Error;

    async fn fetch(&self, start: u64, end: u64) -> std::result::Result<Vec<u8>, Self::Error> {
        let s = start as usize;
        let e = (end as usize + 1).min(self.0.len());
        Ok(self.0[s..e].to_vec())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetcher = FileFetcher(std::fs::read("video.mp4")?);

    // 512 KB is recommended — enough for most format headers and indices
    let probe: Vec<u8> = fetcher.fetch(0, 512 * 1024).await?;

    // Total stream size in bytes (from Content-Length or a HEAD request)
    let total_size: Option<u64> = Some(1_234_567_890);

    let index = parse(&probe, total_size, &fetcher).await?;
    Ok(())
}
```

### 3. Translate timestamps to byte ranges

```rust,no_run
use media_seek::{parse, RangeFetcher};

struct FileFetcher(Vec<u8>);

impl RangeFetcher for FileFetcher {
    type Error = std::io::Error;

    async fn fetch(&self, start: u64, end: u64) -> std::result::Result<Vec<u8>, Self::Error> {
        let s = start as usize;
        let e = (end as usize + 1).min(self.0.len());
        Ok(self.0[s..e].to_vec())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fetcher = FileFetcher(std::fs::read("video.mp4")?);
    let probe: Vec<u8> = fetcher.fetch(0, 512 * 1024).await?;
    let total_size: Option<u64> = Some(1_234_567_890);

    let index = parse(&probe, total_size, &fetcher).await?;

    if let Some(range) = index.find_byte_range(60.0, 120.0) {
        // Always prefetch the init segment so decoders have codec parameters
        let init = fetcher.fetch(0, index.init_end_byte).await?;
        let clip = fetcher.fetch(range.start, range.end).await?;

        // Write init + clip to a file, then trim with FFmpeg stream copy:
        // ffmpeg -i combined.mp4 -ss 60 -t 60 -c copy -avoid_negative_ts 1 -y out.mp4
        let _ = (init, clip);
    }
    Ok(())
}
```

---

## 📖 Documentation

The full API reference is available on [docs.rs](https://docs.rs/media-seek).

### `parse()`

```rust,ignore
pub async fn parse<F: RangeFetcher>(
    probe: &[u8],
    total_size: Option<u64>,
    fetcher: &F,
) -> Result<ContainerIndex>;
```

Detects the container format from magic bytes in `probe` and dispatches to the appropriate parser. Returns `Err(Error::UnsupportedFormat)` for unrecognised formats.

### `ContainerIndex`

```rust,ignore
pub struct ContainerIndex {
    /// Last byte (inclusive) of the codec initialisation data (moov, EBML header, …).
    /// A partial download must always include `bytes 0..=init_end_byte`.
    pub init_end_byte: u64,
}

impl ContainerIndex {
    /// Returns `Some(ByteRange { start, end })` covering `[start_secs, end_secs]`,
    /// expanded to the nearest decodable boundary, or `None` if the range is not covered.
    pub fn find_byte_range(&self, start_secs: f64, end_secs: f64) -> Option<ByteRange>;
}
```

### `RangeFetcher` trait

```rust,ignore
pub trait RangeFetcher {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fetches bytes `[start, end]` (inclusive) from the remote stream.
    fn fetch(&self, start: u64, end: u64)
        -> impl Future<Output = std::result::Result<Vec<u8>, Self::Error>> + Send;
}
```

`fetch` is called only when extra data is required beyond the initial probe:

- **WebM** — when the Cues element starts beyond the probe window.
- **OGG** — up to 64 equidistant binary-search probes across the stream.
- **AVI** — one fetch of the last 64 KB to locate the `idx1` chunk.
- **MPEG-TS** — up to 64 equidistant binary-search probes for PCR timestamps.

All other formats (MP4, MP3, FLAC, WAV, AIFF, AAC, FLV) parse entirely from the probe.

## 🚨 Error handling

```rust,ignore
pub enum Error {
    /// MHTML, plaintext, or unrecognized magic bytes.
    UnsupportedFormat,
    /// Container index could not be parsed (truncated data, invalid structure).
    ParseFailed { reason: String },
    /// An extra Range fetch required by the parser failed.
    FetchFailed(Box<dyn std::error::Error + Send + Sync>),
}
```

`UnsupportedFormat` is the expected case for storyboard MHTML segments and non-media content. Callers should handle it by falling back to a full download or reporting that seeking is unavailable.

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
Make sure to follow the [Contributing Guidelines](../../CONTRIBUTING.md).

## 📄 License

This project is licensed under the [GPL-3.0 License](../../LICENSE.md).
