//! Cache backend implementations.
//!
//! This module provides different backend implementations for caching video metadata and files.
//! Each backend must implement the appropriate traits for video and file caching.

use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use std::path::PathBuf;

#[cfg(feature = "cache")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

#[cfg(feature = "cache-json")]
pub mod json;
pub mod memory;
#[cfg(feature = "cache-sqlite")]
pub mod sqlite;

use crate::model::playlist::Playlist;

/// Trait for playlist cache backend implementations.
#[async_trait::async_trait]
pub trait PlaylistBackend: Send + Sync + std::fmt::Debug {
    /// Creates a new instance of the backend.
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self>
    where
        Self: Sized;

    /// Retrieves a playlist by its URL.
    async fn get(&self, url: &str) -> Result<Option<Playlist>>;

    /// Retrieves a playlist by its ID.
    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>>;

    /// Stores a playlist in the cache.
    async fn put(&self, url: String, playlist: Playlist) -> Result<()>;

    /// Invalidates (removes) a playlist from the cache by URL.
    async fn invalidate(&self, url: &str) -> Result<()>;

    /// Cleans expired entries from the cache.
    async fn clean(&self) -> Result<()>;

    /// Clears all entries from the cache.
    async fn clear_all(&self) -> Result<()>;
}

#[async_trait::async_trait]
pub trait VideoBackend: Send + Sync + std::fmt::Debug {
    /// Creates a new instance of the backend.
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self>
    where
        Self: Sized;

    /// Retrieves a video by its URL.
    async fn get(&self, url: &str) -> Result<Option<Video>>;

    /// Stores a video in the cache.
    async fn put(&self, url: String, video: Video) -> Result<()>;

    /// Removes a video from the cache by URL.
    async fn remove(&self, url: &str) -> Result<()>;

    /// Cleans expired entries from the cache.
    async fn clean(&self) -> Result<()>;

    /// Retrieves a video by its ID.
    async fn get_by_id(&self, id: &str) -> Result<CachedVideo>;
}

/// Trait for file cache backend implementations.
#[async_trait::async_trait]
pub trait FileBackend: Send + Sync + std::fmt::Debug {
    /// Creates a new instance of the backend.
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self>
    where
        Self: Sized;

    /// Retrieves a file from the cache by its hash.
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)>;

    /// Retrieves a file from the cache by video ID and format ID.
    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)>;

    /// Retrieves a file from the cache based on video ID and quality preferences.
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)>;

    /// Store a file in the cache.
    async fn put(&self, file: CachedFile, source_path: &std::path::Path) -> Result<PathBuf>;

    /// Removes a file from the cache by its ID.
    async fn remove(&self, id: &str) -> Result<()>;

    /// Cleans expired entries from the cache.
    async fn clean(&self) -> Result<()>;

    /// Retrieve a thumbnail from the cache by video ID.
    async fn get_thumbnail_by_video_id(&self, video_id: &str)
    -> Option<(CachedThumbnail, PathBuf)>;

    /// Store a thumbnail in the cache.
    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &std::path::Path,
    ) -> Result<PathBuf>;

    /// Retrieve a subtitle from the cache by video ID and language.
    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)>;
}
