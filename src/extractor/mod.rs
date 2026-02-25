//! Video extractor system for multi-site support.
//!
//! This module provides a trait-based architecture for handling different video sites:
//! - `Youtube`: Highly optimized extractor for YouTube with platform-specific features
//! - `Generic`: Universal extractor for all other yt-dlp supported sites
//!
//! The `Downloader` struct automatically detects and uses the appropriate extractor.

use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use async_trait::async_trait;
use downcast_rs::{Downcast, impl_downcast};
use std::fmt;

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
            Self::Youtube => write!(f, "youtube"),
            Self::Generic(Some(name)) => write!(f, "{}", name),
            Self::Generic(None) => write!(f, "generic"),
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
    /// Add custom yt-dlp argument.
    fn with_arg(&mut self, arg: String) -> &mut Self;

    /// Set timeout for yt-dlp operations.
    fn with_timeout(&mut self, timeout: Duration) -> &mut Self;

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
}

pub use detector::detect_extractor_type;
pub use generic::Generic;
pub use youtube::Youtube;

use crate::executor::Executor;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Helper to execute the extractor command and parse the output as a Video.
///
/// This handles the common pattern of:
/// 1. Creating an Executor
/// 2. Running it
/// 3. Deserializing the JSON output
/// 4. Post-processing the video (e.g. setting video_id on formats)
///
/// # Arguments
///
/// * `executable_path` - Path to the yt-dlp executable
/// * `args` - Arguments to pass to yt-dlp
/// * `timeout` - Maximum duration to wait for execution
///
/// # Returns
///
/// Parsed Video metadata with formats
///
/// # Errors
///
/// Returns an error if execution fails, JSON parsing fails, or the operation times out
pub async fn execute_and_parse_video(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<Video> {
    tracing::debug!(
        executable = ?executable_path,
        arg_count = args.len(),
        timeout_secs = timeout.as_secs(),
        "Executing extractor for video"
    );

    let executor = Executor::new(executable_path.clone(), args.to_vec(), timeout);

    // Create a temporary directory to store the output JSON
    // This avoids loading the entire JSON into memory as a string
    let temp_dir = tempfile::tempdir()?;
    let output_path = temp_dir
        .path()
        .join(format!("video_{}.json", uuid::Uuid::new_v4()));

    tracing::debug!(
        executable = ?executable_path,
        output_path = ?output_path,
        "Redirecting yt-dlp output to temporary file"
    );

    let _output = executor.execute_to_file(&output_path).await?;

    tracing::debug!(
        output_path = ?output_path,
        "Opening output file for parsing"
    );

    // Open the file using tokio::fs (async)
    let file = tokio::fs::File::open(&output_path).await?;
    // Convert to std::fs::File for serde_json which is synchronous
    let file = file.into_std().await;

    tracing::debug!("Spawning blocking task for JSON parsing");

    // Use spawn_blocking to perform CPU-intensive and blocking I/O JSON parsing
    // without blocking the async runtime
    let mut video: Video = tokio::task::spawn_blocking(move || {
        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader)
    })
    .await??;

    tracing::debug!(
        video_id = %video.id,
        title = %video.title,
        format_count = video.formats.len(),
        "Video parsed successfully"
    );

    // Set video ID on each format for caching purposes
    for format in &mut video.formats {
        format.video_id = Some(video.id.clone());
    }

    tracing::debug!(
        video_id = %video.id,
        "Set video_id on all formats"
    );

    Ok(video)
}

/// Helper to execute the extractor command and parse the output as a Playlist.
///
/// # Arguments
///
/// * `executable_path` - Path to the yt-dlp executable
/// * `args` - Arguments to pass to yt-dlp
/// * `timeout` - Maximum duration to wait for execution
///
/// # Returns
///
/// Parsed Playlist metadata with entries
///
/// # Errors
///
/// Returns an error if execution fails, JSON parsing fails, or the operation times out
pub async fn execute_and_parse_playlist(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<Playlist> {
    tracing::debug!(
        executable = ?executable_path,
        arg_count = args.len(),
        timeout_secs = timeout.as_secs(),
        "Executing extractor for playlist"
    );

    let executor = Executor::new(executable_path.clone(), args.to_vec(), timeout);

    // Create a temporary directory to store the output JSON
    let temp_dir = tempfile::tempdir()?;
    let output_path = temp_dir
        .path()
        .join(format!("playlist_{}.json", uuid::Uuid::new_v4()));

    tracing::debug!(
        executable = ?executable_path,
        output_path = ?output_path,
        "Redirecting yt-dlp output to temporary file"
    );

    let _output = executor.execute_to_file(&output_path).await?;

    tracing::debug!(
        output_path = ?output_path,
        "Opening output file for parsing"
    );

    // Open the file using tokio::fs (async)
    let file = tokio::fs::File::open(&output_path).await?;
    let file = file.into_std().await;

    tracing::debug!("Spawning blocking task for JSON parsing");

    // Use spawn_blocking to perform CPU-intensive and blocking I/O JSON parsing
    let playlist: Playlist = tokio::task::spawn_blocking(move || {
        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader)
    })
    .await??;

    tracing::debug!(
        playlist_id = %playlist.id,
        title = %playlist.title,
        entry_count = playlist.entries.len(),
        "Playlist parsed successfully"
    );

    Ok(playlist)
}
