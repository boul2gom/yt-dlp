//! Playlist cache data types and tiered wrapper.
//!
//! Provides the `CachedPlaylist` data structure and the `PlaylistCache` wrapper
//! that orchestrates L1 (Moka) and L2 (persistent) lookups.

#[cfg(has_persistent_cache)]
use crate::cache::backend::PersistentPlaylistBackend;
use crate::cache::backend::PlaylistBackend;
#[cfg(feature = "cache-memory")]
use crate::cache::backend::memory::MokaPlaylistCache;
use crate::error::Result;
use crate::model::playlist::Playlist;
use crate::utils::current_timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Structure for storing playlist metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedPlaylist {
    /// The ID of the playlist.
    pub id: String,
    /// The title of the playlist.
    pub title: String,
    /// The URL of the playlist.
    pub url: String,
    /// The complete playlist metadata as JSON.
    pub playlist_json: String,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: i64,
}

impl CachedPlaylist {
    /// Deserialize the cached playlist JSON into a Playlist struct.
    ///
    /// # Returns
    ///
    /// The deserialized `Playlist` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON deserialization fails.
    pub fn playlist(&self) -> Result<Playlist> {
        Ok(serde_json::from_str(&self.playlist_json)?)
    }
}

impl From<(String, Playlist)> for CachedPlaylist {
    fn from((url, playlist): (String, Playlist)) -> Self {
        let playlist_json = serde_json::to_string(&playlist).unwrap_or_default();

        Self {
            id: playlist.id.clone(),
            title: playlist.title.clone(),
            url,
            playlist_json,
            cached_at: current_timestamp(),
        }
    }
}

impl std::fmt::Display for CachedPlaylist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CachedPlaylist(id={}, title={})", self.id, self.title)
    }
}

/// Playlist cache manager with tiered L1 (Moka) + L2 (persistent) lookup.
#[derive(Debug)]
pub struct PlaylistCache {
    #[cfg(feature = "cache-memory")]
    memory: MokaPlaylistCache,
    #[cfg(has_persistent_cache)]
    persistent: PersistentPlaylistBackend,
}

impl PlaylistCache {
    /// Default TTL: 6 hours
    const DEFAULT_TTL: u64 = 6 * 60 * 60;

    /// Create a new PlaylistCache with default TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    /// * `redis_url` - Connection URL for Redis backend (only when `cache-redis` is enabled).
    ///
    /// # Returns
    ///
    /// A new `PlaylistCache` instance with default TTL (6 hours).
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub async fn new(
        cache_dir: impl Into<PathBuf>,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
    ) -> Result<Self> {
        Self::with_ttl(
            cache_dir,
            #[cfg(feature = "cache-redis")]
            redis_url,
            Self::DEFAULT_TTL,
        )
        .await
    }

    /// Create a new PlaylistCache with custom TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    /// * `redis_url` - Connection URL for Redis backend (only when `cache-redis` is enabled).
    /// * `ttl_seconds` - Time-to-live for cache entries in seconds.
    ///
    /// # Returns
    ///
    /// A new `PlaylistCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub async fn with_ttl(
        cache_dir: impl Into<PathBuf>,
        #[cfg(feature = "cache-redis")] redis_url: Option<&str>,
        ttl_seconds: u64,
    ) -> Result<Self> {
        let cache_dir = cache_dir.into();

        tracing::debug!(cache_dir = ?cache_dir, ttl_seconds = ttl_seconds, "⚙️ Creating playlist cache");

        Ok(Self {
            #[cfg(feature = "cache-memory")]
            memory: MokaPlaylistCache::new(cache_dir.clone(), Some(ttl_seconds)).await?,
            #[cfg(has_persistent_cache)]
            persistent: PersistentPlaylistBackend::new(
                cache_dir,
                #[cfg(feature = "cache-redis")]
                redis_url,
                Some(ttl_seconds),
            )
            .await?,
        })
    }

    /// Get a playlist from the cache by URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to retrieve.
    ///
    /// # Returns
    ///
    /// `Some(Playlist)` if found and not expired, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend query fails.
    pub async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(url = url, "🔍 Looking up playlist by URL");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(playlist) = self.memory.get(url).await? {
            tracing::debug!(url = url, "✅ Playlist cache hit (L1 memory)");
            return Ok(Some(playlist));
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(playlist) = self.persistent.get(url).await? {
            tracing::debug!(url = url, "✅ Playlist cache hit (L2 persistent)");

            // Backfill L1
            #[cfg(feature = "cache-memory")]
            let _ = self.memory.put(url.to_string(), playlist.clone()).await;

            return Ok(Some(playlist));
        }

        Ok(None)
    }

    /// Get a playlist from the cache by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The playlist ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(Playlist)` if found and not expired, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend query fails.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(playlist_id = id, "🔍 Looking up playlist by ID");

        // L1: Moka
        #[cfg(feature = "cache-memory")]
        if let Some(playlist) = self.memory.get_by_id(id).await? {
            tracing::debug!(playlist_id = id, "✅ Playlist cache hit by ID (L1 memory)");
            return Ok(Some(playlist));
        }

        // L2: persistent
        #[cfg(has_persistent_cache)]
        if let Some(playlist) = self.persistent.get_by_id(id).await? {
            tracing::debug!(
                playlist_id = id,
                "✅ Playlist cache hit by ID (L2 persistent)"
            );
            return Ok(Some(playlist));
        }

        Ok(None)
    }

    /// Store a playlist in the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist.
    /// * `playlist` - The playlist metadata to cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend put operation fails.
    pub async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            "⚙️ Storing playlist in cache"
        );

        #[cfg(feature = "cache-memory")]
        self.memory.put(url.clone(), playlist.clone()).await?;

        #[cfg(has_persistent_cache)]
        self.persistent.put(url, playlist).await?;

        Ok(())
    }

    /// Remove a playlist from the cache (both layers).
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to invalidate.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend invalidate operation fails.
    pub async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in cache");

        #[cfg(feature = "cache-memory")]
        self.memory.invalidate(url).await?;

        #[cfg(has_persistent_cache)]
        self.persistent.invalidate(url).await?;

        Ok(())
    }

    /// Clean expired entries (both layers).
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clean operation fails.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("⚙️ Cleaning playlist cache");

        #[cfg(feature = "cache-memory")]
        self.memory.clean().await?;

        #[cfg(has_persistent_cache)]
        self.persistent.clean().await?;

        Ok(())
    }

    /// Clear all playlists (both layers).
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clear operation fails.
    pub async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from cache");

        #[cfg(feature = "cache-memory")]
        self.memory.clear_all().await?;

        #[cfg(has_persistent_cache)]
        self.persistent.clear_all().await?;

        Ok(())
    }
}
