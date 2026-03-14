//! Playlist cache data types and tiered wrapper.
//!
//! Provides the `CachedPlaylist` data structure and the `PlaylistCache` wrapper
//! that orchestrates L1 (Moka) and L2 (persistent) lookups.

use serde::{Deserialize, Serialize};

#[cfg(persistent_cache)]
use crate::cache::backend::PersistentPlaylistBackend;
use crate::cache::backend::PlaylistBackend;
#[cfg(feature = "cache-memory")]
use crate::cache::backend::memory::MokaPlaylistCache;
use crate::cache::config::CacheConfig;
use crate::error::Result;
use crate::model::playlist::Playlist;
use crate::utils::current_timestamp;

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
        // Serialization failure here would indicate a Playlist struct that can't
        // round-trip, which is a programming error — panic is appropriate.
        let playlist_json = serde_json::to_string(&playlist).expect("Playlist serialization must not fail");

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
    #[cfg(persistent_cache)]
    persistent: PersistentPlaylistBackend,
}

impl PlaylistCache {
    /// Create a new PlaylistCache with custom TTL.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration specifying directories, TTLs, and backend settings.
    /// * `ttl` - Time-to-live for cache entries in seconds (optional).
    ///
    /// # Returns
    ///
    /// A new `PlaylistCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails or the backend is ambiguous.
    pub async fn new(config: &CacheConfig, ttl: Option<u64>) -> Result<Self> {
        tracing::debug!(cache_dir = ?config.cache_dir, ttl = ?ttl, "⚙️ Creating playlist cache");

        Ok(Self {
            #[cfg(feature = "cache-memory")]
            memory: MokaPlaylistCache::new(config.cache_dir.clone(), ttl).await?,
            #[cfg(persistent_cache)]
            persistent: PersistentPlaylistBackend::new(config, ttl).await?,
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
        #[cfg(persistent_cache)]
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
        #[cfg(persistent_cache)]
        if let Some(playlist) = self.persistent.get_by_id(id).await? {
            tracing::debug!(playlist_id = id, "✅ Playlist cache hit by ID (L2 persistent)");
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
        tracing::debug!(url = url, playlist_id = playlist.id, "⚙️ Storing playlist in cache");

        #[cfg(feature = "cache-memory")]
        self.memory.put(url.clone(), playlist.clone()).await?;

        #[cfg(persistent_cache)]
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

        #[cfg(persistent_cache)]
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

        #[cfg(persistent_cache)]
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

        #[cfg(persistent_cache)]
        self.persistent.clear_all().await?;

        Ok(())
    }
}
