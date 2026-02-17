//! Playlist cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching playlist metadata,
//! using pluggable backend implementations.

use crate::cache::backend::PlaylistBackend;
use crate::error::{Error, Result};
use crate::model::playlist::Playlist;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(all(feature = "cache-json", not(feature = "cache-sqlite")))]
use crate::cache::backend::json::JsonPlaylistCache;
#[cfg(feature = "cache-sqlite")]
use crate::cache::backend::sqlite::SqlitePlaylistCache;

/// Structure for storing playlist metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "cache-sqlite", derive(sqlx::FromRow))]
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
    pub fn playlist(&self) -> Result<Playlist> {
        serde_json::from_str(&self.playlist_json)
            .map_err(|e| Error::Unknown(format!("Failed to parse playlist: {}", e)))
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
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }
}

/// Playlist cache for storing and retrieving playlist metadata.
#[derive(Debug)]
pub struct PlaylistCache {
    backend: Box<dyn PlaylistBackend>,
}

impl PlaylistCache {
    /// Default TTL: 6 hours
    const DEFAULT_TTL: u64 = 6 * 60 * 60;

    /// Create a new PlaylistCache.
    pub async fn new(cache_dir: impl AsRef<Path>) -> Result<Self> {
        Self::with_ttl(cache_dir, Self::DEFAULT_TTL).await
    }

    /// Create a new PlaylistCache with custom TTL.
    pub async fn with_ttl(cache_dir: impl AsRef<Path>, ttl_seconds: u64) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        #[cfg(feature = "cache-sqlite")]
        {
            let backend = SqlitePlaylistCache::new(cache_dir, Some(ttl_seconds)).await?;
            Ok(Self {
                backend: Box::new(backend),
            })
        }

        #[cfg(all(feature = "cache-json", not(feature = "cache-sqlite")))]
        {
            let backend = JsonPlaylistCache::new(cache_dir, Some(ttl_seconds)).await?;
            Ok(Self {
                backend: Box::new(backend),
            })
        }

        #[cfg(not(any(feature = "cache-sqlite", feature = "cache-json")))]
        {
            Err(Error::Unknown("No cache backend enabled".to_string()))
        }
    }

    /// Get a playlist from the cache by URL.
    pub async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        self.backend.get(url).await
    }

    /// Get a playlist from the cache by ID.
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        self.backend.get_by_id(id).await
    }

    /// Store a playlist in the cache.
    pub async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        self.backend.put(url, playlist).await
    }

    /// Remove a playlist from the cache.
    pub async fn invalidate(&self, url: &str) -> Result<()> {
        self.backend.invalidate(url).await
    }

    /// Clean expired entries.
    pub async fn clean(&self) -> Result<()> {
        self.backend.clean().await
    }

    /// Clear all playlists.
    pub async fn clear_all(&self) -> Result<()> {
        self.backend.clear_all().await
    }
}
