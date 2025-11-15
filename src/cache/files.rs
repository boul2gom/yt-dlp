//! Async-safe cache implementation for downloaded files using sqlx.
//!
//! This module provides a fully async cache implementation that does not block
//! the tokio runtime, unlike the sync implementation with rusqlite.

use crate::cache::video::{CachedFile, CachedThumbnail, CachedType};
use crate::error::Result;
use crate::model::format::Format;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Type alias for full file query result tuple.
type FileQueryResult = (
    String,         // id
    String,         // filename
    String,         // relative_path
    Option<String>, // video_id
    String,         // file_type
    Option<String>, // format_id
    Option<String>, // format_json
    Option<String>, // video_quality
    Option<String>, // audio_quality
    Option<String>, // video_codec
    Option<String>, // audio_codec
    Option<String>, // language_code
    i64,            // filesize
    String,         // mime_type
    i64,            // cached_at
);

/// Type alias for thumbnail query result tuple.
type ThumbnailQueryResult = (
    String,         // id
    String,         // filename
    String,         // relative_path
    Option<String>, // video_id
    String,         // file_type
    Option<String>, // format_id
    Option<String>, // format_json
    i64,            // filesize
    String,         // mime_type
    i64,            // cached_at
);

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone)]
pub struct DownloadCache {
    /// The SQLite connection pool.
    pool: SqlitePool,
    /// The time-to-live for cache entries in seconds.
    ttl: i64,
    /// The directory where to store the cached files.
    cache_dir: PathBuf,
}

impl DownloadCache {
    /// Creates a new download cache with the specified cache directory and TTL.
    ///
    /// # Arguments
    ///
    /// * `cache_path` - The path to the cache directory.
    /// * `ttl` - The time-to-live for cache entries in seconds (optional, defaults to 7 days).
    ///
    /// # Returns
    ///
    /// Returns a new download cache instance if successful.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory cannot be created
    /// or the database connection cannot be established.
    pub async fn new(
        cache_path: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating download cache at {:?}", cache_path);

        // Create the cache directory if it doesn't exist
        let cache_dir = cache_path.as_ref().to_path_buf();
        tokio::fs::create_dir_all(&cache_dir).await?;

        // Create the database file
        let db_path = cache_dir.join("downloads.db");

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

        // Create the files table if it doesn't exist
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
                language_code TEXT,
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

        // Create the thumbnails table if it doesn't exist
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS thumbnails (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                video_id TEXT NOT NULL,
                filesize INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                width INTEGER,
                height INTEGER,
                cached_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| {
            crate::error::Error::Unknown(format!("Failed to create thumbnails table: {}", e))
        })?;

        // Create indexes for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_video_id ON files (video_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_format_id ON files (format_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_files_language_code ON files (video_id, language_code)",
        )
        .execute(&pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_thumbnails_video_id ON thumbnails (video_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to create index: {}", e)))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60) as i64,
            cache_dir,
        })
    }

    /// Calculates the SHA-256 hash of a file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path to the file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be read.
    pub async fn calculate_file_hash(
        file_path: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<String> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Calculating hash for file {:?}", file_path);

        let mut file = File::open(&file_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        let mut hasher = Sha256::new();
        hasher.update(&buffer);
        let hash = hasher.finalize();

        Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
    }

    /// Sanitize a filename to prevent path traversal attacks
    fn sanitize_filename(filename: &str) -> String {
        filename
            .replace("..", "")
            .replace(['/', '\\', ':'], "")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect()
    }

    /// Determines the MIME type of a file based on its extension.
    fn determine_mime_type(file_path: impl AsRef<Path>) -> String {
        let extension = file_path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension.to_lowercase().as_str() {
            "mp4" => "video/mp4".to_string(),
            "webm" => "video/webm".to_string(),
            "mp3" => "audio/mpeg".to_string(),
            "m4a" => "audio/mp4".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "png" => "image/png".to_string(),
            "vtt" => "text/vtt".to_string(),
            "srt" => "application/x-subrip".to_string(),
            "ass" | "ssa" => "text/x-ssa".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache entries cannot be removed.
    pub async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning download cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        // Get all expired files with their paths
        let expired_files: Vec<(String,)> =
            sqlx::query_as("SELECT relative_path FROM files WHERE cached_at < ?")
                .bind(cutoff)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| {
                    crate::error::Error::Unknown(format!("Failed to query expired files: {}", e))
                })?;

        // Delete expired files from filesystem
        for (relative_path,) in expired_files {
            let file_path = self.cache_dir.join(&relative_path);
            if file_path.exists()
                && let Err(_e) = tokio::fs::remove_file(&file_path).await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    "Failed to delete cached file {}: {}",
                    file_path.display(),
                    _e
                );
            }
        }

        // Delete expired entries from database
        sqlx::query("DELETE FROM files WHERE cached_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Unknown(format!("Failed to clean files: {}", e)))?;

        Ok(())
    }

    /// Gets a file from the cache by hash.
    ///
    /// # Arguments
    ///
    /// * `file_hash` - The SHA-256 hash of the file.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if found and not expired.
    pub async fn get_by_hash(&self, file_hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for file in cache by hash: {}", file_hash);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let result: Option<FileQueryResult> = sqlx::query_as(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at
             FROM files
             WHERE id = ? AND cached_at > ?"
        )
        .bind(file_hash)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        if let Some((
            id,
            filename,
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality,
            audio_quality,
            video_codec,
            audio_codec,
            language_code,
            filesize,
            mime_type,
            cached_at,
        )) = result
        {
            let file_path = self.cache_dir.join(&relative_path);

            if file_path.exists() {
                let cached_file = CachedFile {
                    id,
                    filename,
                    relative_path,
                    video_id,
                    file_type,
                    format_id,
                    format_json,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                    language_code,
                    filesize,
                    mime_type,
                    cached_at,
                };

                return Some((cached_file, file_path));
            }
        }

        None
    }

    /// Puts a file in the cache.
    pub async fn put_file(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: Option<String>,
        format: Option<&Format>,
    ) -> Result<CachedFile> {
        self.put_file_with_preferences(
            source_path,
            filename,
            video_id,
            format,
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Puts a file in the cache with preferences.
    #[allow(clippy::too_many_arguments)]
    pub async fn put_file_with_preferences(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: Option<String>,
        format: Option<&Format>,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Result<CachedFile> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching file {:?}", source_path);

        let file_hash = Self::calculate_file_hash(&source_path).await?;
        let metadata = tokio::fs::metadata(&source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(&source_path);

        let filename_str = filename.as_ref();
        let sanitized_filename = Self::sanitize_filename(filename_str);
        let extension = Path::new(&sanitized_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("files/{}.{}", file_hash, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if !dest_path.exists() {
            tokio::fs::copy(&source_path, &dest_path).await?;
        }

        let (file_type, format_id, format_json) = if let Some(f) = format {
            (
                serde_json::to_string(&CachedType::Format).unwrap_or_default(),
                Some(f.format_id.clone()),
                Some(serde_json::to_string(f).unwrap_or_default()),
            )
        } else {
            (
                serde_json::to_string(&CachedType::Other).unwrap_or_default(),
                None,
                None,
            )
        };

        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let video_quality_str =
            video_quality.map(|vq| serde_json::to_string(&vq).unwrap_or_default());
        let audio_quality_str =
            audio_quality.map(|aq| serde_json::to_string(&aq).unwrap_or_default());
        let video_codec_str = video_codec
            .clone()
            .map(|vc| serde_json::to_string(&vc).unwrap_or_default());
        let audio_codec_str = audio_codec
            .clone()
            .map(|ac| serde_json::to_string(&ac).unwrap_or_default());

        sqlx::query(
            "INSERT OR REPLACE INTO files
             (id, filename, relative_path, video_id, file_type, format_id, format_json,
              video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&file_hash)
        .bind(filename_str)
        .bind(&relative_path)
        .bind(&video_id)
        .bind(&file_type)
        .bind(&format_id)
        .bind(&format_json)
        .bind(&video_quality_str)
        .bind(&audio_quality_str)
        .bind(&video_codec_str)
        .bind(&audio_codec_str)
        .bind::<Option<String>>(None) // language_code (only for subtitles)
        .bind(filesize)
        .bind(&mime_type)
        .bind(cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to insert file: {}", e)))?;

        Ok(CachedFile {
            id: file_hash,
            filename: filename_str.to_string(),
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality: video_quality_str,
            audio_quality: audio_quality_str,
            video_codec: video_codec_str,
            audio_codec: audio_codec_str,
            language_code: None,
            filesize,
            mime_type,
            cached_at,
        })
    }

    /// Gets a file from cache by video ID and format ID.
    pub async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for file in cache by video ID: {} and format ID: {}",
            video_id,
            format_id
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let result: Option<FileQueryResult> = sqlx::query_as(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at
             FROM files
             WHERE video_id = ? AND format_id = ? AND cached_at > ?"
        )
        .bind(video_id)
        .bind(format_id)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        if let Some((
            id,
            filename,
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality,
            audio_quality,
            video_codec,
            audio_codec,
            language_code,
            filesize,
            mime_type,
            cached_at,
        )) = result
        {
            let file_path = self.cache_dir.join(&relative_path);

            if file_path.exists() {
                let cached_file = CachedFile {
                    id,
                    filename,
                    relative_path,
                    video_id,
                    file_type,
                    format_id,
                    format_json,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                    language_code,
                    filesize,
                    mime_type,
                    cached_at,
                };

                return Some((cached_file, file_path));
            }
        }

        None
    }

    /// Gets a file from cache by video ID and preferences.
    pub async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for file in cache by video ID: {} and preferences",
            video_id
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let mut query = "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                                video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at
                         FROM files
                         WHERE video_id = ? AND cached_at > ?".to_string();

        let mut bindings: Vec<String> = vec![video_id.to_string(), cutoff.to_string()];

        if let Some(vq) = &video_quality {
            query.push_str(" AND video_quality = ?");
            bindings.push(serde_json::to_string(vq).unwrap_or_default());
        }

        if let Some(aq) = &audio_quality {
            query.push_str(" AND audio_quality = ?");
            bindings.push(serde_json::to_string(aq).unwrap_or_default());
        }

        if let Some(vc) = &video_codec {
            query.push_str(" AND video_codec = ?");
            bindings.push(serde_json::to_string(vc).unwrap_or_default());
        }

        if let Some(ac) = &audio_codec {
            query.push_str(" AND audio_codec = ?");
            bindings.push(serde_json::to_string(ac).unwrap_or_default());
        }

        // Build query dynamically with proper bindings
        let mut sql_query = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                i64,
                String,
                i64,
            ),
        >(&query);

        for binding in &bindings {
            sql_query = sql_query.bind(binding);
        }

        let result = sql_query.fetch_optional(&self.pool).await.ok()?;

        if let Some((
            id,
            filename,
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality_str,
            audio_quality_str,
            video_codec_str,
            audio_codec_str,
            language_code,
            filesize,
            mime_type,
            cached_at,
        )) = result
        {
            let file_path = self.cache_dir.join(&relative_path);

            if file_path.exists() {
                let cached_file = CachedFile {
                    id,
                    filename,
                    relative_path,
                    video_id,
                    file_type,
                    format_id,
                    format_json,
                    video_quality: video_quality_str,
                    audio_quality: audio_quality_str,
                    video_codec: video_codec_str,
                    audio_codec: audio_codec_str,
                    language_code,
                    filesize,
                    mime_type,
                    cached_at,
                };

                return Some((cached_file, file_path));
            }
        }

        None
    }

    /// Puts a thumbnail in the cache.
    pub async fn put_thumbnail(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        thumbnail: &crate::model::thumbnail::Thumbnail,
    ) -> Result<CachedThumbnail> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching thumbnail {:?}", source_path);

        let file_hash = Self::calculate_file_hash(&source_path).await?;
        let metadata = tokio::fs::metadata(&source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(&source_path);

        let filename_str = filename.as_ref();
        let extension = Path::new(filename_str)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("thumbnails/{}.{}", file_hash, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::copy(&source_path, &dest_path).await?;

        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let width = thumbnail.width.map(|w| w as i32);
        let height = thumbnail.height.map(|h| h as i32);

        let cached_thumbnail = CachedThumbnail {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path: relative_path.clone(),
            video_id: video_id.clone(),
            filesize,
            mime_type: mime_type.clone(),
            width,
            height,
            cached_at,
        };

        sqlx::query(
            "INSERT INTO thumbnails (id, filename, relative_path, video_id, filesize, mime_type, width, height, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&file_hash)
        .bind(filename_str)
        .bind(&relative_path)
        .bind(&video_id)
        .bind(filesize)
        .bind(&mime_type)
        .bind(width)
        .bind(height)
        .bind(cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to insert thumbnail: {}", e)))?;

        Ok(cached_thumbnail)
    }

    /// Gets a thumbnail from cache by video ID.
    pub async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for thumbnail in cache by video ID: {}", video_id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;

        let file_type_json = serde_json::to_string(&CachedType::Thumbnail).ok()?;

        let result: Option<ThumbnailQueryResult> = sqlx::query_as(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at
             FROM files
             WHERE video_id = ? AND file_type = ? AND format_id = 'thumbnail' AND cached_at > ?"
        )
        .bind(video_id)
        .bind(&file_type_json)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        if let Some((
            id,
            filename,
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            filesize,
            mime_type,
            cached_at,
        )) = result
        {
            let file_path = self.cache_dir.join(&relative_path);

            if file_path.exists() {
                let cached_file = CachedFile {
                    id,
                    filename,
                    relative_path,
                    video_id,
                    file_type,
                    format_id,
                    format_json,
                    video_quality: None,
                    audio_quality: None,
                    video_codec: None,
                    audio_codec: None,
                    language_code: None,
                    filesize,
                    mime_type,
                    cached_at,
                };

                return Some((cached_file, file_path));
            }
        }

        None
    }

    /// Gets a subtitle file from cache by video ID and language code.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The video ID
    /// * `language_code` - The language code of the subtitle (e.g., "en", "fr")
    ///
    /// # Returns
    ///
    /// The cached subtitle file and its path if found and not expired
    pub async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language_code: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for subtitle in cache: video_id={}, language={}",
            video_id,
            language_code
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let cutoff = now - self.ttl;
        let file_type_json = serde_json::to_string(&CachedType::Subtitle).ok()?;

        let result: Option<FileQueryResult> = sqlx::query_as(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at
             FROM files
             WHERE video_id = ? AND language_code = ? AND file_type = ? AND cached_at > ?"
        )
        .bind(video_id)
        .bind(language_code)
        .bind(&file_type_json)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        if let Some((
            id,
            filename,
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            video_quality,
            audio_quality,
            video_codec,
            audio_codec,
            language_code,
            filesize,
            mime_type,
            cached_at,
        )) = result
        {
            let file_path = self.cache_dir.join(&relative_path);

            if file_path.exists() {
                let cached_file = CachedFile {
                    id,
                    filename,
                    relative_path,
                    video_id,
                    file_type,
                    format_id,
                    format_json,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                    language_code,
                    filesize,
                    mime_type,
                    cached_at,
                };

                return Some((cached_file, file_path));
            }
        }

        None
    }

    /// Puts a subtitle file in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - The path to the subtitle file
    /// * `filename` - The filename for the subtitle
    /// * `video_id` - The video ID this subtitle belongs to
    /// * `language_code` - The language code (e.g., "en", "fr")
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be cached
    pub async fn put_subtitle_file(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        language_code: String,
    ) -> Result<CachedFile> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Caching subtitle file {:?} for video {} in language {}",
            source_path,
            video_id,
            language_code
        );

        let file_hash = Self::calculate_file_hash(&source_path).await?;
        let metadata = tokio::fs::metadata(&source_path).await?;
        let filesize = metadata.len() as i64;
        let mime_type = Self::determine_mime_type(&source_path);

        let filename_str = filename.as_ref();
        let sanitized_filename = Self::sanitize_filename(filename_str);
        let extension = Path::new(&sanitized_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("subtitles/{}/{}.{}", video_id, language_code, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if !dest_path.exists() {
            tokio::fs::copy(&source_path, &dest_path).await?;
        }

        let file_type = serde_json::to_string(&CachedType::Subtitle).unwrap_or_default();
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            "INSERT OR REPLACE INTO files
             (id, filename, relative_path, video_id, file_type, format_id, format_json,
              video_quality, audio_quality, video_codec, audio_codec, language_code, filesize, mime_type, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&file_hash)
        .bind(filename_str)
        .bind(&relative_path)
        .bind(&video_id)
        .bind(&file_type)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind(&language_code)
        .bind(filesize)
        .bind(&mime_type)
        .bind(cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Unknown(format!("Failed to insert subtitle file: {}", e)))?;

        Ok(CachedFile {
            id: file_hash,
            filename: filename_str.to_string(),
            relative_path,
            video_id: Some(video_id),
            file_type,
            format_id: None,
            format_json: None,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            language_code: Some(language_code),
            filesize,
            mime_type,
            cached_at,
        })
    }
}
