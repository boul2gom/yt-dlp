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
pub trait VideoExtractor: Downcast + Send + Sync + std::fmt::Debug {
    /// Fetch video metadata from a URL.
    ///
    /// # Arguments
    /// * `url` - The video URL to fetch
    ///
    /// # Returns
    /// Video metadata including formats, title, duration, etc.
    ///
    /// # Errors
    /// Returns error if the URL is unsupported, geo-blocked, or requires authentication
    async fn fetch_video(&self, url: &str) -> Result<Video>;

    /// Fetch playlist metadata from a URL.
    ///
    /// # Arguments
    /// * `url` - The playlist URL to fetch
    ///
    /// # Returns
    /// Playlist metadata including entries and metadata
    ///
    /// # Errors
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

pub mod detector;
pub mod generic;
pub mod youtube;

pub use detector::detect_extractor_type;
pub use generic::Generic;
pub use youtube::Youtube;

use crate::executor::Executor;
use std::path::PathBuf;
use std::time::Duration;

/// Helper to execute the extractor command and parse the output as a Video.
///
/// This handles the common pattern of:
/// 1. Creating an Executor
/// 2. Running it
/// 3. Deserializing the JSON output
/// 4. Post-processing the video (e.g. setting video_id on formats)
pub async fn execute_and_parse_video(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<Video> {
    let executor = Executor::new(executable_path, args.to_vec(), timeout);

    let output = executor.execute().await?;
    let mut video: Video = serde_json::from_str(&output.stdout)?;

    // Set video ID on each format for caching purposes
    for format in &mut video.formats {
        format.video_id = Some(video.id.clone());
    }

    Ok(video)
}

/// Helper to execute the extractor command and parse the output as a Playlist.
pub async fn execute_and_parse_playlist(
    executable_path: PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<Playlist> {
    let executor = Executor::new(executable_path, args.to_vec(), timeout);

    let output = executor.execute().await?;
    Ok(serde_json::from_str(&output.stdout)?)
}
