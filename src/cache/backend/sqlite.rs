//! SQLite backend implementation for video and file caching.
//!
//! This module provides async-safe SQLite implementations using sqlx.

use super::{FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::utils::current_timestamp;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

async fn create_pool(cache_dir: &Path, db_name: &str) -> Result<SqlitePool> {
    if !cache_dir.exists() {
        tokio::fs::create_dir_all(cache_dir).await?;
    }

    let db_path = cache_dir.join(db_name);

    let connection_options =
        SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
            .map_err(|e| crate::error::Error::database("Create connection options", e))?
            .create_if_missing(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await
        .map_err(|e| crate::error::Error::database("Create connection pool", e))
}

#[cfg(feature = "cache-backend")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

/// SQLite-backed video cache implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::cache::backend::sqlite::SqliteVideoCache;
/// use yt_dlp::cache::backend::VideoBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = SqliteVideoCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SqliteVideoCache {
    pool: SqlitePool,
    ttl: i64,
}

impl SqliteVideoCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "⚙️ Creating new SQLite video cache"
        );

        let pool = create_pool(&cache_dir, "video_cache.db").await?;

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
        .map_err(|e| crate::error::Error::database("Create videos table", e))?;

        // Create an index on the URL for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_videos_url ON videos(url)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::database("Create videos URL index", e))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(24 * 60 * 60) as i64,
        })
    }
}

impl VideoBackend for SqliteVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(
            url = url,
            ttl = self.ttl,
            "🔍 Looking for video in SQLite cache by URL"
        );

        let now = current_timestamp();

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
        .map_err(|e| crate::error::Error::database("Fetch video by URL", e))?;

        match cached {
            Some(cv) => {
                tracing::debug!(
                    url = url,
                    video_id = cv.id,
                    video_title = cv.title,
                    "✅ Cache hit for video"
                );

                Ok(Some(cv.video()?))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(
            url = url,
            video_id = video.id,
            video_title = video.title,
            "⚙️ Caching video to SQLite backend"
        );

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
        .map_err(|e| crate::error::Error::database("Insert video", e))?;

        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Removing video from SQLite cache");

        sqlx::query("DELETE FROM videos WHERE url = ?")
            .bind(url)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Delete video", e))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        tracing::debug!(ttl = self.ttl, "⚙️ Cleaning SQLite video cache");

        let now = current_timestamp();

        let cutoff = now - self.ttl;

        sqlx::query("DELETE FROM videos WHERE cached_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Clean video cache", e))?;

        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(
            video_id = id,
            ttl = self.ttl,
            "🔍 Looking for video in SQLite cache by ID"
        );

        let now = current_timestamp();

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
        .map_err(|e| crate::error::Error::database("Fetch video by ID", e))?;

        match cached {
            Some(cv) => {
                tracing::debug!(
                    video_id = id,
                    video_title = cv.title,
                    "✅ Cache hit for video ID"
                );

                Ok(cv)
            }
            None => Err(crate::error::Error::Unknown(format!(
                "Video with ID {} not found or expired in cache",
                id
            ))),
        }
    }
}

/// SQLite-backed playlist cache implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::cache::backend::sqlite::SqlitePlaylistCache;
/// use yt_dlp::cache::backend::PlaylistBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = SqlitePlaylistCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SqlitePlaylistCache {
    pool: SqlitePool,
    ttl: i64,
}

impl SqlitePlaylistCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "⚙️ Creating new SQLite playlist cache"
        );

        let pool = create_pool(&cache_dir, "playlist_cache.db").await?;

        // Initialize the database schema
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS playlist_cache (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                playlist_json TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| crate::error::Error::database("Create playlist_cache table", e))?;

        // Create an index on the URL
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_playlist_url ON playlist_cache(url)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::database("Create playlist URL index", e))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(6 * 60 * 60) as i64, // 6 hours default
        })
    }
}

impl PlaylistBackend for SqlitePlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(
            url = url,
            ttl = self.ttl,
            "🔍 Looking for playlist in SQLite cache by URL"
        );

        let now = current_timestamp();

        let cached = sqlx::query_as::<_, CachedPlaylist>(
            "SELECT id, title, url, playlist_json, cached_at
             FROM playlist_cache
             WHERE url = ? AND cached_at > ?",
        )
        .bind(url)
        .bind(now - self.ttl)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Fetch playlist by URL", e))?;

        match cached {
            Some(cp) => Ok(Some(cp.playlist()?)),
            None => Ok(None),
        }
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(
            playlist_id = id,
            ttl = self.ttl,
            "🔍 Looking for playlist in SQLite cache by ID"
        );

        let now = current_timestamp();

        let cached = sqlx::query_as::<_, CachedPlaylist>(
            "SELECT id, title, url, playlist_json, cached_at
             FROM playlist_cache
             WHERE id = ? AND cached_at > ?",
        )
        .bind(id)
        .bind(now - self.ttl)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Fetch playlist by ID", e))?;

        match cached {
            Some(cp) => Ok(Some(cp.playlist()?)),
            None => Ok(None),
        }
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            playlist_title = playlist.title,
            entry_count = playlist.entries.len(),
            "⚙️ Caching playlist to SQLite backend"
        );

        let cached = CachedPlaylist::from((url, playlist));

        sqlx::query(
            "INSERT OR REPLACE INTO playlist_cache (id, title, url, playlist_json, cached_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&cached.id)
        .bind(&cached.title)
        .bind(&cached.url)
        .bind(&cached.playlist_json)
        .bind(cached.cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Insert playlist", e))?;

        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in SQLite cache");
        sqlx::query("DELETE FROM playlist_cache WHERE url = ?")
            .bind(url)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Delete playlist", e))?;
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        tracing::debug!(ttl = self.ttl, "⚙️ Cleaning SQLite playlist cache");
        let now = current_timestamp();

        sqlx::query("DELETE FROM playlist_cache WHERE cached_at < ?")
            .bind(now - self.ttl)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Clean playlist cache", e))?;
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from SQLite cache");

        sqlx::query("DELETE FROM playlist_cache")
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Clear playlist cache", e))?;
        Ok(())
    }
}

/// SQLite-backed file cache implementation.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::cache::backend::sqlite::SqliteFileCache;
/// use yt_dlp::cache::backend::FileBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = SqliteFileCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SqliteFileCache {
    pool: SqlitePool,
    ttl: i64,
    cache_dir: PathBuf,
}

impl SqliteFileCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "⚙️ Creating new SQLite file cache"
        );

        let pool = create_pool(&cache_dir, "file_cache.db").await?;

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
                language_code TEXT,
                cached_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| crate::error::Error::database("Create files table", e))?;

        // Create indices for faster lookups
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_video_id ON files(video_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::database("Create files video_id index", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_format_id ON files(format_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::database("Create files format_id index", e))?;

        // Initialize the database schema for thumbnails
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
        .map_err(|e| crate::error::Error::database("Create thumbnails table", e))?;

        // Create index for thumbnails
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_thumbnails_video_id ON thumbnails(video_id)")
            .execute(&pool)
            .await
            .map_err(|e| crate::error::Error::database("Create thumbnails video_id index", e))?;

        Ok(Self {
            pool,
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60) as i64, // 7 days default
            cache_dir,
        })
    }
}

impl FileBackend for SqliteFileCache {
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            hash = hash,
            ttl = self.ttl,
            "🔍 Looking for file in SQLite cache by hash"
        );

        let now = current_timestamp();

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at
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
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            ttl = self.ttl,
            "🔍 Looking for file in SQLite cache by video and format"
        );

        let now = current_timestamp();

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at
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

    #[cfg(feature = "cache-backend")]
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            video_quality = ?video_quality,
            audio_quality = ?audio_quality,
            video_codec = ?video_codec,
            audio_codec = ?audio_codec,
            ttl = self.ttl,
            "🔍 Looking for file in SQLite cache by preferences"
        );

        let now = current_timestamp();

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at
             FROM files
             WHERE video_id = ? AND cached_at > ?",
        )
        .bind(video_id)
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .ok()?;

        result.into_iter().find_map(|cached| {
            if cached.matches_preferences(
                video_quality,
                audio_quality,
                video_codec.clone(),
                audio_codec.clone(),
            ) {
                let path = self.cache_dir.join(&cached.relative_path);
                Some((cached, path))
            } else {
                None
            }
        })
    }

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            filename = file.filename,
            file_id = file.id,
            source_path = ?source_path,
            video_id = ?file.video_id,
            format_id = ?file.format_id,
            filesize = file.filesize,
            "⚙️ Caching file to SQLite backend"
        );

        // Write file to disk (copy from source)
        let file_path = self.cache_dir.join(&file.relative_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &file_path).await?; // Changed to copy

        // Store metadata in database
        sqlx::query(
            "INSERT OR REPLACE INTO files
             (id, filename, relative_path, video_id, file_type, format_id, format_json,
              video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(&file.language_code)
        .bind(file.cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Insert cached file", e))?;

        Ok(file_path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        tracing::debug!(file_id = id, "⚙️ Removing file from SQLite cache");

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
            .map_err(|e| crate::error::Error::database("Delete cached file", e))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        tracing::debug!(ttl = self.ttl, "⚙️ Cleaning SQLite file cache");

        let now = current_timestamp();
        let cutoff = now - self.ttl;

        // Get all expired files
        let expired = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at
             FROM files
             WHERE cached_at < ?",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Fetch expired files", e))?;

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
            .map_err(|e| crate::error::Error::database("Clean file cache", e))?;

        // Same for thumbnails
        let expired_thumbnails = sqlx::query_as::<_, CachedThumbnail>(
            "SELECT id, filename, relative_path, video_id, filesize, mime_type, width, height, cached_at
             FROM thumbnails
             WHERE cached_at < ?"
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Fetch expired thumbnails", e))?;

        for thumb in &expired_thumbnails {
            let path = self.cache_dir.join(&thumb.relative_path);
            if path.exists() {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }

        sqlx::query("DELETE FROM thumbnails WHERE cached_at < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::database("Clean thumbnails cache", e))?;

        Ok(())
    }

    async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            ttl = self.ttl,
            "🔍 Looking for thumbnail in SQLite cache by video ID"
        );

        let now = current_timestamp();

        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedThumbnail>(
            "SELECT id, filename, relative_path, video_id, filesize, mime_type, width, height, cached_at
             FROM thumbnails
             WHERE video_id = ? AND cached_at > ?"
        )
        .bind(video_id)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result.map(|cached| {
            let path = self.cache_dir.join(&cached.relative_path);
            (cached, path)
        })
    }

    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &Path,
    ) -> Result<PathBuf> {
        tracing::debug!(
            filename = thumbnail.filename,
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            source_path = ?source_path,
            width = ?thumbnail.width,
            height = ?thumbnail.height,
            "⚙️ Caching thumbnail to SQLite backend"
        );

        let file_path = self.cache_dir.join(&thumbnail.relative_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &file_path).await?; // Changed to copy

        sqlx::query(
            "INSERT OR REPLACE INTO thumbnails (id, filename, relative_path, video_id, filesize, mime_type, width, height, cached_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&thumbnail.id)
        .bind(&thumbnail.filename)
        .bind(&thumbnail.relative_path)
        .bind(&thumbnail.video_id)
        .bind(thumbnail.filesize)
        .bind(&thumbnail.mime_type)
        .bind(thumbnail.width)
        .bind(thumbnail.height)
        .bind(thumbnail.cached_at)
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::database("Insert thumbnail", e))?;

        Ok(file_path)
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            ttl = self.ttl,
            "🔍 Looking for subtitle in SQLite cache by video ID and language"
        );

        let now = current_timestamp();
        let cutoff = now - self.ttl;

        let result = sqlx::query_as::<_, CachedFile>(
            "SELECT id, filename, relative_path, video_id, file_type, format_id, format_json,
                    video_quality, audio_quality, video_codec, audio_codec, filesize, mime_type, language_code, cached_at
             FROM files
             WHERE video_id = ? AND language_code = ? AND cached_at > ?",
        )
        .bind(video_id)
        .bind(language)
        .bind(cutoff)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        result.map(|cached| {
            let path = self.cache_dir.join(&cached.relative_path);
            (cached, path)
        })
    }
}
