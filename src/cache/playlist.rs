//! Playlist cache wrapper using backend implementations.
//!
//! This module provides a high-level API for caching playlist metadata,
//! using pluggable backend implementations (SQLite by default).

use crate::error::{Error, Result};
use crate::model::playlist::Playlist;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "cache")]
use sqlx::SqlitePool;

/// Structure for storing playlist metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "cache", derive(sqlx::FromRow))]
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
    /// # Errors
    ///
    /// Returns an error if the JSON cannot be deserialized.
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
///
/// The cache uses SQLite as the backend and supports TTL (time-to-live) for entries.
/// Default TTL is 6 hours (playlists change more frequently than videos).
#[cfg(feature = "cache")]
#[derive(Debug, Clone)]
pub struct PlaylistCache {
    pool: Arc<SqlitePool>,
    ttl_seconds: i64,
}

#[cfg(feature = "cache")]
impl PlaylistCache {
    /// Default TTL for playlist cache: 6 hours
    const DEFAULT_TTL: i64 = 6 * 60 * 60;

    /// Create a new PlaylistCache with the given database path.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!("Initializing playlist cache at {:?}", db_path);

        // Create database URL
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        // Create connection pool
        let pool = SqlitePool::connect(&db_url)
            .await
            .map_err(|e| Error::database("connect to playlist cache database", e))?;

        // Create cache instance
        let cache = Self {
            pool: Arc::new(pool),
            ttl_seconds: Self::DEFAULT_TTL,
        };

        // Initialize the database schema
        cache.init().await?;

        Ok(cache)
    }

    /// Create a new PlaylistCache with a custom TTL.
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file
    /// * `ttl_seconds` - Time-to-live in seconds
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub async fn with_ttl(db_path: impl AsRef<Path>, ttl_seconds: i64) -> Result<Self> {
        let mut cache = Self::new(db_path).await?;
        cache.ttl_seconds = ttl_seconds;
        Ok(cache)
    }

    /// Initialize the database schema.
    ///
    /// Creates the playlist_cache table if it doesn't exist.
    async fn init(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating playlist cache table if not exists");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playlist_cache (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                playlist_json TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::database("create playlist cache table", e))?;

        // Create index on URL for faster lookups
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_playlist_url
            ON playlist_cache(url)
            "#,
        )
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::database("create playlist URL index", e))?;

        Ok(())
    }

    /// Get a playlist from the cache by URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    ///
    /// # Returns
    ///
    /// Some(Playlist) if found and not expired, None otherwise
    pub async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Looking up playlist in cache: {}", url);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cached: Option<CachedPlaylist> = sqlx::query_as(
            r#"
            SELECT id, title, url, playlist_json, cached_at
            FROM playlist_cache
            WHERE url = ?
            AND cached_at > ?
            "#,
        )
        .bind(url)
        .bind(now - self.ttl_seconds)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Error::database("fetch playlist from cache", e))?;

        match cached {
            Some(cached) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Found playlist in cache: {}", cached.id);
                Ok(Some(cached.playlist()?))
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Playlist not found in cache or expired");
                Ok(None)
            }
        }
    }

    /// Get a playlist from the cache by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the playlist
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    ///
    /// # Returns
    ///
    /// Some(Playlist) if found and not expired, None otherwise
    pub async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Looking up playlist by ID in cache: {}", id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cached: Option<CachedPlaylist> = sqlx::query_as(
            r#"
            SELECT id, title, url, playlist_json, cached_at
            FROM playlist_cache
            WHERE id = ?
            AND cached_at > ?
            "#,
        )
        .bind(id)
        .bind(now - self.ttl_seconds)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Error::database("fetch playlist by ID from cache", e))?;

        match cached {
            Some(cached) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Found playlist in cache: {}", cached.id);
                Ok(Some(cached.playlist()?))
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Playlist not found in cache or expired");
                Ok(None)
            }
        }
    }

    /// Store a playlist in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist
    /// * `playlist` - The playlist to cache
    ///
    /// # Errors
    ///
    /// Returns an error if the database insert fails.
    pub async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching playlist: {}", playlist.id);

        let cached = CachedPlaylist::from((url, playlist));

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO playlist_cache (id, title, url, playlist_json, cached_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&cached.id)
        .bind(&cached.title)
        .bind(&cached.url)
        .bind(&cached.playlist_json)
        .bind(cached.cached_at)
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::database("insert playlist into cache", e))?;

        #[cfg(feature = "tracing")]
        tracing::debug!("Successfully cached playlist: {}", cached.id);

        Ok(())
    }

    /// Remove a playlist from the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist to remove
    ///
    /// # Errors
    ///
    /// Returns an error if the database delete fails.
    pub async fn invalidate(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Invalidating playlist cache for: {}", url);

        sqlx::query(
            r#"
            DELETE FROM playlist_cache
            WHERE url = ?
            "#,
        )
        .bind(url)
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::database("delete playlist from cache", e))?;

        Ok(())
    }

    /// Clear all expired entries from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the database delete fails.
    pub async fn clear_expired(&self) -> Result<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        #[cfg(feature = "tracing")]
        tracing::debug!("Clearing expired playlists from cache");

        let result = sqlx::query(
            r#"
            DELETE FROM playlist_cache
            WHERE cached_at <= ?
            "#,
        )
        .bind(now - self.ttl_seconds)
        .execute(&*self.pool)
        .await
        .map_err(|e| Error::database("clear expired playlists", e))?;

        let rows_affected = result.rows_affected();

        #[cfg(feature = "tracing")]
        tracing::debug!("Cleared {} expired playlists", rows_affected);

        Ok(rows_affected)
    }

    /// Clear all playlists from the cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the database delete fails.
    pub async fn clear_all(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Clearing all playlists from cache");

        sqlx::query("DELETE FROM playlist_cache")
            .execute(&*self.pool)
            .await
            .map_err(|e| Error::database("clear all playlists", e))?;

        Ok(())
    }
}
