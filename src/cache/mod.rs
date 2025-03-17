//! Cache module for storing video metadata and downloaded files.
//!
//! This module provides functionality for caching video metadata and downloaded files
//! to avoid making repeated requests for the same videos and re-downloading the same files.

use crate::error::Result;
use crate::model::Video;
use crate::model::format::Format;
use crate::model::format_selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::model::thumbnail::Thumbnail;
use rusqlite::{Connection, OpenFlags, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Structure for storing video metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// The video quality preference used to select this format (if any).
    pub video_quality: Option<VideoQuality>,
    /// The audio quality preference used to select this format (if any).
    pub audio_quality: Option<AudioQuality>,
    /// The video codec preference used to select this format (if any).
    pub video_codec: Option<VideoCodecPreference>,
    /// The audio codec preference used to select this format (if any).
    pub audio_codec: Option<AudioCodecPreference>,
    /// The file size in bytes.
    pub filesize: u64,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The cache timestamp (Unix timestamp).
    pub cached_at: u64,
}

/// Structure for storing thumbnail metadata in cache.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedThumbnail {
    /// The ID of the thumbnail (SHA-256 hash of the content).
    pub id: String,
    /// The original filename.
    pub filename: String,
    /// The path to the file relative to the cache directory.
    pub relative_path: String,
    /// The video ID this thumbnail is associated with.
    pub video_id: String,
    /// The file size in bytes.
    pub filesize: u64,
    /// The MIME type of the file.
    pub mime_type: String,
    /// The width of the thumbnail in pixels (if available).
    pub width: Option<u32>,
    /// The height of the thumbnail in pixels (if available).
    pub height: Option<u32>,
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

// Implementation of the Display trait for CachedVideo
impl fmt::Display for CachedVideo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CachedVideo(id={}, title=\"{}\", cached_at={})",
            self.id, self.title, self.cached_at
        )
    }
}

// Implementation of Eq for CachedVideo
impl Eq for CachedVideo {}

// Implementation of Hash for CachedVideo
impl Hash for CachedVideo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.url.hash(state);
        self.cached_at.hash(state);
    }
}

// Implementation of the Display trait for CachedFile
impl fmt::Display for CachedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CachedFile(id={}, filename=\"{}\", type={:?}, size={})",
            self.id, self.filename, self.file_type, self.filesize
        )
    }
}

// Implementation of Eq for CachedFile
impl Eq for CachedFile {}

// Implementation of Hash for CachedFile
impl Hash for CachedFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.filename.hash(state);
        self.relative_path.hash(state);
        self.video_id.hash(state);
        std::mem::discriminant(&self.file_type).hash(state);
    }
}

// Implementation of the Display trait for CachedType
impl fmt::Display for CachedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CachedType::Format => write!(f, "Format"),
            CachedType::Thumbnail => write!(f, "Thumbnail"),
            CachedType::Other => write!(f, "Other"),
        }
    }
}

// Implementation of Eq for CachedType
impl Eq for CachedType {}

// Implementation of Hash for CachedType
impl Hash for CachedType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

impl fmt::Display for CachedThumbnail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CachedThumbnail(id={}, video_id={}, filename={})",
            self.id, self.video_id, self.filename
        )
    }
}

impl PartialOrd for CachedThumbnail {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

impl Ord for CachedThumbnail {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl Eq for CachedThumbnail {}

impl Hash for CachedThumbnail {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
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
    pub fn get(&self, url: &str) -> Option<Video> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for video in cache: {}", url);

        let connection = self.connection.lock().unwrap();

        // Look up by URL
        let mut stmt = match connection
            .prepare("SELECT id, title, url, video_json, cached_at FROM videos WHERE url = ?")
        {
            Ok(stmt) => stmt,
            Err(_) => return None,
        };

        let mut rows = match stmt.query(params![url]) {
            Ok(rows) => rows,
            Err(_) => return None,
        };

        if let Ok(Some(row)) = rows.next() {
            // Check if the cache has expired
            let cached_at: u64 = match row.get(4) {
                Ok(cached_at) => cached_at,
                Err(_) => return None,
            };

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let video_json: String = match row.get(3) {
                    Ok(video_json) => video_json,
                    Err(_) => return None,
                };

                let video: Video = match serde_json::from_str(&video_json) {
                    Ok(video) => video,
                    Err(_) => return None,
                };

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

    /// Retrieves a video from the cache by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the video to retrieve.
    ///
    /// # Returns
    ///
    /// Returns the cached video if it exists and has not expired, otherwise an error.
    pub fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for video in cache by ID: {}", id);

        let connection = self.connection.lock().unwrap();

        // Look up by ID
        let mut stmt = connection
            .prepare("SELECT id, title, url, video_json, cached_at FROM videos WHERE id = ?")?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            // Check if the cache has expired
            let cached_at: u64 = row.get(4)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if now - cached_at <= self.ttl {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let url: String = row.get(2)?;
                let video_json: String = row.get(3)?;
                let video: Video = serde_json::from_str(&video_json)?;

                #[cfg(feature = "tracing")]
                tracing::debug!("Cache hit for video ID: {}", id);

                Ok(CachedVideo {
                    id,
                    title,
                    url,
                    video,
                    cached_at,
                })
            } else {
                #[cfg(feature = "tracing")]
                tracing::debug!("Cache expired for video ID: {}", id);
                Err(crate::error::Error::FormatNotFound(format!(
                    "Video with ID {} has expired in cache",
                    id
                )))
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache miss for video ID: {}", id);
            Err(crate::error::Error::FormatNotFound(format!(
                "Video with ID {} not found in cache",
                id
            )))
        }
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
    /// This function will return an error if the cache directory cannot be created or the database connection cannot be established.
    pub fn new(cache_path: impl AsRef<Path> + std::fmt::Debug, ttl: Option<u64>) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating download cache at {:?}", cache_path);

        // Create the cache directory if it doesn't exist
        let cache_dir = cache_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&cache_dir)?;

        // Create the database file
        let db_path = cache_dir.join("downloads.db");
        let connection = Connection::open(&db_path)?;

        // Create the files table if it doesn't exist
        connection.execute(
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
            [],
        )?;

        // Create the thumbnails table if it doesn't exist
        connection.execute(
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
            [],
        )?;

        // Create indexes for faster lookups
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_video_id ON files (video_id)",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_files_format_id ON files (format_id)",
            [],
        )?;
        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_thumbnails_video_id ON thumbnails (video_id)",
            [],
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60), // 7 days by default
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

    /// Puts a file in the cache, with preferences.
    ///
    /// # Arguments
    ///
    /// * `source_path` - The path to the file to cache.
    /// * `filename` - The original filename.
    /// * `video_id` - The ID of the video this file is associated with (if any).
    /// * `format` - The format information (if available).
    /// * `video_quality` - The video quality preference used to select this format (if any).
    /// * `audio_quality` - The audio quality preference used to select this format (if any).
    /// * `video_codec` - The video codec preference used to select this format (if any).
    /// * `audio_codec` - The audio codec preference used to select this format (if any).
    ///
    /// # Returns
    ///
    /// Returns the cached file information if successful.
    ///
    /// # Errors
    ///
    /// This function will return an error if the file cannot be copied to the cache or the cache entry cannot be written to the database.
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
            video_quality,
            audio_quality,
            video_codec,
            audio_codec,
            filesize,
            mime_type,
            cached_at,
        };

        // Store in the database
        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT OR REPLACE INTO files (id, filename, relative_path, video_id, file_type, format_id, format_json, video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                cached_file.id,
                cached_file.filename,
                cached_file.relative_path,
                cached_file.video_id.clone(),
                serde_json::to_string(&cached_file.file_type).unwrap_or_default(),
                cached_file.format_id.clone(),
                cached_file.format_json.clone(),
                cached_file.video_quality.map(|vq| serde_json::to_string(&vq).unwrap_or_default()),
                cached_file.audio_quality.map(|aq| serde_json::to_string(&aq).unwrap_or_default()),
                cached_file.video_codec.clone().map(|vc| serde_json::to_string(&vc).unwrap_or_default()),
                cached_file.audio_codec.clone().map(|ac| serde_json::to_string(&ac).unwrap_or_default()),
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
    pub async fn put_thumbnail(
        &self,
        source_path: impl AsRef<Path> + std::fmt::Debug,
        filename: impl AsRef<str> + std::fmt::Debug,
        video_id: String,
        thumbnail: &Thumbnail,
    ) -> Result<CachedThumbnail> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Caching thumbnail {:?}", source_path);

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

        let relative_path = format!("thumbnails/{}.{}", file_hash, extension);
        let dest_path = self.cache_dir.join(&relative_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Copy the file to the cache directory
        tokio::fs::copy(&source_path, &dest_path).await?;

        // Get current timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create the cached thumbnail
        let cached_thumbnail = CachedThumbnail {
            id: file_hash.clone(),
            filename: filename_str.to_string(),
            relative_path,
            video_id: video_id.clone(),
            filesize,
            mime_type,
            width: thumbnail.width.map(|w| w as u32),
            height: thumbnail.height.map(|h| h as u32),
            cached_at: now,
        };

        // Insert into database
        let connection = self.connection.lock().unwrap();

        // Convert Option<u32> to Option<i32> for SQLite compatibility
        let width_i32 = cached_thumbnail.width.map(|w| w as i32);
        let height_i32 = cached_thumbnail.height.map(|h| h as i32);

        connection.execute(
            "INSERT INTO thumbnails (id, filename, relative_path, video_id, filesize, mime_type, width, height, cached_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                cached_thumbnail.id,
                cached_thumbnail.filename,
                cached_thumbnail.relative_path,
                cached_thumbnail.video_id,
                cached_thumbnail.filesize as i64,
                cached_thumbnail.mime_type,
                width_i32,
                height_i32,
                cached_thumbnail.cached_at as i64
            ],
        )?;

        Ok(cached_thumbnail)
    }

    /// Gets a file from the cache by hash.
    ///
    /// # Arguments
    ///
    /// * `file_hash` - The SHA-256 hash of the file.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if the file is in the cache and has not expired, otherwise `None`.
    pub fn get_by_hash(&self, file_hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Looking for file in cache by hash: {}", file_hash);

        let connection = self.connection.lock().unwrap();

        let mut stmt = match connection
            .prepare("SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at FROM files WHERE id = ?") {
            Ok(stmt) => stmt,
            Err(_) => return None,
        };

        let mut rows = match stmt.query(params![file_hash]) {
            Ok(rows) => rows,
            Err(_) => return None,
        };

        let row = match rows.next() {
            Ok(Some(row)) => row,
            _ => return None,
        };

        // Check if the cache has expired
        let cached_at: u64 = match row.get(13) {
            Ok(cached_at) => cached_at,
            Err(_) => return None,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - cached_at <= self.ttl {
            let file_type_str: String = match row.get(4) {
                Ok(file_type_str) => file_type_str,
                Err(_) => return None,
            };

            let file_type: CachedType = match serde_json::from_str(&file_type_str) {
                Ok(file_type) => file_type,
                Err(_) => CachedType::Other,
            };

            // Parse quality and codec preferences
            let video_quality: Option<VideoQuality> = match row.get::<_, Option<String>>(7) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_quality: Option<AudioQuality> = match row.get::<_, Option<String>>(8) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let video_codec: Option<VideoCodecPreference> = match row.get::<_, Option<String>>(9) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_codec: Option<AudioCodecPreference> = match row.get::<_, Option<String>>(10) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let id: String = match row.get(0) {
                Ok(id) => id,
                Err(_) => return None,
            };

            let filename: String = match row.get(1) {
                Ok(filename) => filename,
                Err(_) => return None,
            };

            let relative_path: String = match row.get(2) {
                Ok(relative_path) => relative_path,
                Err(_) => return None,
            };

            let video_id: Option<String> = match row.get(3) {
                Ok(video_id) => video_id,
                Err(_) => return None,
            };

            let format_id: Option<String> = match row.get(5) {
                Ok(format_id) => format_id,
                Err(_) => return None,
            };

            let format_json: Option<String> = match row.get(6) {
                Ok(format_json) => format_json,
                Err(_) => return None,
            };

            let filesize: u64 = match row.get(11) {
                Ok(filesize) => filesize,
                Err(_) => return None,
            };

            let mime_type: String = match row.get(12) {
                Ok(mime_type) => mime_type,
                Err(_) => return None,
            };

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
                filesize,
                mime_type,
                cached_at,
            };

            let file_path = self.cache_dir.join(&cached_file.relative_path);

            // Verify the file exists
            if file_path.exists() {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Cache hit for video ID: {} and format ID: {}",
                    cached_file
                        .video_id
                        .as_ref()
                        .unwrap_or(&String::from("unknown")),
                    cached_file
                        .format_id
                        .as_ref()
                        .unwrap_or(&String::from("unknown"))
                );

                return Some((cached_file, file_path));
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache expired for file with hash: {}", file_hash);
        }

        #[cfg(feature = "tracing")]
        tracing::debug!("File not found in cache with hash: {}", file_hash);

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

        let mut stmt = match connection
            .prepare("SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at FROM files WHERE video_id = ? AND format_id = ?") {
            Ok(stmt) => stmt,
            Err(_) => return None,
        };

        let mut rows = match stmt.query(params![video_id, format_id]) {
            Ok(rows) => rows,
            Err(_) => return None,
        };

        let row = match rows.next() {
            Ok(Some(row)) => row,
            _ => return None,
        };

        // Check if the cache has expired
        let cached_at: u64 = match row.get(13) {
            Ok(cached_at) => cached_at,
            Err(_) => return None,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - cached_at <= self.ttl {
            let file_type_str: String = match row.get(4) {
                Ok(file_type_str) => file_type_str,
                Err(_) => return None,
            };

            let file_type: CachedType = match serde_json::from_str(&file_type_str) {
                Ok(file_type) => file_type,
                Err(_) => CachedType::Other,
            };

            // Parse quality and codec preferences
            let video_quality: Option<VideoQuality> = match row.get::<_, Option<String>>(7) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_quality: Option<AudioQuality> = match row.get::<_, Option<String>>(8) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let video_codec: Option<VideoCodecPreference> = match row.get::<_, Option<String>>(9) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_codec: Option<AudioCodecPreference> = match row.get::<_, Option<String>>(10) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let id: String = match row.get(0) {
                Ok(id) => id,
                Err(_) => return None,
            };

            let filename: String = match row.get(1) {
                Ok(filename) => filename,
                Err(_) => return None,
            };

            let relative_path: String = match row.get(2) {
                Ok(relative_path) => relative_path,
                Err(_) => return None,
            };

            let row_video_id: Option<String> = match row.get(3) {
                Ok(video_id) => video_id,
                Err(_) => return None,
            };

            let row_format_id: Option<String> = match row.get(5) {
                Ok(format_id) => format_id,
                Err(_) => return None,
            };

            let format_json: Option<String> = match row.get(6) {
                Ok(format_json) => format_json,
                Err(_) => return None,
            };

            let filesize: u64 = match row.get(11) {
                Ok(filesize) => filesize,
                Err(_) => return None,
            };

            let mime_type: String = match row.get(12) {
                Ok(mime_type) => mime_type,
                Err(_) => return None,
            };

            let cached_file = CachedFile {
                id,
                filename,
                relative_path,
                video_id: row_video_id,
                file_type,
                format_id: row_format_id,
                format_json,
                video_quality,
                audio_quality,
                video_codec,
                audio_codec,
                filesize,
                mime_type,
                cached_at,
            };

            let file_path = self.cache_dir.join(&cached_file.relative_path);

            // Verify the file exists
            if file_path.exists() {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Cache hit for video ID: {} and format ID: {}",
                    cached_file
                        .video_id
                        .as_ref()
                        .unwrap_or(&String::from("unknown")),
                    cached_file
                        .format_id
                        .as_ref()
                        .unwrap_or(&String::from("unknown"))
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

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Cache miss for video ID: {} and format ID: {}",
            video_id,
            format_id
        );

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
                    video_quality: None,
                    audio_quality: None,
                    video_codec: None,
                    audio_codec: None,
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
    pub async fn remove_file(&self, file_hash: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Removing file from cache: {}", file_hash);

        // Get the relative path and remove the database entry in a block
        // to release the MutexGuard before calling await
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
    pub async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Cleaning download cache");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let connection = self.connection.lock().unwrap();

        // Get all expired files
        let mut stmt =
            connection.prepare("SELECT id, relative_path FROM files WHERE cached_at < ?")?;

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

    /// Gets a file from the cache by video ID and format preferences.
    ///
    /// # Arguments
    ///
    /// * `video_id` - The ID of the video.
    /// * `video_quality` - The video quality preference.
    /// * `audio_quality` - The audio quality preference.
    /// * `video_codec` - The video codec preference.
    /// * `audio_codec` - The audio codec preference.
    ///
    /// # Returns
    ///
    /// Returns the cached file information and path if the file is in the cache and has not expired, otherwise `None`.
    pub fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Looking for file in cache by video ID: {} and format preferences",
            video_id
        );

        let connection = self.connection.lock().unwrap();

        // Build the query based on which preferences are provided
        let mut query = "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json, video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, cached_at FROM files WHERE video_id = ?".to_string();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(video_id.to_string())];

        if let Some(vq) = &video_quality {
            query.push_str(" AND video_quality = ?");
            params_vec.push(Box::new(serde_json::to_string(vq).unwrap_or_default()));
        }

        if let Some(aq) = &audio_quality {
            query.push_str(" AND audio_quality = ?");
            params_vec.push(Box::new(serde_json::to_string(aq).unwrap_or_default()));
        }

        if let Some(vc) = &video_codec {
            query.push_str(" AND video_codec = ?");
            params_vec.push(Box::new(serde_json::to_string(vc).unwrap_or_default()));
        }

        if let Some(ac) = &audio_codec {
            query.push_str(" AND audio_codec = ?");
            params_vec.push(Box::new(serde_json::to_string(ac).unwrap_or_default()));
        }

        let mut stmt = match connection.prepare(&query) {
            Ok(stmt) => stmt,
            Err(_) => return None,
        };

        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut rows = match stmt.query(params_slice.as_slice()) {
            Ok(rows) => rows,
            Err(_) => return None,
        };

        let row = match rows.next() {
            Ok(Some(row)) => row,
            _ => return None,
        };

        // Check if the cache has expired
        let cached_at: u64 = match row.get(13) {
            Ok(cached_at) => cached_at,
            Err(_) => return None,
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now - cached_at <= self.ttl {
            let file_type_str: String = match row.get(4) {
                Ok(file_type_str) => file_type_str,
                Err(_) => return None,
            };

            let file_type: CachedType = match serde_json::from_str(&file_type_str) {
                Ok(file_type) => file_type,
                Err(_) => CachedType::Other,
            };

            // Parse quality and codec preferences
            let video_quality: Option<VideoQuality> = match row.get::<_, Option<String>>(7) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_quality: Option<AudioQuality> = match row.get::<_, Option<String>>(8) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let video_codec: Option<VideoCodecPreference> = match row.get::<_, Option<String>>(9) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let audio_codec: Option<AudioCodecPreference> = match row.get::<_, Option<String>>(10) {
                Ok(opt_str) => opt_str.and_then(|s| serde_json::from_str(&s).ok()),
                Err(_) => None,
            };

            let id: String = match row.get(0) {
                Ok(id) => id,
                Err(_) => return None,
            };

            let filename: String = match row.get(1) {
                Ok(filename) => filename,
                Err(_) => return None,
            };

            let relative_path: String = match row.get(2) {
                Ok(relative_path) => relative_path,
                Err(_) => return None,
            };

            let row_video_id: Option<String> = match row.get(3) {
                Ok(video_id) => video_id,
                Err(_) => return None,
            };

            let row_format_id: Option<String> = match row.get(5) {
                Ok(format_id) => format_id,
                Err(_) => return None,
            };

            let format_json: Option<String> = match row.get(6) {
                Ok(format_json) => format_json,
                Err(_) => return None,
            };

            let filesize: u64 = match row.get(11) {
                Ok(filesize) => filesize,
                Err(_) => return None,
            };

            let mime_type: String = match row.get(12) {
                Ok(mime_type) => mime_type,
                Err(_) => return None,
            };

            let cached_file = CachedFile {
                id,
                filename,
                relative_path,
                video_id: row_video_id,
                file_type,
                format_id: row_format_id,
                format_json,
                video_quality,
                audio_quality,
                video_codec,
                audio_codec,
                filesize,
                mime_type,
                cached_at,
            };

            let file_path = self.cache_dir.join(&cached_file.relative_path);

            // Verify the file exists
            if file_path.exists() {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Cache hit for video ID: {} and format preferences",
                    cached_file
                        .video_id
                        .as_ref()
                        .unwrap_or(&String::from("unknown"))
                );

                return Some((cached_file, file_path));
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Cache expired for video ID: {} and format preferences",
                video_id
            );
        }

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Cache miss for video ID: {} and format preferences",
            video_id
        );

        None
    }
}
