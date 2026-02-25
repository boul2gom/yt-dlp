//! Playlist cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching playlist metadata,
//! using pluggable backend implementations.

use crate::cache::backend::{PlaylistBackend, PlaylistBackendEnum};
use crate::error::Result;
use crate::model::playlist::Playlist;
use crate::utils::current_timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Playlist cache for storing and retrieving playlist metadata.
#[derive(Debug)]
pub struct PlaylistCache {
    backend: PlaylistBackendEnum,
}

impl PlaylistCache {
    /// Default TTL: 6 hours
    const DEFAULT_TTL: u64 = 6 * 60 * 60;

    /// Create a new PlaylistCache with default TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    ///
    /// # Returns
    ///
    /// A new `PlaylistCache` instance with default TTL (6 hours).
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub async fn new(cache_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_ttl(cache_dir, Self::DEFAULT_TTL).await
    }

    /// Create a new PlaylistCache with custom TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - Directory where cache data will be stored.
    /// * `ttl_seconds` - Time-to-live for cache entries in seconds.
    ///
    /// # Returns
    ///
    /// A new `PlaylistCache` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails or no backend is enabled.
    pub async fn with_ttl(cache_dir: impl Into<PathBuf>, ttl_seconds: u64) -> Result<Self> {
        let cache_dir = cache_dir.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl_seconds = ttl_seconds,
            "⚙️ Creating playlist cache"
        );

        let backend = PlaylistBackendEnum::new(cache_dir, Some(ttl_seconds)).await?;
        Ok(Self { backend })
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

        let result = self.backend.get(url).await;

        if let Ok(Some(_)) = &result {
            tracing::debug!(url = url, "✅ Playlist cache hit by URL");
        }

        result
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

        let result = self.backend.get_by_id(id).await;

        if let Ok(Some(_)) = &result {
            tracing::debug!(playlist_id = id, "✅ Playlist cache hit by ID");
        }

        result
    }

    /// Store a playlist in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist.
    /// * `playlist` - The playlist metadata to cache.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend put operation fails.
    pub async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            playlist_title = playlist.title,
            entry_count = playlist.entries.len(),
            "⚙️ Storing playlist in cache"
        );

        self.backend.put(url, playlist).await
    }

    /// Remove a playlist from the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to invalidate.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend invalidate operation fails.
    pub async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in cache");

        self.backend.invalidate(url).await
    }

    /// Clean expired entries.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clean operation fails.
    pub async fn clean(&self) -> Result<()> {
        tracing::debug!("⚙️ Cleaning playlist cache");

        self.backend.clean().await
    }

    /// Clear all playlists.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend clear operation fails.
    pub async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from cache");

        self.backend.clear_all().await
    }
}
