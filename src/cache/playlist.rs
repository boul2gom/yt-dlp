//! Playlist cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching playlist metadata,
//! using pluggable backend implementations.

use crate::cache::backend::{PlaylistBackend, PlaylistBackendEnum};
use crate::cache::current_timestamp;
use crate::error::{Error, Result};
use crate::model::playlist::Playlist;
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
            cached_at: current_timestamp(),
        }
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

        #[cfg(feature = "tracing")]
        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl_seconds = ttl_seconds,
            "Creating playlist cache"
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
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Retrieving playlist from cache by URL");

        let result = self.backend.get(url).await;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = url,
            found = result.as_ref().map(|r| r.is_some()).unwrap_or(false),
            "Playlist cache lookup by URL completed"
        );

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
        #[cfg(feature = "tracing")]
        tracing::debug!(playlist_id = id, "Retrieving playlist from cache by ID");

        let result = self.backend.get_by_id(id).await;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            playlist_id = id,
            found = result.as_ref().map(|r| r.is_some()).unwrap_or(false),
            "Playlist cache lookup by ID completed"
        );

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
        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = %url,
            playlist_id = %playlist.id,
            playlist_title = %playlist.title,
            entry_count = playlist.entries.len(),
            "Putting playlist in cache"
        );

        let result = self.backend.put(url.clone(), playlist).await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!(url = %url, "Successfully cached playlist");
        } else {
            tracing::debug!(url = %url, "Failed to cache playlist");
        }

        result
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
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Invalidating playlist in cache");

        let result = self.backend.invalidate(url).await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!(url = url, "Successfully invalidated playlist");
        } else {
            tracing::debug!(url = url, "Failed to invalidate playlist");
        }

        result
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
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning playlist cache");

        let result = self.backend.clean().await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!("Successfully cleaned playlist cache");
        } else {
            tracing::debug!("Failed to clean playlist cache");
        }

        result
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
        #[cfg(feature = "tracing")]
        tracing::debug!("Clearing all playlists from cache");

        let result = self.backend.clear_all().await;

        #[cfg(feature = "tracing")]
        if result.is_ok() {
            tracing::debug!("Successfully cleared all playlists");
        } else {
            tracing::debug!("Failed to clear all playlists");
        }

        result
    }
}
