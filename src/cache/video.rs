//! Video cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching video metadata,
//! using pluggable backend implementations.

use crate::cache::backend::{VideoBackend, VideoBackendEnum};
use crate::cache::current_timestamp;
use crate::error::Result;
use crate::model::Video;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "cache-sqlite", derive(sqlx::FromRow))]
pub struct CachedVideo {
    /// The ID of the video.
    pub id: String,
    /// The title of the video.
    pub title: String,
    /// The URL of the video.
    pub url: String,
    /// The complete video metadata as JSON.
    pub video_json: String,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: i64,
}

impl CachedVideo {
    /// Deserializes the cached video JSON into a Video struct.
    ///
    /// # Returns
    ///
    /// The deserialized `Video` object.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON deserialization fails.
    pub fn video(&self) -> Result<Video> {
        serde_json::from_str(&self.video_json)
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to parse video: {}", e)))
    }
}

impl From<(String, Video)> for CachedVideo {
    fn from((url, video): (String, Video)) -> Self {
        let video_json = serde_json::to_string(&video).unwrap_or_default();

        Self {
            id: video.id.clone(),
            title: video.title.clone(),
            url,
            video_json,
            cached_at: current_timestamp(),
        }
    }
}

/// Structure for storing downloaded file metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "cache-sqlite", derive(sqlx::FromRow))]
pub struct CachedFile {
    /// The ID of the file (SHA-256 hash of the content).
    pub id: String,
    /// The original filename.
    pub filename: String,
    /// The path to the file relative to the cache directory.
    pub relative_path: String,
    /// The video ID this file is associated with (if any).
    pub video_id: Option<String>,
    /// The file type (format, thumbnail, etc.)
    pub file_type: String,
    /// The format ID this file is associated with (if any).
    pub format_id: Option<String>,
    /// The format information serialized as JSON (if available).
    pub format_json: Option<String>,
    /// The video quality preference used to select this format (if any).
    pub video_quality: Option<String>,
    /// The audio quality preference used to select this format (if any).
    pub audio_quality: Option<String>,
    /// The video codec preference used to select this format (if any).
    pub video_codec: Option<String>,
    /// The audio codec preference used to select this format (if any).
    pub audio_codec: Option<String>,
    /// The language code for subtitle files (if any).
    pub language_code: Option<String>,
    /// The file size in bytes.
    pub filesize: i64,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: i64,
}

/// Enum representing the type of cached file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CachedType {
    /// A video or audio format
    Format,
    /// A thumbnail image
    Thumbnail,
    /// A subtitle file
    Subtitle,
    /// Any other type of file
    Other,
}

/// Structure for storing thumbnail metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "cache-sqlite", derive(sqlx::FromRow))]
pub struct CachedThumbnail {
    /// The ID of the thumbnail (SHA-256 hash of the content).
    pub id: String,
    /// The original filename.
    pub filename: String,
    /// The path to the file relative to the cache directory.
    pub relative_path: String,
    /// The video ID this thumbnail is associated with.
    pub video_id: String,
    /// The file size in bytes.
    pub filesize: i64,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The width of the thumbnail in pixels (if available).
    pub width: Option<i32>,
    /// The height of the thumbnail in pixels (if available).
    pub height: Option<i32>,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: i64,
}

/// Video cache manager using pluggable backend.
#[derive(Debug)]
pub struct VideoCache {
    backend: VideoBackendEnum,
}

impl VideoCache {
    /// Creates a new video cache with the specified directory and TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    /// * `ttl` - Time-to-live for cache entries in seconds (optional).
    ///
    /// # Returns
    ///
    /// A new `VideoCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails or no backend is enabled.
    pub async fn new(cache_dir: impl Into<PathBuf>, ttl: Option<u64>) -> Result<Self> {
        let cache_dir = cache_dir.into();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "Creating video cache"
        );

        let backend = VideoBackendEnum::new(cache_dir, ttl).await?;
        Ok(Self { backend })
    }

    /// Retrieves a video from the cache by its URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to retrieve.
    ///
    /// # Returns
    ///
    /// `Some(Video)` if found and not expired, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend query fails.
    pub async fn get(&self, url: &str) -> Result<Option<Video>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Retrieving video from cache by URL");

        let result = self.backend.get(url).await;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = url,
            found = result.as_ref().map(|r| r.is_some()).unwrap_or(false),
            "Video cache lookup by URL completed"
        );

        result
    }

    /// Puts a video in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    /// * `video` - The video metadata to cache.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend put operation fails.
    pub async fn put(&self, url: String, video: Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = %url,
            video_id = %video.id,
            video_title = %video.title,
            "Putting video in cache"
        );

        let result = self.backend.put(url.clone(), video).await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!(url = %url, "Successfully cached video");
        } else {
            tracing::debug!(url = %url, "Failed to cache video");
        }

        result
    }

    /// Removes a video from the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to remove.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend remove operation fails.
    pub async fn remove(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Removing video from cache");

        let result = self.backend.remove(url).await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!(url = url, "Successfully removed video from cache");
        } else {
            tracing::debug!(url = url, "Failed to remove video from cache");
        }

        result
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clean operation fails.
    pub async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning video cache");

        let result = self.backend.clean().await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!("Successfully cleaned video cache");
        } else {
            tracing::debug!("Failed to clean video cache");
        }

        result
    }

    /// Retrieves a video from the cache by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The video ID to search for.
    ///
    /// # Returns
    ///
    /// The cached video metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the video is not found, expired, or the backend query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        #[cfg(feature = "tracing")]
        tracing::debug!(video_id = id, "Retrieving video from cache by ID");

        let result = self.backend.get_by_id(id).await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!(video_id = id, "Found video in cache by ID");
        } else {
            tracing::debug!(video_id = id, "Video not found in cache by ID");
        }

        result
    }
}
