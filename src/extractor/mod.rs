//! Video extractor system for multi-site support.
//!
//! This module provides a trait-based architecture for handling different video sites:
//! - `Youtube`: Highly optimized extractor for YouTube with platform-specific features
//! - `Generic`: Universal extractor for all other yt-dlp supported sites
//!
//! The `Downloader` struct automatically detects and uses the appropriate extractor.

use std::fmt;

use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};

use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;

/// Identifies which extractor implementation is in use.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtractorName {
    /// YouTube-specific extractor with platform optimizations.
    Youtube,
    /// Generic extractor for all other yt-dlp supported sites.
    /// Contains the optional site-specific extractor name reported by yt-dlp
    /// (e.g. `"vimeo"`, `"tiktok"`).
    Generic(Option<String>),
}

impl fmt::Display for ExtractorName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Youtube => f.write_str("Youtube"),
            Self::Generic(Some(name)) => write!(f, "Generic(name={})", name),
            Self::Generic(None) => f.write_str("Generic"),
        }
    }
}

/// Core trait for video extractors.
///
/// This trait defines the common interface that all extractors must implement.
/// Each extractor handles fetching video metadata and playlists from their respective platform.
#[async_trait]
pub trait VideoExtractor: Downcast + Send + Sync + fmt::Debug {
    /// Fetch video metadata from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The video URL to fetch
    ///
    /// # Returns
    ///
    /// Video metadata including formats, title, duration, etc.
    ///
    /// # Errors
    ///
    /// Returns error if the URL is unsupported, geo-blocked, or requires authentication
    async fn fetch_video(&self, url: &str) -> Result<Video>;

    /// Fetch playlist metadata from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The playlist URL to fetch
    ///
    /// # Returns
    ///
    /// Playlist metadata including entries and metadata
    ///
    /// # Errors
    ///
    /// Returns error if the URL is unsupported or invalid
    async fn fetch_playlist(&self, url: &str) -> Result<Playlist>;

    /// Get the name of this extractor.
    fn name(&self) -> ExtractorName;

    /// Check if this extractor supports the given URL pattern.
    ///
    /// This is a fast, synchronous check based on URL patterns.
    /// Use `fetch_video()` for definitive validation.
    fn supports_url(&self, url: &str) -> bool;
}

impl_downcast!(VideoExtractor);

/// Common configuration methods for all extractors.
pub trait ExtractorConfig: VideoExtractor {
    /// Returns a mutable reference to the internal args vector.
    fn args_mut(&mut self) -> &mut Vec<String>;

    /// Returns a mutable reference to the internal timeout.
    fn timeout_mut(&mut self) -> &mut Duration;

    /// Add custom yt-dlp argument.
    fn with_arg(&mut self, arg: String) -> &mut Self {
        self.args_mut().push(arg);
        self
    }

    /// Set timeout for yt-dlp operations.
    fn with_timeout(&mut self, timeout: Duration) -> &mut Self {
        *self.timeout_mut() = timeout;
        self
    }

    /// Use a Netscape cookie file for authentication.
    fn with_cookies(&mut self, path: impl AsRef<Path>) -> &mut Self {
        let cookie_path = path.as_ref().display().to_string();
        self.with_arg(format!("--cookies={}", cookie_path))
    }

    /// Extract cookies from a browser for authentication.
    fn with_cookies_from_browser(&mut self, browser: &str) -> &mut Self {
        self.with_arg(format!("--cookies-from-browser={}", browser))
    }

    /// Use .netrc for authentication.
    fn with_netrc(&mut self) -> &mut Self {
        self.with_arg("--netrc".to_string())
    }
}

pub mod detector;
pub mod generic;
pub mod youtube;

/// Common logic for extractors to execute yt-dlp and parse output.
#[async_trait]
pub trait ExtractorBase: VideoExtractor {
    /// Get the executable path.
    fn executable_path(&self) -> PathBuf;
    /// Get the request timeout.
    fn timeout(&self) -> Duration;
    /// Build base arguments for yt-dlp.
    fn build_base_args(&self) -> Vec<String>;

    /// Fetch and parse video metadata.
    async fn fetch_video_metadata(&self, url: &str) -> Result<Video> {
        let mut args = self.build_base_args();
        args.push(url.to_string());
        execute_and_parse_video(self.executable_path(), &args, self.timeout()).await
    }

    /// Fetch and parse playlist metadata.
    async fn fetch_playlist_metadata(&self, url: &str) -> Result<Playlist> {
        let mut args = self.build_base_args();
        args.push("--flat-playlist".to_string());
        args.push(url.to_string());
        execute_and_parse_playlist(self.executable_path(), &args, self.timeout()).await
    }

    /// Fetches video metadata and emits structured tracing on success or failure.
    ///
    /// Wraps `fetch_video_metadata` with a consistent log pattern. Call this from
    /// `VideoExtractor::fetch_video` implementations to avoid repeating the
    /// match-and-log boilerplate. The `extractor` string is emitted as a structured
    /// field so each implementor can identify itself in logs.
    async fn log_and_fetch_video(&self, url: &str, extractor: &str) -> Result<Video> {
        let result = self.fetch_video_metadata(url).await;
        match &result {
            Ok(video) => tracing::debug!(
                url = url,
                extractor = extractor,
                video_id = video.id,
                title = video.title,
                format_count = video.formats.len(),
                "✅ Video fetched successfully"
            ),
            Err(e) => tracing::warn!(
                url = url,
                extractor = extractor,
                error = %e,
                "Failed to fetch video"
            ),
        }
        result
    }

    /// Fetches playlist metadata and emits structured tracing on success or failure.
    ///
    /// Wraps `fetch_playlist_metadata` with a consistent log pattern. Call this from
    /// `VideoExtractor::fetch_playlist` implementations to avoid repeating the
    /// match-and-log boilerplate.
    async fn log_and_fetch_playlist(&self, url: &str, extractor: &str) -> Result<Playlist> {
        let result = self.fetch_playlist_metadata(url).await;
        match &result {
            Ok(playlist) => tracing::debug!(
                url = url,
                extractor = extractor,
                playlist_id = playlist.id,
                title = playlist.title,
                entry_count = playlist.entries.len(),
                "✅ Playlist fetched successfully"
            ),
            Err(e) => tracing::warn!(
                url = url,
                extractor = extractor,
                error = %e,
                "Failed to fetch playlist"
            ),
        }
        result
    }
}

/// Implements [`ExtractorConfig`] for a struct with `args: Vec<String>` and `timeout: Duration` fields.
///
/// Both fields must be named exactly `args` and `timeout`.
macro_rules! impl_extractor_config {
    ($type:path) => {
        impl $crate::extractor::ExtractorConfig for $type {
            fn args_mut(&mut self) -> &mut Vec<String> {
                &mut self.args
            }

            fn timeout_mut(&mut self) -> &mut std::time::Duration {
                &mut self.timeout
            }
        }
    };
}
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use detector::detect_extractor_type;
pub use generic::Generic;
pub(crate) use impl_extractor_config;
pub use youtube::Youtube;

use crate::executor::Executor;

/// Internal generic helper: execute yt-dlp and parse its JSON output as `T`.
///
/// Handles the common pattern of creating an `Executor`, writing output to a
/// temporary file, and deserialising it with `serde_json` inside `spawn_blocking`.
async fn execute_and_parse<T>(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
    label: &'static str,
) -> Result<T>
where
    T: serde::de::DeserializeOwned + Send + 'static,
{
    tracing::debug!(
        executable = ?executable_path,
        arg_count = args.len(),
        timeout_secs = timeout.as_secs(),
        "📡 Executing extractor for {label}"
    );

    let executor = Executor::new(executable_path.clone(), args.to_vec(), timeout);

    let temp_dir = tempfile::tempdir()?;
    let output_path = temp_dir.path().join(format!("{}_{}.json", label, uuid::Uuid::new_v4()));

    tracing::debug!(
        executable = ?executable_path,
        output_path = ?output_path,
        "📡 Redirecting yt-dlp output to temporary file"
    );

    let _output = executor.execute_to_file(&output_path).await?;

    tracing::debug!(output_path = ?output_path, "⚙️ Opening output file for parsing");

    let file = tokio::fs::File::open(&output_path).await?;
    let file = file.into_std().await;

    tracing::debug!("⚙️ Spawning blocking task for JSON parsing");

    let result: T =
        tokio::task::spawn_blocking(move || serde_json::from_reader(std::io::BufReader::new(file))).await??;

    Ok(result)
}

/// Helper to execute the extractor command and parse the output as a Video.
///
/// # Errors
///
/// Returns an error if execution fails, JSON parsing fails, or the operation times out
pub async fn execute_and_parse_video(executable_path: PathBuf, args: &[String], timeout: Duration) -> Result<Video> {
    let mut video: Video = execute_and_parse(executable_path, args, timeout, "video").await?;

    tracing::debug!(
        video_id = %video.id,
        title = %video.title,
        format_count = video.formats.len(),
        "✅ Video parsed successfully"
    );

    for format in &mut video.formats {
        format.video_id = Some(video.id.clone());
    }

    tracing::debug!(video_id = %video.id, "⚙️ Set video_id on all formats");

    Ok(video)
}

/// Helper to execute the extractor command and parse the output as a Playlist.
///
/// # Errors
///
/// Returns an error if execution fails, JSON parsing fails, or the operation times out
pub async fn execute_and_parse_playlist(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<Playlist> {
    let playlist: Playlist = execute_and_parse(executable_path, args, timeout, "playlist").await?;

    tracing::debug!(
        playlist_id = %playlist.id,
        title = %playlist.title,
        entry_count = playlist.entries.len(),
        "✅ Playlist parsed successfully"
    );

    Ok(playlist)
}
