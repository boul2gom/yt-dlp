//! Cache backend trait definitions and dispatch enums.
//!
//! This module defines the backend traits (`VideoBackend`, `PlaylistBackend`, `FileBackend`)
//! and provides persistent-layer dispatch enums that delegate to the correct concrete
//! backend based on enabled features. The in-memory Moka backend is separate and used
//! as the L1 layer; the persistent enum is the L2 layer.

use std::future::Future;
use std::path::PathBuf;

use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;

#[cfg(feature = "cache-json")]
pub mod json;
#[cfg(feature = "cache-memory")]
pub mod memory;
#[cfg(feature = "cache-redb")]
pub mod redb;
#[cfg(feature = "cache-redis")]
pub mod redis;

#[cfg(feature = "cache-json")]
use json::{JsonFileCache, JsonPlaylistCache, JsonVideoCache};
#[cfg(feature = "cache-redb")]
use redb::{RedbFileCache, RedbPlaylistCache, RedbVideoCache};
#[cfg(feature = "cache-redis")]
use redis::{RedisFileCache, RedisPlaylistCache, RedisVideoCache};

/// Trait for video cache backend implementations.
pub trait VideoBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a video by its URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the backend lookup fails.
    ///
    /// # Returns
    ///
    /// The cached `Video` if found, or `None` if not present.
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Video>>> + Send;

    /// Stores a video in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to use as the cache key
    /// * `video` - The video metadata to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails.
    fn put(&self, url: String, video: Video) -> impl Future<Output = Result<()>> + Send;

    /// Removes a video from the cache by URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&self, url: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Retrieves a video by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier of the video
    ///
    /// # Errors
    ///
    /// Returns an error if the backend lookup fails.
    ///
    /// # Returns
    ///
    /// The cached video entry.
    fn get_by_id(&self, id: &str) -> impl Future<Output = Result<CachedVideo>> + Send;
}

/// Trait for playlist cache backend implementations.
pub trait PlaylistBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a playlist by its URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to retrieve
    ///
    /// # Errors
    ///
    /// Returns an error if the backend lookup fails.
    ///
    /// # Returns
    ///
    /// The cached `Playlist` if found, or `None` if not present.
    fn get(&self, url: &str) -> impl Future<Output = Result<Option<Playlist>>> + Send;

    /// Retrieves a playlist by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier of the playlist
    ///
    /// # Errors
    ///
    /// Returns an error if the backend lookup fails.
    ///
    /// # Returns
    ///
    /// The cached `Playlist` if found, or `None` if not present.
    fn get_by_id(&self, id: &str) -> impl Future<Output = Result<Option<Playlist>>> + Send;

    /// Stores a playlist in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to use as the cache key
    /// * `playlist` - The playlist to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails.
    fn put(&self, url: String, playlist: Playlist) -> impl Future<Output = Result<()>> + Send;

    /// Invalidates (removes) a playlist from the cache by URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to invalidate
    ///
    /// # Errors
    ///
    /// Returns an error if the invalidation operation fails.
    fn invalidate(&self, url: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Clears all entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the clear operation fails.
    fn clear_all(&self) -> impl Future<Output = Result<()>> + Send;
}

/// Trait for file cache backend implementations.
pub trait FileBackend: Send + Sync + std::fmt::Debug {
    /// Retrieves a file from the cache by its hash.
    ///
    /// # Arguments
    ///
    /// * `hash` - The content hash of the file
    ///
    /// # Returns
    ///
    /// The cached file entry and its path, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying I/O or deserialization fails.
    fn get_by_hash(&self, hash: &str) -> impl Future<Output = Result<Option<(CachedFile, PathBuf)>>> + Send;

    /// Retrieves a file from the cache by video ID and format ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier
    /// * `format_id` - The format identifier
    ///
    /// # Returns
    ///
    /// The cached file entry and its path, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying I/O or deserialization fails.
    fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> impl Future<Output = Result<Option<(CachedFile, PathBuf)>>> + Send;

    /// Retrieves a file from the cache based on video ID and quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier
    /// * `preferences` - The format preferences to match against
    ///
    /// # Returns
    ///
    /// The cached file entry and its path, or `None` if no match.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying I/O or deserialization fails.
    fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> impl Future<Output = Result<Option<(CachedFile, PathBuf)>>> + Send;

    /// Store a file in the cache.
    ///
    /// # Arguments
    ///
    /// * `file` - The cached file metadata
    /// * `source_path` - Path to the source file to store
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be stored.
    ///
    /// # Returns
    ///
    /// The path where the file was cached.
    fn put(&self, file: CachedFile, source_path: &std::path::Path) -> impl Future<Output = Result<PathBuf>> + Send;

    /// Removes a file from the cache by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier of the cached file
    ///
    /// # Errors
    ///
    /// Returns an error if the removal operation fails.
    fn remove(&self, id: &str) -> impl Future<Output = Result<()>> + Send;

    /// Cleans expired entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cleanup operation fails.
    fn clean(&self) -> impl Future<Output = Result<()>> + Send;

    /// Retrieve a thumbnail from the cache by video ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier
    ///
    /// # Returns
    ///
    /// The cached thumbnail entry and its path, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying I/O or deserialization fails.
    fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> impl Future<Output = Result<Option<(CachedThumbnail, PathBuf)>>> + Send;

    /// Store a thumbnail in the cache.
    ///
    /// # Arguments
    ///
    /// * `thumbnail` - The cached thumbnail metadata
    /// * `source_path` - Path to the source thumbnail file
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be stored.
    ///
    /// # Returns
    ///
    /// The path where the thumbnail was cached.
    fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &std::path::Path,
    ) -> impl Future<Output = Result<PathBuf>> + Send;

    /// Retrieve a subtitle from the cache by video ID and language.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video identifier
    /// * `language` - The subtitle language code
    ///
    /// # Returns
    ///
    /// The cached subtitle file entry and its path, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying I/O or deserialization fails.
    fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> impl Future<Output = Result<Option<(CachedFile, PathBuf)>>> + Send;
}

// ── Persistent backend dispatch enums ──

/// Enum dispatch for persistent video backends.
///
/// Exactly one variant is compiled, determined by the enabled persistent feature.
/// The compile_error in `cache/mod.rs` ensures at most one persistent backend.
#[cfg(persistent_cache)]
#[derive(Debug)]
pub enum PersistentVideoBackend {
    #[cfg(feature = "cache-json")]
    Json(JsonVideoCache),
    #[cfg(feature = "cache-redb")]
    Redb(RedbVideoCache),
    #[cfg(feature = "cache-redis")]
    Redis(RedisVideoCache),
}

/// Enum dispatch for persistent playlist backends.
#[cfg(persistent_cache)]
#[derive(Debug)]
pub enum PersistentPlaylistBackend {
    #[cfg(feature = "cache-json")]
    Json(JsonPlaylistCache),
    #[cfg(feature = "cache-redb")]
    Redb(RedbPlaylistCache),
    #[cfg(feature = "cache-redis")]
    Redis(RedisPlaylistCache),
}

/// Enum dispatch for persistent file backends.
#[cfg(persistent_cache)]
#[derive(Debug)]
pub enum PersistentFileBackend {
    #[cfg(feature = "cache-json")]
    Json(JsonFileCache),
    #[cfg(feature = "cache-redb")]
    Redb(RedbFileCache),
    #[cfg(feature = "cache-redis")]
    Redis(RedisFileCache),
}

// ── Persistent video backend constructors & dispatch ──

#[cfg(persistent_cache)]
impl PersistentVideoBackend {
    /// Creates the persistent video backend based on the enabled feature.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for file-based backends
    /// * `redis_url` - Connection URL for Redis backend
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns an error if backend initialization fails.
    pub async fn new(
        cache_dir: PathBuf,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
        ttl: Option<u64>,
    ) -> Result<Self> {
        #[cfg(feature = "cache-json")]
        {
            Ok(Self::Json(JsonVideoCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redb")]
        {
            Ok(Self::Redb(RedbVideoCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redis")]
        {
            let _ = cache_dir;
            let url = redis_url.unwrap_or("redis://127.0.0.1/");
            Ok(Self::Redis(RedisVideoCache::new(url, ttl).await?))
        }
    }
}

#[cfg(persistent_cache)]
impl VideoBackend for PersistentVideoBackend {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get(url).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get(url).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get(url).await,
        }
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.put(url, video).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.put(url, video).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.put(url, video).await,
        }
    }

    async fn remove(&self, url: &str) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.remove(url).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.remove(url).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.remove(url).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.clean().await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.clean().await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.clean().await,
        }
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_by_id(id).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_by_id(id).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_by_id(id).await,
        }
    }
}

// ── Persistent playlist backend constructors & dispatch ──

#[cfg(persistent_cache)]
impl PersistentPlaylistBackend {
    /// Creates the persistent playlist backend based on the enabled feature.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for file-based backends
    /// * `redis_url` - Connection URL for Redis backend
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns an error if backend initialization fails.
    pub async fn new(
        cache_dir: PathBuf,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
        ttl: Option<u64>,
    ) -> Result<Self> {
        #[cfg(feature = "cache-json")]
        {
            Ok(Self::Json(JsonPlaylistCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redb")]
        {
            Ok(Self::Redb(RedbPlaylistCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redis")]
        {
            let _ = cache_dir;
            let url = redis_url.unwrap_or("redis://127.0.0.1/");
            Ok(Self::Redis(RedisPlaylistCache::new(url, ttl).await?))
        }
    }
}

#[cfg(persistent_cache)]
impl PlaylistBackend for PersistentPlaylistBackend {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get(url).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get(url).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get(url).await,
        }
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_by_id(id).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_by_id(id).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_by_id(id).await,
        }
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.put(url, playlist).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.put(url, playlist).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.put(url, playlist).await,
        }
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.invalidate(url).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.invalidate(url).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.invalidate(url).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.clean().await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.clean().await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.clean().await,
        }
    }

    async fn clear_all(&self) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.clear_all().await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.clear_all().await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.clear_all().await,
        }
    }
}

// ── Persistent file backend constructors & dispatch ──

#[cfg(persistent_cache)]
impl PersistentFileBackend {
    /// Creates the persistent file backend based on the enabled feature.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory for file-based backends
    /// * `redis_url` - Connection URL for Redis backend
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns an error if backend initialization fails.
    pub async fn new(
        cache_dir: PathBuf,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
        ttl: Option<u64>,
    ) -> Result<Self> {
        #[cfg(feature = "cache-json")]
        {
            Ok(Self::Json(JsonFileCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redb")]
        {
            Ok(Self::Redb(RedbFileCache::new(cache_dir, ttl).await?))
        }
        #[cfg(feature = "cache-redis")]
        {
            let url = redis_url.unwrap_or("redis://127.0.0.1/");
            Ok(Self::Redis(RedisFileCache::new(url, cache_dir, ttl).await?))
        }
    }
}

#[cfg(persistent_cache)]
impl FileBackend for PersistentFileBackend {
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_by_hash(hash).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_by_hash(hash).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_by_hash(hash).await,
        }
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_by_video_and_format(video_id, format_id).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_by_video_and_format(video_id, format_id).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_by_video_and_format(video_id, format_id).await,
        }
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_by_video_and_preferences(video_id, preferences).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_by_video_and_preferences(video_id, preferences).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_by_video_and_preferences(video_id, preferences).await,
        }
    }

    async fn put(&self, file: CachedFile, source_path: &std::path::Path) -> Result<PathBuf> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.put(file, source_path).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.put(file, source_path).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.put(file, source_path).await,
        }
    }

    async fn remove(&self, id: &str) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.remove(id).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.remove(id).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.remove(id).await,
        }
    }

    async fn clean(&self) -> Result<()> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.clean().await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.clean().await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.clean().await,
        }
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_thumbnail_by_video_id(video_id).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_thumbnail_by_video_id(video_id).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_thumbnail_by_video_id(video_id).await,
        }
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &std::path::Path) -> Result<PathBuf> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.put_thumbnail(thumbnail, source_path).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.put_thumbnail(thumbnail, source_path).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.put_thumbnail(thumbnail, source_path).await,
        }
    }

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.get_subtitle_by_language(video_id, language).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.get_subtitle_by_language(video_id, language).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.get_subtitle_by_language(video_id, language).await,
        }
    }
}
