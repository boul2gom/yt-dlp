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

/// Core trait for video extractors.
///
/// This trait defines the common interface that all extractors must implement.
/// Each extractor handles fetching video metadata and playlists from their respective platforms.
#[async_trait]
pub trait VideoExtractor: Send + Sync + std::fmt::Debug {
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
    fn name(&self) -> &str;

    /// Check if this extractor supports the given URL pattern.
    ///
    /// This is a fast, synchronous check based on URL patterns.
    /// Use `fetch_video()` for definitive validation.
    fn supports_url(&self, url: &str) -> bool;
}

pub mod detector;
pub mod generic;
pub mod youtube;

pub use detector::{ExtractorType, detect_extractor_type};
pub use generic::Generic;
pub use youtube::Youtube;
