//! Cache module for storing video metadata.
//!
//! This module provides functionality for caching video metadata
//! to avoid making repeated requests for the same videos.

use crate::error::Result;
use crate::model::Video;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedVideo {
    /// The ID of the video.
    pub id: String,
    /// The title of the video.
    pub title: String,
    /// The URL of the video.
    pub url: String,
    /// The complete video metadata.
    pub video: Video,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: u64,
}

impl From<(String, Video)> for CachedVideo {
    fn from((url, video): (String, Video)) -> Self {
        Self {
            id: video.id.clone(),
            title: video.title.clone(),
            url,
            video,
            cached_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

/// Cache manager for video metadata using SQLite.
#[derive(Debug)]
pub struct VideoCache {
    /// The SQLite connection.
    connection: Arc<Mutex<Connection>>,
    /// The time-to-live for cache entries in seconds.
    ttl: u64,
}

impl VideoCache {
    /// Creates a new cache manager.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache database.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 24 hours).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory cannot be created or the database cannot be initialized.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn new(cache_dir: impl AsRef<Path> + std::fmt::Debug, ttl: Option<u64>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating new video cache in {:?}", cache_dir);

        // Create the cache directory if it doesn't exist
        if !cache_dir.as_ref().exists() {
            std::fs::create_dir_all(cache_dir.as_ref())?;
        }

        let db_path = cache_dir.as_ref().join("video_cache.db");
        let connection = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        // Initialize the database schema
        connection.execute(
            "CREATE TABLE IF NOT EXISTS videos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                video_json TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Create an index on the URL for faster lookups
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_videos_url ON videos(url)",
            [],
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            ttl: ttl.unwrap_or(24 * 60 * 60), // 24 hours by default
        })
    }

    /// Retrieves a video from the cache by its URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to retrieve.
    ///
    /// # Returns
    ///
    /// Returns `Some(Video)` if the video is in the cache and has not expired, otherwise `None`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn get(&self, url: &str) -> Option<Video> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for video in cache: {}", url);

        let connection = self.connection.lock().unwrap();

        // Look up by URL
        let mut stmt = connection
            .prepare("SELECT id, title, url, video_json, cached_at FROM videos WHERE url = ?")
            .ok()?;

        let mut rows = stmt.query(params![url]).ok()?;

        if let Some(row) = rows.next().ok()? {
            // Check if the cache has expired
            let cached_at: u64 = row.get(4).ok()?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let video_json: String = row.get(3).ok()?;
                let video: Video = serde_json::from_str(&video_json).ok()?;

                #[cfg(feature = "tracing")]
                tracing::debug!("Cache hit for video: {}", url);

                return Some(video);
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache expired for video: {}", url);
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache miss for video: {}", url);
        }

        None
    }

    /// Puts a video in the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    /// * `video` - The video metadata.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", skip(video)))]
    pub fn put(&self, url: String, video: Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching video: {}", url);

        let cached = CachedVideo::from((url, video));
        let video_json = serde_json::to_string(&cached.video)?;

        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT OR REPLACE INTO videos (id, title, url, video_json, cached_at) VALUES (?, ?, ?, ?, ?)",
            params![
                cached.id,
                cached.title,
                cached.url,
                video_json,
                cached.cached_at
            ],
        )?;

        Ok(())
    }

    /// Removes a video from the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to remove.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn remove(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Removing video from cache: {}", url);

        let connection = self.connection.lock().unwrap();

        connection.execute("DELETE FROM videos WHERE url = ?", params![url])?;

        Ok(())
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache cannot be written to the database.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning video cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let connection = self.connection.lock().unwrap();

        connection.execute(
            "DELETE FROM videos WHERE cached_at < ?",
            params![now - self.ttl],
        )?;

        Ok(())
    }
}
