//! Video cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching video metadata,
//! using pluggable backend implementations (SQLite by default).

use crate::error::Result;
use crate::model::Video;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "cache")]
use crate::cache::backend::{VideoBackend, sqlite::SqliteVideoCache};

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
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
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }
}

/// Structure for storing downloaded file metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
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

/// Video cache manager using SQLite backend by default.
#[derive(Debug, Clone)]
pub struct VideoCache {
    backend: SqliteVideoCache,
}

impl VideoCache {
    /// Creates a new video cache using SQLite backend.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache database.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 24 hours).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory cannot be created
    /// or the database cannot be initialized.
    pub async fn new(
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<Self> {
        let backend = SqliteVideoCache::new(cache_dir.as_ref().to_path_buf(), ttl).await?;
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
    /// Returns `Some(Video)` if the video is in the cache and has not expired,
    /// otherwise `None`.
    pub async fn get(&self, url: &str) -> Result<Option<Video>> {
        self.backend.get(url).await
    }

    /// Puts a video in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    /// * `video` - The video metadata.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    pub async fn put(&self, url: String, video: Video) -> Result<()> {
        self.backend.put(url, video).await
    }

    /// Removes a video from the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to remove.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    pub async fn remove(&self, url: &str) -> Result<()> {
        self.backend.remove(url).await
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    pub async fn clean(&self) -> Result<()> {
        self.backend.clean().await
    }

    /// Retrieves a video from the cache by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the video to retrieve.
    ///
    /// # Returns
    ///
    /// Returns the cached video if it exists and has not expired, otherwise an error.
    pub async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        self.backend.get_by_id(id).await
    }
}
