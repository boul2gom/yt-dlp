//! Cache backend implementations.
//!
//! This module provides different backend implementations for caching video metadata and files.
//! Each backend must implement the appropriate traits for video and file caching.

use crate::cache::video::{CachedFile, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use std::path::PathBuf;

#[cfg(feature = "cache")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

pub mod memory;
pub mod sqlite;

/// Trait for video cache backend implementations.
#[async_trait::async_trait]
pub trait VideoBackend: Send + Sync {
    /// Create a new video cache backend.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache.
    /// * `ttl` - The time-to-live for cache entries in seconds.
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self>
    where
        Self: Sized;

    /// Retrieve a video from the cache by its URL.
    async fn get(&self, url: &str) -> Result<Option<Video>>;

    /// Store a video in the cache.
    async fn put(&self, url: String, video: Video) -> Result<()>;

    /// Remove a video from the cache by its URL.
    async fn remove(&self, url: &str) -> Result<()>;

    /// Clean expired entries from the cache.
    async fn clean(&self) -> Result<()>;

    /// Retrieve a video from the cache by its ID.
    async fn get_by_id(&self, id: &str) -> Result<CachedVideo>;
}

/// Trait for file cache backend implementations.
#[async_trait::async_trait]
pub trait FileBackend: Send + Sync {
    /// Create a new file cache backend.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache.
    /// * `ttl` - The time-to-live for cache entries in seconds.
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self>
    where
        Self: Sized;

    /// Retrieve a file from the cache by its hash.
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)>;

    /// Retrieve a file from the cache by video ID and format ID.
    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)>;

    /// Retrieve a file from the cache by video ID and quality preferences.
    #[cfg(feature = "cache")]
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)>;

    /// Store a file in the cache.
    async fn put(&self, file: CachedFile, content: &[u8]) -> Result<PathBuf>;

    /// Remove a file from the cache.
    async fn remove(&self, id: &str) -> Result<()>;

    /// Clean expired entries from the cache.
    async fn clean(&self) -> Result<()>;
}
