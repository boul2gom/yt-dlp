//! Video cache data types and tiered wrapper.
//!
//! Provides `CachedVideo`, `CachedFile`, `CachedThumbnail` data structures and the
//! `VideoCache` wrapper that orchestrates L1 (Moka) and L2 (persistent) lookups.

use serde::{Deserialize, Serialize};

use crate::cache::FormatPreferences;
#[cfg(persistent_cache)]
use crate::cache::backend::PersistentVideoBackend;
use crate::cache::backend::VideoBackend;
#[cfg(feature = "cache-memory")]
use crate::cache::backend::memory::MokaVideoCache;
use crate::cache::config::CacheConfig;
use crate::error::Result;
use crate::model::{Video, utils};
use crate::utils::current_timestamp;

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Creates a new `CachedVideo` by serializing the given video.
    ///
    /// # Arguments
    ///
    /// * `url` - The original URL of the video.
    /// * `video` - The video metadata to cache.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization fails.
    ///
    /// # Returns
    ///
    /// A fully initialized `CachedVideo`.
    pub fn new(url: String, video: &Video) -> Result<Self> {
        let video_json = serde_json::to_string(video)?;
        Ok(Self {
            id: video.id.clone(),
            title: video.title.clone(),
            url,
            video_json,
            cached_at: current_timestamp(),
        })
    }

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
        Ok(serde_json::from_str(&self.video_json)?)
    }
}

impl std::fmt::Display for CachedVideo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CachedVideo(id={}, title={})", self.id, self.title)
    }
}

/// Structure for storing downloaded file metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl CachedFile {
    /// Checks if this cached file matches the given preferences.
    pub fn matches_preferences(&self, preferences: &FormatPreferences) -> bool {
        if preferences.video_quality.is_some()
            && self.video_quality != utils::serde::serialize_json_opt(preferences.video_quality)
        {
            return false;
        }

        if preferences.audio_quality.is_some()
            && self.audio_quality != utils::serde::serialize_json_opt(preferences.audio_quality)
        {
            return false;
        }

        if preferences.video_codec.is_some()
            && self.video_codec != utils::serde::serialize_json_opt(preferences.video_codec.clone())
        {
            return false;
        }

        if preferences.audio_codec.is_some()
            && self.audio_codec != utils::serde::serialize_json_opt(preferences.audio_codec.clone())
        {
            return false;
        }

        true
    }
}

impl std::fmt::Display for CachedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CachedFile(id={}, filename={}, size={})",
            self.id, self.filename, self.filesize
        )
    }
}

/// Enum representing the type of cached file
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for CachedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format => f.write_str("Format"),
            Self::Thumbnail => f.write_str("Thumbnail"),
            Self::Subtitle => f.write_str("Subtitle"),
            Self::Other => f.write_str("Other"),
        }
    }
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

impl std::fmt::Display for CachedThumbnail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CachedThumbnail(id={}, video_id={})", self.id, self.video_id)
    }
}

/// Video cache manager with tiered L1 (Moka) + L2 (persistent) lookup.
///
/// On `get`: L1 → miss → L2 → backfill L1.
/// On `put`: write to both layers.
#[derive(Debug)]
pub struct VideoCache {
    #[cfg(feature = "cache-memory")]
    memory: MokaVideoCache,
    #[cfg(persistent_cache)]
    persistent: PersistentVideoBackend,
}

impl VideoCache {
    /// Creates a new video cache with the configured layers.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    /// * `ttl` - Time-to-live for cache entries in seconds (optional).
    ///
    /// # Returns
    ///
    /// A new `VideoCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if backend initialization fails or the backend is ambiguous.
    pub async fn new(config: &CacheConfig, ttl: Option<u64>) -> Result<Self> {
        tracing::debug!(cache_dir = ?config.cache_dir, ttl = ?ttl, "⚙️ Creating video cache");

        Ok(Self {
            #[cfg(feature = "cache-memory")]
            memory: MokaVideoCache::new(config.cache_dir.clone(), ttl).await?,
            #[cfg(persistent_cache)]
            persistent: PersistentVideoBackend::new(config, ttl).await?,
        })
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
        tracing::debug!(url = url, "🔍 Looking up video by URL");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(video) = self.memory.get(url).await? {
            tracing::debug!(url = url, "✅ Video cache hit (L1 memory)");
            return Ok(Some(video));
        }

        // L2: persistent
        #[cfg(persistent_cache)]
        if let Some(video) = self.persistent.get(url).await? {
            tracing::debug!(url = url, "✅ Video cache hit (L2 persistent)");

            // Backfill L1
            #[cfg(feature = "cache-memory")]
            let _ = self.memory.put(url.to_string(), video.clone()).await;

            return Ok(Some(video));
        }

        Ok(None)
    }

    /// Puts a video in the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    /// * `video` - The video metadata to cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend put operation fails.
    pub async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(url = url, video_id = video.id, "⚙️ Storing video in cache");

        #[cfg(feature = "cache-memory")]
        self.memory.put(url.clone(), video.clone()).await?;

        #[cfg(persistent_cache)]
        self.persistent.put(url, video).await?;

        Ok(())
    }

    /// Removes a video from the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to remove.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend remove operation fails.
    pub async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Removing video from cache");

        #[cfg(feature = "cache-memory")]
        self.memory.remove(url).await?;

        #[cfg(persistent_cache)]
        self.persistent.remove(url).await?;

        Ok(())
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clean operation fails.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("⚙️ Cleaning video cache");

        #[cfg(feature = "cache-memory")]
        self.memory.clean().await?;

        #[cfg(persistent_cache)]
        self.persistent.clean().await?;

        Ok(())
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
    /// Returns an error if the video is not found or the backend query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, "🔍 Looking up video by ID");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Ok(cached) = self.memory.get_by_id(id).await {
            tracing::debug!(video_id = id, "✅ Video cache hit by ID (L1 memory)");
            return Ok(cached);
        }

        // L2: persistent
        #[cfg(persistent_cache)]
        let result = {
            let cached = self.persistent.get_by_id(id).await?;
            tracing::debug!(video_id = id, "✅ Video cache hit by ID (L2 persistent)");

            // Backfill L1
            #[cfg(feature = "cache-memory")]
            if let Ok(video) = cached.video() {
                let _ = self.memory.put(cached.url.clone(), video).await;
            }

            Ok(cached)
        };
        #[cfg(not(persistent_cache))]
        let result = Err(crate::error::Error::cache_miss(format!("video:{}", id)));

        result
    }
}
