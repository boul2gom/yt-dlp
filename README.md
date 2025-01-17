<h2 align="center">🎬️ A Rust library (with auto dependencies installation) for YouTube downloading</h2>

<div align="center">This library is a Rust asynchronous wrapper around the yt-dlp command line tool, a feature-rich youtube (and others) audio/video downloader, which is a fork of youtube-dl with a lot of additional features and improvements.</div>
<div align="center">
  The crate is designed to download audio and video from various websites.
  You don't need to care about dependencies, yt-dlp and ffmpeg will be downloaded automatically.
</div>

<br>
<div align="center">⚠️ The project is still in development, so if you encounter any bugs or have any feature requests, please open an issue or a discussion.</div>
<br>

<div align="center">
  <a href="https://github.com/boul2gom/yt-dlp/issues/new?assignees=&labels=bug&template=BUG_REPORT.md&title=bug%3A+">Report a Bug</a>
  ·
  <a href="https://github.com/boul2gom/yt-dlp/discussions/new?assignees=&labels=enhancement&title=feat%3A+">Request a Feature</a>
  ·
  <a href="https://github.com/boul2gom/yt-dlp/discussions/new?assignees=&labels=help%20wanted&title=ask%3A+">Ask a Question</a>
</div>

---

<p align="center">
  <a href="https://github.com/boul2gom/yt-dlp/actions/workflows/ci-dev.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/boul2gom/yt-dlp/ci-dev.yml?label=Develop%20CI&logo=Github" alt="Develop CI"/>
  </a>  
  <a href="https://crates.io/crates/yt-dlp">
    <img src="https://img.shields.io/github/v/release/boul2gom/yt-dlp?label=Release&logo=Rust" alt="Release"/>
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/blob/develop/LICENSE.md">
    <img src="https://img.shields.io/github/license/boul2gom/yt-dlp?label=License&logo=Github" alt="License">
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
    <img src="https://img.shields.io/github/issues-pr-raw/boul2gom/yt-dlp?label=Pull requests&logo=Github" alt="Pull requests">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/stargazers">
    <img src="https://img.shields.io/github/stars/boul2gom/yt-dlp?label=Stars&logo=Github" alt="Stars">
  </a>
  <a href="https://github.com/boul2gom/yt-dlp/fork">
    <img src="https://img.shields.io/github/forks/boul2gom/yt-dlp?label=Forks&logo=Github" alt="Forks">
  </a>
</p>

<p align="center">
    🚧 Few stats and tools scores about the project: 
</p>
<p align="center">
  <a href="https://scorecard.dev/viewer/?uri=github.com/boul2gom/yt-dlp">
    <img src="https://img.shields.io/ossf-scorecard/github.com/boul2gom/yt-dlp?label=Scorecard&logo=SecurityScorecard" alt="OpenSSF Scorecard"/>
  </a>
  <a href="https://crates.io/crates/yt-dlp">
    <img src="https://img.shields.io/crates/d/yt-dlp?label=Downloads&logo=Rust" alt="Downloads"/>
  </a>
  <img src="https://img.shields.io/sonar/tech_debt/boul2gom_yt-dlp?server=https%3A%2F%2Fsonarcloud.io&label=Tech%20Debt&logo=SonarCloud" alt="Tech Debt"/>
  <img src="https://img.shields.io/codefactor/grade/github/boul2gom/yt-dlp/develop?label=Quality&logo=Codefactor" alt="Codefactor">
</p>

---

## 💭️ Why using external Python app ?

Originally, to download videos from YouTube, I used the [```rustube```](https://crates.io/crates/rustube) crate, written in pure Rust and without any external dependencies.
However, I quickly realized that due to frequent breaking changes on the YouTube website, the crate was outdated and no longer functional.

After few tests and researches, I concluded that the python app [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) was the best compromise, thanks to its regular updates and massive community.
His standalone binaries and his ability to output the fetched data in JSON format make it a most imperfect candidate for a Rust wrapper.

Using an external program is not ideal, but it is the most reliable and maintained solution for now.

## 📥 How to get it

Add the following to your `Cargo.toml` file:
```toml
[dependencies]
yt-dlp = "latest version of the crate"
```

A new release is automatically published every two weeks, to keep up to date with dependencies and features.
Make sure to check the [releases](https://github.com/boul2gom/yt-dlp/releases) page to see the latest version of the crate.

## 🔌 Optional features

This library puts a lot of functionality behind optional features in order to optimize
compile time for the most common use cases. The following features are
available.

- **`tracing`** — <img align="center" width="20" alt="Tracing" src="https://raw.githubusercontent.com/tokio-rs/tracing/refs/heads/master/assets/logo.svg" /> Enables profiling with the [```tracing```](https://crates.io/crates/tracing) crate.
  When this feature is enabled, the library will output span events at log levels `trace` and `debug`, depending on the importance of the called function.

#### 📝 Profiling with `tracing` (disabled by default):
The crate supports the `tracing` feature to enable profiling, which can be useful for debugging.
You can enable it by adding the following to your `Cargo.toml` file:
```toml
[dependencies]
yt-dlp = { version = "latest version of the crate", features = ["tracing"] }
```

## 📖 Documentation

The documentation is available on [docs.rs](https://docs.rs/yt-dlp).

## 📚 Examples

- 📦 Installing the [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) and [```ffmpeg```](https://ffmpeg.org/) binaries:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executables_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let fetcher = Youtube::with_new_binaries(executables_dir, output_dir).await?;
    Ok(())
}
```

- 📦 Installing the [```yt-dlp```](https://github.com/yt-dlp/yt-dlp/) binary only:
```rust
use yt_dlp::fetcher::deps::LibraryInstaller;
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
```rust
use yt_dlp::fetcher::deps::LibraryInstaller;
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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    fetcher.update_downloader().await?;
    Ok(())
}
```

- 📥 Fetching a video (with its audio) and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let video_path = fetcher.download_video_from_url(url, "my-video.mp4").await?;
    Ok(())
}
```

- 🎬 Fetching a video (without its audio) and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;
    
    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    fetcher.download_video_stream_from_url(url, "video.mp4").await?;
    Ok(())
}
```

- 🎵 Fetching an audio and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    fetcher.download_audio_stream_from_url(url, "audio.mp3").await?;
    Ok(())
}
```

- 📜 Fetching a specific format and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;
    
    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let video = fetcher.fetch_video_infos(url).await?;
    println!("Video title: {}", video.title);

    let video_format = video.best_video_format().unwrap();
    let format_path = fetcher.download_format(&video_format, "my-video-stream.mp4").await?;
    
    let audio_format = video.worst_audio_format().unwrap();
    let audio_path = fetcher.download_format(&audio_format, "my-audio-stream.mp3").await?;
    
    Ok(())
}
```

- ⚙️ Combining an audio and a video file into a single file:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let video = fetcher.fetch_video_infos(url).await?;

    let audio_format = video.best_audio_format().unwrap();
    let audio_path = fetcher.download_format(&audio_format, "audio-stream.mp3").await?;

    let video_format = video.worst_video_format().unwrap();
    let video_path = fetcher.download_format(&video_format, "video-stream.mp4").await?;

    let output_path = fetcher.combine_audio_and_video("audio-stream.mp3", "video-stream.mp4", "my-output.mp4").await?;
    Ok(())
}
```

- 📸 Fetching a thumbnail and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::fetcher::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let thumbnail_path = fetcher.download_thumbnail_from_url(url, "thumbnail.jpg").await?;
    Ok(())
}
```

## 🐛 Bugs reported
- [ ] Output dir not created, so video are downloaded in the void
- [ ] Dependencies are re-installed
- [ ] Download is very slow

## 💡Support coming soon
- [ ] Subtitles
- [ ] Chapters
- [ ] Heatmap
- [ ] Playlist (and index)
- [ ] TikTok videos
- [ ] Instagram videos
- [ ] Twitter videos
- [ ] Twitch videos
- [ ] Global ```yt-dlp``` providers

## 💡Features coming soon
- [ ] Common traits on all structs
- [ ] Cache system for fetched data (downloaded files with metadata, hash and format stored in a database)
- [ ] Metadata and tags on downloaded files
- [ ] Thumbnails and cover arts on downloaded files
- [ ] Proxy support for ```yt-dlp``` and ```reqwest```
- [ ] Resuming downloads with HTTP Range requests
- [ ] Downloading only a part of a video or audio
- [ ] Audio and video format selection with enums
- [ ] Post-processing options with ```ffmpeg```
- [ ] Live streams serving, through a local server
- [ ] Live streams recording, with ```ffmpeg``` or ```reqwest```
- [ ] Notifications and alerts on download events
- [ ] Webhooks, Rust hooks and callbacks on download events, errors and progress
- [ ] Scheduled downloads and downloads queue with priority and network limits
- [ ] Statistics and analytics on downloads and fetches
