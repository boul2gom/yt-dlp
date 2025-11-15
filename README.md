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
  <a href="https://crates.io/crates/yt-dlp">
    <img src="https://img.shields.io/crates/d/yt-dlp?label=Downloads&logo=Rust" alt="Downloads"/>
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
yt-dlp = "1.4.1"
```

A new release is automatically published every two weeks, to keep up to date with dependencies and features.
Make sure to check the [releases](https://github.com/boul2gom/yt-dlp/releases) page to see the latest version of the crate.

## 🔌 Optional features

This library puts a lot of functionality behind optional features in order to optimize
compile time for the most common use cases. The following features are
available.

- **`cache`** (enabled by default) - Enables video metadata, files and thumbnails caching
- **`tracing`** — <img align="center" width="20" alt="Tracing" src="https://raw.githubusercontent.com/tokio-rs/tracing/refs/heads/master/assets/logo.svg" /> Enables profiling with the [```tracing```](https://crates.io/crates/tracing) crate.
  When this feature is enabled, the library will output span events at log levels `trace` and `debug`, depending on the importance of the called function.
- **`rustls`** - Enables the `rustls-tls` feature in the [```reqwest```](https://crates.io/crates/reqwest) crate.
  This enables building the application without openssl or other system sourced SSL libraries.

#### 📝 Profiling with `tracing` (disabled by default):
The crate supports the `tracing` feature to enable profiling, which can be useful for debugging.
You can enable it by adding the following to your `Cargo.toml` file:
```toml
[dependencies]
yt-dlp = { version = "1.4.1", features = ["tracing"] }
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
```rust
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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
use yt_dlp::client::deps::Libraries;

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

- ✨ Using the fluent API with custom quality preferences:
```rust
use yt_dlp::Youtube;
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
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");

    // Use the fluent download builder API
    let video_path = fetcher.download(url, "my-video.mp4")
        .video_quality(VideoQuality::Q1080p)
        .video_codec(VideoCodecPreference::H264)
        .audio_quality(AudioQuality::Best)
        .execute()
        .await?;

    Ok(())
}
```

- 🎬 Fetching a video (without its audio) and downloading it:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
use yt_dlp::client::deps::Libraries;

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
use yt_dlp::client::deps::Libraries;

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
use yt_dlp::client::deps::Libraries;

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
use yt_dlp::client::deps::Libraries;

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

- 📥 Download with download manager and priority:
```rust
use yt_dlp::Youtube;
use yt_dlp::download::manager::{ManagerConfig, DownloadPriority};
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Custom download manager configuration
    let config = ManagerConfig {
        max_concurrent_downloads: 5,        // Maximum 5 concurrent downloads
        segment_size: 1024 * 1024 * 10,    // 10 MB per segment
        parallel_segments: 8,               // 8 parallel segments per download
        retry_attempts: 5,                  // 5 retry attempts on failure
        max_buffer_size: 1024 * 1024 * 20, // 20 MB maximum buffer
    };

    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");
    
    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");
    
    let libraries = Libraries::new(youtube, ffmpeg);
    
    // Create a fetcher with custom configuration
    let fetcher = Youtube::with_download_manager_config(libraries, output_dir, config)?;

    // Download a video with high priority
    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let video = fetcher.fetch_video_infos(url).await?;
    
    let download_id = fetcher.download_video_with_priority(
        &video, 
        "video-high-priority.mp4", 
        Some(DownloadPriority::High)
    ).await?;

    // Wait for download completion
    let status = fetcher.wait_for_download(download_id).await;
    println!("Final download status: {:?}", status);
    
    Ok(())
}
```

- 📊 Download with progress tracking:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
    
    // Download with progress callback
    let download_id = fetcher.download_video_with_progress(
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
    fetcher.wait_for_download(download_id).await;
    
    Ok(())
}
```

- 🛑 Canceling a download:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
    
    // Start a download
    let download_id = fetcher.download_video_with_priority(
        &video, 
        "video-to-cancel.mp4", 
        None
    ).await?;

    // Check status
    let status = fetcher.get_download_status(download_id).await;
    println!("Download status: {:?}", status);

    // Cancel the download
    let canceled = fetcher.cancel_download(download_id).await;
    println!("Download canceled: {}", canceled);
    
    Ok(())
}
```

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

```rust
use yt_dlp::Youtube;
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
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    
    // Download a high quality video with VP9 codec and high quality audio with Opus codec
    let video_path = fetcher.download_video_with_quality(
        url.clone(),
        "complete-video.mp4",
        VideoQuality::High,
        VideoCodecPreference::VP9,
        AudioQuality::High,
        AudioCodecPreference::Opus
    ).await?;
    
    // Download just the video stream with medium quality and AVC1 codec
    let video_stream_path = fetcher.download_video_stream_with_quality(
        url.clone(),
        "video-only.mp4",
        VideoQuality::Medium,
        VideoCodecPreference::AVC1
    ).await?;
    
    // Download just the audio stream with high quality and AAC codec
    let audio_stream_path = fetcher.download_audio_stream_with_quality(
        url,
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

## 📋 Metadata
The project supports automatic addition of metadata to downloaded files in several formats:

- **MP3**: Title, artist, comment, genre (from tags), release year
- **M4A**: Title, artist, comment, genre (from tags), release year  
- **MP4**: All basic metadata, plus technical information (resolution, FPS, video codec, video bitrate, audio codec, audio bitrate, audio channels, sample rate)
- **WebM**: All basic metadata (via Matroska format), plus technical information as with MP4

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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

    // Check if video has heatmap data
    if video.has_heatmap() {
        if let Some(heatmap) = video.get_heatmap() {
            println!("Video has {} heatmap segments", heatmap.points.len());

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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

    // List all available subtitle languages
    let languages = fetcher.list_subtitle_languages(&video);
    println!("Available subtitle languages: {:?}", languages);

    // Check if specific language is available
    if fetcher.has_subtitle_language(&video, "en") {
        println!("English subtitles are available");
    }

    Ok(())
}
```

- 📥 Downloading a specific subtitle:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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

    // Download English subtitles
    let subtitle_path = fetcher
        .download_subtitle(&video, "en", "subtitle_en.srt")
        .await?;
    println!("Subtitle downloaded to: {:?}", subtitle_path);

    Ok(())
}
```

- 📥 Downloading all available subtitles:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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

    // Download all available subtitles
    let subtitle_paths = fetcher
        .download_all_subtitles(&video, &output_dir)
        .await?;
    println!("Downloaded {} subtitle files", subtitle_paths.len());

    Ok(())
}
```

- 🎬 Embedding subtitles into a video:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

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

    // Download video
    let video_path = fetcher.download_video(&video, "video.mp4").await?;

    // Download subtitles
    let en_subtitle = fetcher
        .download_subtitle(&video, "en", "subtitle_en.srt")
        .await?;
    let fr_subtitle = fetcher
        .download_subtitle(&video, "fr", "subtitle_fr.srt")
        .await?;

    // Embed subtitles into video
    let video_with_subs = fetcher
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
```rust
use yt_dlp::Youtube;
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
    let fetcher = Youtube::new(libraries, output_dir)?;

    let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    let video = fetcher.fetch_video_infos(url).await?;

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

## 📂 Playlists

The library provides full playlist support, including fetching playlist metadata and downloading videos with various selection options:

- 📋 Fetching playlist information:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = fetcher.fetch_playlist_infos(playlist_url).await?;

    println!("Playlist: {}", playlist.title);
    println!("Videos: {}", playlist.entry_count());
    println!("Uploader: {}", playlist.uploader);

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
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = fetcher.fetch_playlist_infos(playlist_url).await?;

    // Download all videos with a pattern
    // Use %(playlist_index)s for index, %(title)s for title, %(id)s for video ID
    let video_paths = fetcher
        .download_playlist(&playlist, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} videos", video_paths.len());

    Ok(())
}
```

- 🎯 Downloading specific videos by index:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = fetcher.fetch_playlist_infos(playlist_url).await?;

    // Download specific videos by index (0-based)
    let indices = vec![0, 2, 5, 10]; // Videos at positions 1, 3, 6, and 11
    let video_paths = fetcher
        .download_playlist_items(&playlist, &indices, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} specific videos", video_paths.len());

    Ok(())
}
```

- 📊 Downloading a range of videos:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = fetcher.fetch_playlist_infos(playlist_url).await?;

    // Download videos 5-15 (0-based, inclusive)
    let video_paths = fetcher
        .download_playlist_range(&playlist, 5, 15, "%(playlist_index)s - %(title)s.mp4")
        .await?;

    println!("Downloaded {} videos from range", video_paths.len());

    Ok(())
}
```

- 🔍 Filtering and analyzing playlists:
```rust
use yt_dlp::Youtube;
use std::path::PathBuf;
use yt_dlp::client::deps::Libraries;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let libraries_dir = PathBuf::from("libs");
    let output_dir = PathBuf::from("output");

    let youtube = libraries_dir.join("yt-dlp");
    let ffmpeg = libraries_dir.join("ffmpeg");

    let libraries = Libraries::new(youtube, ffmpeg);
    let fetcher = Youtube::new(libraries, output_dir)?;

    let playlist_url = String::from("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
    let playlist = fetcher.fetch_playlist_infos(playlist_url).await?;

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

## 💡Features coming soon
- [ ] Proxy support for `yt-dlp` and `reqwest`
- [ ] Downloading only a part of a video or audio (with time or chapter)
- [ ] Post-processing options with `ffmpeg`
- [ ] Live streams serving, through a local server
- [ ] Live streams recording, with `ffmpeg` or `reqwest`
- [ ] Notifications and alerts on download events
- [ ] Webhooks, Rust hooks and callbacks on download events, errors and progress
- [ ] Statistics and analytics on downloads and fetches
