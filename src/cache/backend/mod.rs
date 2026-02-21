//! Cache backend implementations.
//!
//! This module provides different backend implementations for caching video metadata and files.
//! Each backend must implement the appropriate traits for video and file caching.

use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use std::path::PathBuf;

use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

#[cfg(cache_backend = "json")]
pub mod json;
#[cfg(cache_backend = "memory")]
pub mod memory;
#[cfg(cache_backend = "sqlite")]
pub mod sqlite;

use crate::model::playlist::Playlist;

// Re-export concrete types used by enum variants
#[cfg(cache_backend = "json")]
use json::{JsonFileCache, JsonPlaylistCache, JsonVideoCache};
#[cfg(cache_backend = "memory")]
use memory::{MemoryFileCache, MemoryPlaylistCache, MemoryVideoCache};
#[cfg(cache_backend = "sqlite")]
use sqlite::{SqliteFileCache, SqlitePlaylistCache, SqliteVideoCache};

/// Trait for playlist cache backend implementations.
pub trait PlaylistBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a playlist by its URL.
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Playlist>>> + Send;

    /// Retrieves a playlist by its ID.
    fn get_by_id(&self, id: &str) -> impl Future<Output = Result<Option<Playlist>>> + Send;

    /// Stores a playlist in the cache.
    fn put(&self, url: String, playlist: Playlist) -> impl Future<Output = Result<()>> + Send;

    /// Invalidates (removes) a playlist from the cache by URL.
    fn invalidate(&self, url: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Clears all entries from the cache.
    fn clear_all(&self) -> impl Future<Output = Result<()>> + Send;
}

/// Trait for video cache backend implementations.
pub trait VideoBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a video by its URL.
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Video>>> + Send;

    /// Stores a video in the cache.
    fn put(&self, url: String, video: Video) -> impl Future<Output = Result<()>> + Send;

    /// Removes a video from the cache by URL.
    fn remove(&self, url: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Retrieves a video by its ID.
    fn get_by_id(&self, id: &str) -> impl Future<Output = Result<CachedVideo>> + Send;
}

/// Trait for file cache backend implementations.
pub trait FileBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a file from the cache by its hash.
    fn get_by_hash(&self, hash: &str)
    -> impl Future<Output = Option<(CachedFile, PathBuf)>> + Send;

    /// Retrieves a file from the cache by video ID and format ID.
    fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> impl Future<Output = Option<(CachedFile, PathBuf)>> + Send;

    /// Retrieves a file from the cache based on video ID and quality preferences.
    fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> impl Future<Output = Option<(CachedFile, PathBuf)>> + Send;

    /// Store a file in the cache.
    fn put(
        &self,
        file: CachedFile,
        source_path: &std::path::Path,
    ) -> impl Future<Output = Result<PathBuf>> + Send;

    /// Removes a file from the cache by its ID.
    fn remove(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Retrieve a thumbnail from the cache by video ID.
    fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> impl Future<Output = Option<(CachedThumbnail, PathBuf)>> + Send;

    /// Store a thumbnail in the cache.
    fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &std::path::Path,
    ) -> impl Future<Output = Result<PathBuf>> + Send;

    /// Retrieve a subtitle from the cache by video ID and language.
    fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> impl Future<Output = Option<(CachedFile, PathBuf)>> + Send;
}

/// Zero-cost enum dispatch for `VideoBackend` implementations.
///
/// `build.rs` guarantees exactly one variant is compiled per build, regardless
/// of how many cache feature flags are active simultaneously.
#[derive(Debug)]
pub enum VideoBackendEnum {
    #[cfg(cache_backend = "sqlite")]
    Sqlite(SqliteVideoCache),
    #[cfg(cache_backend = "json")]
    Json(JsonVideoCache),
    #[cfg(cache_backend = "memory")]
    Memory(MemoryVideoCache),
}

/// Zero-cost enum dispatch for `FileBackend` implementations.
///
/// `build.rs` guarantees exactly one variant is compiled per build, regardless
/// of how many cache feature flags are active simultaneously.
#[derive(Debug)]
pub enum FileBackendEnum {
    #[cfg(cache_backend = "sqlite")]
    Sqlite(SqliteFileCache),
    #[cfg(cache_backend = "json")]
    Json(JsonFileCache),
    #[cfg(cache_backend = "memory")]
    Memory(MemoryFileCache),
}

/// Zero-cost enum dispatch for `PlaylistBackend` implementations.
///
/// `build.rs` guarantees exactly one variant is compiled per build, regardless
/// of how many cache feature flags are active simultaneously.
#[derive(Debug)]
pub enum PlaylistBackendEnum {
    #[cfg(cache_backend = "sqlite")]
    Sqlite(SqlitePlaylistCache),
    #[cfg(cache_backend = "json")]
    Json(JsonPlaylistCache),
    #[cfg(cache_backend = "memory")]
    Memory(MemoryPlaylistCache),
}

impl VideoBackend for VideoBackendEnum {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get(url).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get(url).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get(url).await,
        }
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.put(url, video).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.put(url, video).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.put(url, video).await,
        }
    }

    async fn remove(&self, url: &str) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.remove(url).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.remove(url).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.remove(url).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.clean().await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.clean().await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.clean().await,
        }
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_by_id(id).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_by_id(id).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_by_id(id).await,
        }
    }
}

impl FileBackend for FileBackendEnum {
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_by_hash(hash).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_by_hash(hash).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_by_hash(hash).await,
        }
    }

    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_by_video_and_format(video_id, format_id).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_by_video_and_format(video_id, format_id).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_by_video_and_format(video_id, format_id).await,
        }
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => {
                b.get_by_video_and_preferences(
                    video_id,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
            }
            #[cfg(cache_backend = "json")]
            Self::Json(b) => {
                b.get_by_video_and_preferences(
                    video_id,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
            }
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => {
                b.get_by_video_and_preferences(
                    video_id,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
            }
        }
    }

    async fn put(&self, file: CachedFile, source_path: &std::path::Path) -> Result<PathBuf> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.put(file, source_path).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.put(file, source_path).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.put(file, source_path).await,
        }
    }

    async fn remove(&self, id: &str) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.remove(id).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.remove(id).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.remove(id).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.clean().await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.clean().await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.clean().await,
        }
    }

    async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_thumbnail_by_video_id(video_id).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_thumbnail_by_video_id(video_id).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_thumbnail_by_video_id(video_id).await,
        }
    }

    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &std::path::Path,
    ) -> Result<PathBuf> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.put_thumbnail(thumbnail, source_path).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.put_thumbnail(thumbnail, source_path).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.put_thumbnail(thumbnail, source_path).await,
        }
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_subtitle_by_language(video_id, language).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_subtitle_by_language(video_id, language).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_subtitle_by_language(video_id, language).await,
        }
    }
}

impl VideoBackendEnum {
    /// Creates the appropriate video backend based on enabled features.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        #[cfg(cache_backend = "sqlite")]
        {
            Ok(Self::Sqlite(SqliteVideoCache::new(cache_dir, ttl).await?))
        }
        #[cfg(cache_backend = "json")]
        {
            Ok(Self::Json(JsonVideoCache::new(cache_dir, ttl).await?))
        }
        #[cfg(cache_backend = "memory")]
        {
            Ok(Self::Memory(MemoryVideoCache::new(cache_dir, ttl).await?))
        }
    }
}

impl FileBackendEnum {
    /// Creates the appropriate file backend based on enabled features.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        #[cfg(cache_backend = "sqlite")]
        {
            Ok(Self::Sqlite(SqliteFileCache::new(cache_dir, ttl).await?))
        }
        #[cfg(cache_backend = "json")]
        {
            Ok(Self::Json(JsonFileCache::new(cache_dir, ttl).await?))
        }
        #[cfg(cache_backend = "memory")]
        {
            Ok(Self::Memory(MemoryFileCache::new(cache_dir, ttl).await?))
        }
    }
}

impl PlaylistBackendEnum {
    /// Creates the appropriate playlist backend based on enabled features.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        #[cfg(cache_backend = "sqlite")]
        {
            Ok(Self::Sqlite(
                SqlitePlaylistCache::new(cache_dir, ttl).await?,
            ))
        }
        #[cfg(cache_backend = "json")]
        {
            Ok(Self::Json(JsonPlaylistCache::new(cache_dir, ttl).await?))
        }
        #[cfg(cache_backend = "memory")]
        {
            Ok(Self::Memory(
                MemoryPlaylistCache::new(cache_dir, ttl).await?,
            ))
        }
    }
}

impl PlaylistBackend for PlaylistBackendEnum {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get(url).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get(url).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get(url).await,
        }
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.get_by_id(id).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.get_by_id(id).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.get_by_id(id).await,
        }
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.put(url, playlist).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.put(url, playlist).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.put(url, playlist).await,
        }
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.invalidate(url).await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.invalidate(url).await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.invalidate(url).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.clean().await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.clean().await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.clean().await,
        }
    }

    async fn clear_all(&self) -> Result<()> {
        match self {
            #[cfg(cache_backend = "sqlite")]
            Self::Sqlite(b) => b.clear_all().await,
            #[cfg(cache_backend = "json")]
            Self::Json(b) => b.clear_all().await,
            #[cfg(cache_backend = "memory")]
            Self::Memory(b) => b.clear_all().await,
        }
    }
}
