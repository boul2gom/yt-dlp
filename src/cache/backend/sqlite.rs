//! SQLite backend implementation for video and file caching.
//!
//! This module provides async-safe SQLite implementations using sqlx.

use super::{FileBackend, VideoBackend};
use crate::cache::video::{CachedFile, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "cache")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

/// SQLite-backed video cache implementation.
#[derive(Debug, Clone)]
pub struct SqliteVideoCache {
    pool: SqlitePool,
    ttl: i64,
}

#[async_trait::async_trait]
impl VideoBackend for SqliteVideoCache {
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating new SQLite video cache in {:?}", &cache_dir);

        // Create the cache directory if it doesn't exist
        if !&cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let db_path = &cache_dir.join("video_cache.db");

        // Create connection pool
        let connection_options =
            SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
                .map_err(|e| {
                    crate::error::Error::Unknown(format!(
                        "Failed to create connection options: {}",
                        e
                    ))
                })?
                .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connection_options)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create pool: {}", e)))?;

        // Initialize the database schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS videos (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                video_json TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to create table: {}", e)))?;

        // Create an index on the URL for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_videos_url ON videos(url)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(24 * 60 * 60) as i64,
        })
    }

    async fn get(&self, url: &str) -> Result<Option<Video>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for video in cache: {}", url);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let cached = sqlx::query_as::<_, CachedVideo>(
            "SELECT id, title, url, video_json, cached_at
             FROM videos
             WHERE url = ? AND cached_at > ?",
        )
        .bind(url)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Database query failed: {}", e)))?;

        match cached {
            Some(cv) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache hit for video: {}", url);

                Ok(Some(cv.video()?))
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache miss for video: {}", url);

                Ok(None)
            }
        }
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching video: {}", url);

        let cached = CachedVideo::from((url, video));

        sqlx::query(
            "INSERT OR REPLACE INTO videos (id, title, url, video_json, cached_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&cached.id)
        .bind(&cached.title)
        .bind(&cached.url)
        .bind(&cached.video_json)
        .bind(cached.cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to insert video: {}", e)))?;

        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Removing video from cache: {}", url);

        sqlx::query("DELETE FROM videos WHERE url = ?")
            .bind(url)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to delete video: {}", e)))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning video cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        sqlx::query("DELETE FROM videos WHERE cached_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to clean cache: {}", e)))?;

        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for video in cache by ID: {}", id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let cached = sqlx::query_as::<_, CachedVideo>(
            "SELECT id, title, url, video_json, cached_at
             FROM videos
             WHERE id = ? AND cached_at > ?",
        )
        .bind(id)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Database query failed: {}", e)))?;

        match cached {
            Some(cv) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache hit for video ID: {}", id);

                Ok(cv)
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache miss for video ID: {}", id);

                Err(crate::error::Error::Unknown(format!(
                    "Video with ID {} not found or expired in cache",
                    id
                )))
            }
        }
    }
}

/// SQLite-backed file cache implementation.
#[derive(Debug, Clone)]
pub struct SqliteFileCache {
    pool: SqlitePool,
    ttl: i64,
    cache_dir: PathBuf,
}

#[async_trait::async_trait]
impl FileBackend for SqliteFileCache {
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating new SQLite file cache in {:?}", &cache_dir);

        // Create the cache directory if it doesn't exist
        if !&cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let db_path = &cache_dir.join("file_cache.db");

        // Create connection pool
        let connection_options =
            SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
                .map_err(|e| {
                    crate::error::Error::Unknown(format!(
                        "Failed to create connection options: {}",
                        e
                    ))
                })?
                .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connection_options)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create pool: {}", e)))?;

        // Initialize the database schema for files
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                video_id TEXT,
                file_type TEXT NOT NULL,
                format_id TEXT,
                format_json TEXT,
                video_quality TEXT,
                audio_quality TEXT,
                video_codec TEXT,
                audio_codec TEXT,
                filesize INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| {
            crate::error::Error::Unknown(format!("Failed to create files table: {}", e))
        })?;

        // Create indices for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_video_id ON files(video_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_format_id ON files(format_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60) as i64, // 7 days default
            cache_dir,
        })
    }

    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for file in cache by hash: {}", hash);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at
             FROM files
             WHERE id = ? AND cached_at > ?",
        )
        .bind(hash)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result.map(|cached| {
            let path = self.cache_dir.join(&cached.relative_path);
            (cached, path)
        })
    }

    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for file in cache by video_id={} and format_id={}",
            video_id,
            format_id
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at
             FROM files
             WHERE video_id = ? AND format_id = ? AND cached_at > ?",
        )
        .bind(video_id)
        .bind(format_id)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result.map(|cached| {
            let path = self.cache_dir.join(&cached.relative_path);
            (cached, path)
        })
    }

    #[cfg(feature = "cache")]
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for file in cache by preferences for video_id={}",
            video_id
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let vq = video_quality.and_then(|q| serde_json::to_string(&q).ok());
        let aq = audio_quality.and_then(|q| serde_json::to_string(&q).ok());
        let vc = video_codec.and_then(|c| serde_json::to_string(&c).ok());
        let ac = audio_codec.and_then(|c| serde_json::to_string(&c).ok());

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at
             FROM files
             WHERE video_id = ?
                AND (video_quality = ? OR video_quality IS NULL)
                AND (audio_quality = ? OR audio_quality IS NULL)
                AND (video_codec = ? OR video_codec IS NULL)
                AND (audio_codec = ? OR audio_codec IS NULL)
                AND cached_at > ?",
        )
        .bind(video_id)
        .bind(vq)
        .bind(aq)
        .bind(vc)
        .bind(ac)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result.map(|cached| {
            let path = self.cache_dir.join(&cached.relative_path);
            (cached, path)
        })
    }

    async fn put(&self, file: CachedFile, content: &[u8]) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching file: {}", file.filename);

        // Write file to disk
        let file_path = self.cache_dir.join(&file.relative_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&file_path, content).await?;

        // Store metadata in database
        sqlx::query(
            "INSERT OR REPLACE INTO files
             (id, filename, relative_path, video_id, file_type, format_id, format_json,
              video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&file.id)
        .bind(&file.filename)
        .bind(&file.relative_path)
        .bind(&file.video_id)
        .bind(&file.file_type)
        .bind(&file.format_id)
        .bind(&file.format_json)
        .bind(&file.video_quality)
        .bind(&file.audio_quality)
        .bind(&file.video_codec)
        .bind(&file.audio_codec)
        .bind(file.filesize)
        .bind(&file.mime_type)
        .bind(file.cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to insert file: {}", e)))?;

        Ok(file_path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Removing file from cache: {}", id);

        // Get file path before deleting from database
        if let Some((_cached, path)) = self.get_by_hash(id).await {
            // Delete file from disk
            if path.exists() {
                tokio::fs::remove_file(&path).await?;
            }
        }

        // Delete from database
        sqlx::query("DELETE FROM files WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to delete file: {}", e)))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning file cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        // Get all expired files
        let expired = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at
             FROM files
             WHERE cached_at < ?",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to fetch expired files: {}", e)))?;

        // Delete files from disk
        for file in &expired {
            let path = self.cache_dir.join(&file.relative_path);
            if path.exists() {
                let _ = tokio::fs::remove_file(&path).await; // Ignore errors
            }
        }

        // Delete from database
        sqlx::query("DELETE FROM files WHERE cached_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to clean cache: {}", e)))?;

        Ok(())
    }
}
