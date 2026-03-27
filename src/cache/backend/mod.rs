//! Cache backend trait definitions and dispatch enums.
//!
//! This module defines the backend traits (`VideoBackend`, `PlaylistBackend`, `FileBackend`)
//! and provides persistent-layer dispatch enums that delegate to the correct concrete
//! backend based on enabled features. The in-memory Moka backend is separate and used
//! as the L1 layer; the persistent enum is the L2 layer.

use std::future::Future;
#[cfg(persistent_cache)]
use std::path::Path;
use std::path::PathBuf;

#[cfg(persistent_cache)]
use crate::cache::config::{CacheConfig, PersistentBackendKind};
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

// ── Shared constants ──

/// Default time-to-live for cached videos (24 hours).
pub(crate) const DEFAULT_VIDEO_TTL: u64 = 24 * 60 * 60;
/// Default time-to-live for cached playlists (6 hours).
pub(crate) const DEFAULT_PLAYLIST_TTL: u64 = 6 * 60 * 60;
/// Default time-to-live for cached files (7 days).
pub(crate) const DEFAULT_FILE_TTL: u64 = 7 * 24 * 60 * 60;

// ── Shared helpers ──

/// Compute a stable FNV-1a 64-bit hex hash of a URL.
///
/// Uses a manual implementation for cross-version stability
/// (unlike `DefaultHasher`, which can change between Rust releases).
#[cfg(persistent_cache)]
pub(crate) fn url_hash(url: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut hash = FNV_OFFSET;
    for byte in url.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{:016x}", hash)
}

/// Copy a source file into the cache directory, creating parent directories as needed.
///
/// Returns the destination path (`cache_dir` joined with `relative_path`).
#[cfg(persistent_cache)]
pub(crate) async fn copy_to_cache(cache_dir: &Path, relative_path: &str, source_path: &Path) -> Result<PathBuf> {
    let dest_path = cache_dir.join(relative_path);
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::copy(source_path, &dest_path).await?;
    Ok(dest_path)
}

/// Delegates a method call to the active backend variant.
///
/// Expands to a `match self` block that forwards the call to whichever
/// concrete backend is selected at runtime, respecting feature gates.
#[cfg(persistent_cache)]
macro_rules! delegate_to_backend {
    ($self:ident . $method:ident ( $($arg:expr),* )) => {
        match $self {
            #[cfg(feature = "cache-json")]
            Self::Json(b) => b.$method($($arg),*).await,
            #[cfg(feature = "cache-redb")]
            Self::Redb(b) => b.$method($($arg),*).await,
            #[cfg(feature = "cache-redis")]
            Self::Redis(b) => b.$method($($arg),*).await,
        }
    };
}

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
/// All features' variants are included when their respective feature is enabled.
/// The active backend is selected at construction time via `PersistentBackendKind::resolve`.
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
    /// Creates the persistent video backend for the selected kind.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns `Error::AmbiguousCacheBackend` if `kind` is `None` and multiple backends are compiled in.
    /// Returns an error if the selected backend fails to initialize.
    pub async fn new(config: &CacheConfig, ttl: Option<u64>) -> Result<Self> {
        match PersistentBackendKind::resolve(config.persistent_backend)? {
            #[cfg(feature = "cache-json")]
            PersistentBackendKind::Json => Ok(Self::Json(JsonVideoCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redb")]
            PersistentBackendKind::Redb => Ok(Self::Redb(RedbVideoCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redis")]
            PersistentBackendKind::Redis => {
                let url = config.redis_url.as_deref().unwrap_or("redis://127.0.0.1/");
                Ok(Self::Redis(RedisVideoCache::new(url, ttl).await?))
            }
        }
    }
}

#[cfg(persistent_cache)]
impl VideoBackend for PersistentVideoBackend {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        delegate_to_backend!(self.get(url))
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        delegate_to_backend!(self.put(url, video))
    }

    async fn remove(&self, url: &str) -> Result<()> {
        delegate_to_backend!(self.remove(url))
    }

    async fn clean(&self) -> Result<()> {
        delegate_to_backend!(self.clean())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        delegate_to_backend!(self.get_by_id(id))
    }
}

// ── Persistent playlist backend constructors & dispatch ──

#[cfg(persistent_cache)]
impl PersistentPlaylistBackend {
    /// Creates the persistent playlist backend for the selected kind.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns `Error::AmbiguousCacheBackend` if `kind` is `None` and multiple backends are compiled in.
    /// Returns an error if the selected backend fails to initialize.
    pub async fn new(config: &CacheConfig, ttl: Option<u64>) -> Result<Self> {
        match PersistentBackendKind::resolve(config.persistent_backend)? {
            #[cfg(feature = "cache-json")]
            PersistentBackendKind::Json => Ok(Self::Json(JsonPlaylistCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redb")]
            PersistentBackendKind::Redb => Ok(Self::Redb(RedbPlaylistCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redis")]
            PersistentBackendKind::Redis => {
                let url = config.redis_url.as_deref().unwrap_or("redis://127.0.0.1/");
                Ok(Self::Redis(RedisPlaylistCache::new(url, ttl).await?))
            }
        }
    }
}

#[cfg(persistent_cache)]
impl PlaylistBackend for PersistentPlaylistBackend {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        delegate_to_backend!(self.get(url))
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        delegate_to_backend!(self.get_by_id(id))
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        delegate_to_backend!(self.put(url, playlist))
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        delegate_to_backend!(self.invalidate(url))
    }

    async fn clean(&self) -> Result<()> {
        delegate_to_backend!(self.clean())
    }

    async fn clear_all(&self) -> Result<()> {
        delegate_to_backend!(self.clear_all())
    }
}

// ── Persistent file backend constructors & dispatch ──

#[cfg(persistent_cache)]
impl PersistentFileBackend {
    /// Creates the persistent file backend for the selected kind.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    /// * `ttl` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns `Error::AmbiguousCacheBackend` if `kind` is `None` and multiple backends are compiled in.
    /// Returns an error if the selected backend fails to initialize.
    pub async fn new(config: &CacheConfig, ttl: Option<u64>) -> Result<Self> {
        match PersistentBackendKind::resolve(config.persistent_backend)? {
            #[cfg(feature = "cache-json")]
            PersistentBackendKind::Json => Ok(Self::Json(JsonFileCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redb")]
            PersistentBackendKind::Redb => Ok(Self::Redb(RedbFileCache::new(config.cache_dir.clone(), ttl).await?)),
            #[cfg(feature = "cache-redis")]
            PersistentBackendKind::Redis => {
                let url = config.redis_url.as_deref().unwrap_or("redis://127.0.0.1/");
                Ok(Self::Redis(
                    RedisFileCache::new(url, config.cache_dir.clone(), ttl).await?,
                ))
            }
        }
    }
}

#[cfg(persistent_cache)]
impl FileBackend for PersistentFileBackend {
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        delegate_to_backend!(self.get_by_hash(hash))
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        delegate_to_backend!(self.get_by_video_and_format(video_id, format_id))
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        delegate_to_backend!(self.get_by_video_and_preferences(video_id, preferences))
    }

    async fn put(&self, file: CachedFile, source_path: &std::path::Path) -> Result<PathBuf> {
        delegate_to_backend!(self.put(file, source_path))
    }

    async fn remove(&self, id: &str) -> Result<()> {
        delegate_to_backend!(self.remove(id))
    }

    async fn clean(&self) -> Result<()> {
        delegate_to_backend!(self.clean())
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        delegate_to_backend!(self.get_thumbnail_by_video_id(video_id))
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &std::path::Path) -> Result<PathBuf> {
        delegate_to_backend!(self.put_thumbnail(thumbnail, source_path))
    }

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        delegate_to_backend!(self.get_subtitle_by_language(video_id, language))
    }
}
