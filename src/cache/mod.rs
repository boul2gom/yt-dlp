//! Cache module for storing video metadata and downloaded files.
//!
//! This module provides functionality for caching video metadata and downloaded files
//! to avoid making repeated requests for the same videos and re-downloading the same files.

use crate::error::Result;
use crate::model::format::Format;
use crate::model::thumbnail::Thumbnail;
use crate::model::Video;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

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

/// Structure for storing downloaded file metadata in cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFile {
    /// The ID of the file (SHA-256 hash of the content).
    pub id: String,
    /// The original filename.
    pub filename: String,
    /// The path to the file relative to the cache directory.
    pub relative_path: String,
    /// The video ID this file is associated with (if any).
    pub video_id: Option<String>,
    /// The file type (format, thumbnail, etc.)
    pub file_type: CachedType,
    /// The format ID this file is associated with (if any).
    pub format_id: Option<String>,
    /// The format information serialized as JSON (if available).
    pub format_json: Option<String>,
    /// The file size in bytes.
    pub filesize: u64,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: u64,
}

/// Enum representing the type of cached file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CachedType {
    /// A video or audio format
    Format,
    /// A thumbnail image
    Thumbnail,
    /// Any other type of file
    Other,
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

/// Cache manager for downloaded files using SQLite.
#[derive(Debug)]
pub struct DownloadCache {
    /// The SQLite connection.
    connection: Arc<Mutex<Connection>>,
    /// The time-to-live for cache entries in seconds.
    ttl: u64,
    /// The directory where to store the cached files.
    cache_dir: PathBuf,
}

impl DownloadCache {
    /// Creates a new download cache manager.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache database and files.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 7 days).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory cannot be created or the database cannot be initialized.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn new(cache_dir: impl AsRef<Path> + std::fmt::Debug, ttl: Option<u64>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating new download cache in {:?}", cache_dir);

        // Create the cache directory if it doesn't exist
        let cache_path = cache_dir.as_ref().to_path_buf();
        if !cache_path.exists() {
            std::fs::create_dir_all(&cache_path)?;
        }

        // Create the files directory
        let files_dir = cache_path.join("files");
        if !files_dir.exists() {
            std::fs::create_dir_all(&files_dir)?;
        }

        let db_path = cache_path.join("download_cache.db");
        let connection = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;

        // Initialize the database schema
        connection.execute(
            "CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                video_id TEXT,
                file_type TEXT NOT NULL,
                format_id TEXT,
                format_json TEXT,
                filesize INTEGER NOT NULL,
                mime_type TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
            [],
        )?;

        // Create indexes for faster lookups
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_video_id ON files(video_id)",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_format_id ON files(format_id)",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_file_type ON files(file_type)",
            [],
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60), // 7 days by default
            cache_dir: cache_path,
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
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
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

        Ok(format!("{:x}", hash))
    }

    /// Determines the MIME type of a file based on its extension.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path to the file.
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
            _ => "application/octet-stream".to_string(),
        }
    }

    /// Puts a file in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - The path to the file to cache.
    /// * `filename` - The original filename.
    /// * `video_id` - The ID of the video this file is associated with (if any).
    /// * `format` - The format information (if available).
    ///
    /// # Returns
    ///
    /// Returns the cached file information if successful.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be copied to the cache or the cache entry cannot be written to the database.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(format))
    )]
    pub async fn put_file(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: Option<String>,
        format: Option<&Format>,
    ) -> Result<CachedFile> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching file {:?}", source_path);

        // Calculate the file hash
        let file_hash = Self::calculate_file_hash(&source_path).await?;

        // Get file metadata
        let metadata = tokio::fs::metadata(&source_path).await?;
        let filesize = metadata.len();

        // Determine the MIME type
        let mime_type = Self::determine_mime_type(&source_path);

        // Create the destination path
        let filename_str = filename.as_ref();
        let extension = Path::new(filename_str)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("files/{}.{}", file_hash, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        // Copy the file to the cache directory
        if !dest_path.exists() {
            tokio::fs::copy(&source_path, &dest_path).await?;
        }

        // Prepare format information
        let (file_type, format_id, format_json) = if let Some(f) = format {
            (
                CachedType::Format,
                Some(f.format_id.clone()),
                Some(serde_json::to_string(f).unwrap_or_default()),
            )
        } else {
            (CachedType::Other, None, None)
        };

        // Create the cache entry
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id,
            file_type,
            format_id,
            format_json,
            filesize,
            mime_type,
            cached_at,
        };

        // Store in the database
        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT OR REPLACE INTO files (id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                cached_file.id,
                cached_file.filename,
                cached_file.relative_path,
                cached_file.video_id,
                serde_json::to_string(&cached_file.file_type).unwrap_or_default(),
                cached_file.format_id,
                cached_file.format_json,
                cached_file.filesize,
                cached_file.mime_type,
                cached_file.cached_at
            ],
        )?;

        Ok(cached_file)
    }

    /// Puts a thumbnail in the cache.
    ///
    /// # Arguments
    ///
    /// * `source_path` - The path to the thumbnail file to cache.
    /// * `filename` - The original filename.
    /// * `video_id` - The ID of the video this thumbnail is associated with.
    /// * `thumbnail` - The thumbnail information.
    ///
    /// # Returns
    ///
    /// Returns the cached file information if successful.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be copied to the cache or the cache entry cannot be written to the database.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "debug", skip(thumbnail))
    )]
    pub async fn put_thumbnail(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        thumbnail: &Thumbnail,
    ) -> Result<CachedFile> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching thumbnail {:?} for video {}", source_path, video_id);

        // Calculate the file hash
        let file_hash = Self::calculate_file_hash(&source_path).await?;

        // Get file metadata
        let metadata = tokio::fs::metadata(&source_path).await?;
        let filesize = metadata.len();

        // Determine the MIME type
        let mime_type = Self::determine_mime_type(&source_path);

        // Create the destination path
        let filename_str = filename.as_ref();
        let extension = Path::new(filename_str)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let relative_path = format!("files/{}.{}", file_hash, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        // Copy the file to the cache directory
        if !dest_path.exists() {
            tokio::fs::copy(&source_path, &dest_path).await?;
        }

        // Serialize thumbnail information
        let thumbnail_json = serde_json::to_string(thumbnail).unwrap_or_default();

        // Create the cache entry
        let cached_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cached_file = CachedFile {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id: Some(video_id),
            file_type: CachedType::Thumbnail,
            format_id: Some("thumbnail".to_string()),
            format_json: Some(thumbnail_json),
            filesize,
            mime_type,
            cached_at,
        };

        // Store in the database
        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT OR REPLACE INTO files (id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                cached_file.id,
                cached_file.filename,
                cached_file.relative_path,
                cached_file.video_id,
                serde_json::to_string(&cached_file.file_type).unwrap_or_default(),
                cached_file.format_id,
                cached_file.format_json,
                cached_file.filesize,
                cached_file.mime_type,
                cached_file.cached_at
            ],
        )?;

        Ok(cached_file)
    }

    /// Gets a file from the cache by its hash.
    ///
    /// # Arguments
    ///
    /// * `file_hash` - The SHA-256 hash of the file.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if the file is in the cache and has not expired, otherwise `None`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn get_by_hash(&self, file_hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for file in cache by hash: {}", file_hash);

        let connection = self.connection.lock().unwrap();

        let mut stmt = connection
            .prepare("SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at FROM files WHERE id = ?")
            .ok()?;

        let mut rows = stmt.query(params![file_hash]).ok()?;

        if let Some(row) = rows.next().ok()? {
            // Check if the cache has expired
            let cached_at: u64 = row.get(9).ok()?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let file_type_str: String = row.get(4).ok()?;
                let file_type: CachedType =
                    serde_json::from_str(&file_type_str).unwrap_or(CachedType::Other);

                let cached_file = CachedFile {
                    id: row.get(0).ok()?,
                    filename: row.get(1).ok()?,
                    relative_path: row.get(2).ok()?,
                    video_id: row.get(3).ok()?,
                    file_type,
                    format_id: row.get(5).ok()?,
                    format_json: row.get(6).ok()?,
                    filesize: row.get(7).ok()?,
                    mime_type: row.get(8).ok()?,
                    cached_at,
                };

                let file_path = self.cache_dir.join(&cached_file.relative_path);

                // Verify the file exists
                if file_path.exists() {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Cache hit for file hash: {}", file_hash);

                    return Some((cached_file, file_path));
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache expired for file hash: {}", file_hash);
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache miss for file hash: {}", file_hash);
        }

        None
    }

    /// Gets a file from the cache by video ID and format ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The ID of the video.
    /// * `format_id` - The ID of the format.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if the file is in the cache and has not expired, otherwise `None`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn get_by_video_and_format(
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

        let connection = self.connection.lock().unwrap();

        let mut stmt = connection
            .prepare("SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at FROM files WHERE video_id = ? AND format_id = ?")
            .ok()?;

        let mut rows = stmt.query(params![video_id, format_id]).ok()?;

        if let Some(row) = rows.next().ok()? {
            // Check if the cache has expired
            let cached_at: u64 = row.get(9).ok()?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let file_type_str: String = row.get(4).ok()?;
                let file_type: CachedType =
                    serde_json::from_str(&file_type_str).unwrap_or(CachedType::Other);

                let cached_file = CachedFile {
                    id: row.get(0).ok()?,
                    filename: row.get(1).ok()?,
                    relative_path: row.get(2).ok()?,
                    video_id: row.get(3).ok()?,
                    file_type,
                    format_id: row.get(5).ok()?,
                    format_json: row.get(6).ok()?,
                    filesize: row.get(7).ok()?,
                    mime_type: row.get(8).ok()?,
                    cached_at,
                };

                let file_path = self.cache_dir.join(&cached_file.relative_path);

                // Verify the file exists
                if file_path.exists() {
                    #[cfg(feature = "tracing")]
                    tracing::debug!(
                        "Cache hit for video ID: {} and format ID: {}",
                        video_id,
                        format_id
                    );

                    return Some((cached_file, file_path));
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Cache expired for video ID: {} and format ID: {}",
                    video_id,
                    format_id
                );
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Cache miss for video ID: {} and format ID: {}",
                video_id,
                format_id
            );
        }

        None
    }

    /// Gets a thumbnail from the cache by video ID.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The ID of the video.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if the thumbnail is in the cache and has not expired, otherwise `None`.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub fn get_thumbnail_by_video_id(&self, video_id: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for thumbnail in cache by video ID: {}", video_id);

        let connection = self.connection.lock().unwrap();

        let file_type_json = serde_json::to_string(&CachedType::Thumbnail).ok()?;

        let mut stmt = connection
            .prepare("SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, filesize, mime_type, cached_at FROM files WHERE video_id = ? AND file_type = ? AND format_id = 'thumbnail'")
            .ok()?;

        let mut rows = stmt.query(params![video_id, file_type_json]).ok()?;

        if let Some(row) = rows.next().ok()? {
            // Check if the cache has expired
            let cached_at: u64 = row.get(9).ok()?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let file_type_str: String = row.get(4).ok()?;
                let file_type: CachedType =
                    serde_json::from_str(&file_type_str).unwrap_or(CachedType::Other);

                let cached_file = CachedFile {
                    id: row.get(0).ok()?,
                    filename: row.get(1).ok()?,
                    relative_path: row.get(2).ok()?,
                    video_id: row.get(3).ok()?,
                    file_type,
                    format_id: row.get(5).ok()?,
                    format_json: row.get(6).ok()?,
                    filesize: row.get(7).ok()?,
                    mime_type: row.get(8).ok()?,
                    cached_at,
                };

                let file_path = self.cache_dir.join(&cached_file.relative_path);

                // Verify the file exists
                if file_path.exists() {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Cache hit for thumbnail of video ID: {}", video_id);

                    return Some((cached_file, file_path));
                }
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache expired for thumbnail of video ID: {}", video_id);
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache miss for thumbnail of video ID: {}", video_id);
        }

        None
    }

    /// Removes a file from the cache.
    ///
    /// # Arguments
    ///
    /// * `file_hash` - The SHA-256 hash of the file to remove.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be removed from the cache or the cache entry cannot be removed from the database.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub async fn remove_file(&self, file_hash: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Removing file from cache: {}", file_hash);

        // Récupérer le chemin relatif et supprimer l'entrée de la base de données dans un bloc
        // pour libérer le MutexGuard avant d'appeler await
        let relative_path = {
            let connection = self.connection.lock().unwrap();

            // Get the file path
            let mut stmt = connection
                .prepare("SELECT relative_path FROM files WHERE id = ?")
                .unwrap();

            let relative_path: Option<String> =
                stmt.query_row(params![file_hash], |row| row.get(0)).ok();

            // Delete from database
            connection.execute("DELETE FROM files WHERE id = ?", params![file_hash])?;

            relative_path
        };

        // Delete the file if it exists
        if let Some(path) = relative_path {
            let file_path = self.cache_dir.join(path);
            if file_path.exists() {
                tokio::fs::remove_file(file_path).await?;
            }
        }

        Ok(())
    }

    /// Cleans the cache by removing expired entries.
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache entries cannot be removed from the database or the files cannot be deleted.
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug"))]
    pub async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning download cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let connection = self.connection.lock().unwrap();

        // Get all expired files
        let mut stmt = connection
            .prepare("SELECT id, relative_path FROM files WHERE cached_at < ?")
            .unwrap();

        let expired_files: Vec<(String, String)> = stmt
            .query_map(params![now - self.ttl], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, rusqlite::Error>>()?;

        // Delete expired files from the filesystem
        for (_, relative_path) in &expired_files {
            let file_path = self.cache_dir.join(relative_path);
            if file_path.exists() {
                if let Err(_e) = fs::remove_file(&file_path) {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(
                        "Failed to delete cached file {}: {}",
                        file_path.display(),
                        _e
                    );
                }
            }
        }

        // Delete expired entries from the database
        connection.execute(
            "DELETE FROM files WHERE cached_at < ?",
            params![now - self.ttl],
        )?;

        Ok(())
    }
}
