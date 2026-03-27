//! Redis cache backend for distributed/server deployments.
//!
//! This module provides a persistent cache backed by Redis, suitable for multi-process or
//! distributed environments. Redis handles TTL expiration natively via `SETEX`.
//! File content is NOT stored in Redis — only metadata. Files are stored on disk.

use std::path::{Path, PathBuf};

use redis::AsyncCommands;

use super::{
    DEFAULT_FILE_TTL, DEFAULT_PLAYLIST_TTL, DEFAULT_VIDEO_TTL, FileBackend, PlaylistBackend, VideoBackend,
    copy_to_cache, url_hash,
};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;

const PREFIX_VIDEO: &str = "yt-dlp:video:";
const PREFIX_VIDEO_ID: &str = "yt-dlp:video_id:";
const PREFIX_PLAYLIST: &str = "yt-dlp:playlist:";
const PREFIX_PLAYLIST_ID: &str = "yt-dlp:playlist_id:";
const PREFIX_FILE: &str = "yt-dlp:file:";
const PREFIX_THUMBNAIL: &str = "yt-dlp:thumbnail:";

async fn get_redis_connection(client: &redis::Client) -> Result<redis::aio::MultiplexedConnection> {
    client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| crate::error::Error::redis("get connection", e))
}

fn prefixed_url_key(prefix: &str, url: &str) -> String {
    format!("{}{}", prefix, url_hash(url))
}

fn prefixed_id_key(prefix: &str, id: &str) -> String {
    format!("{}{}", prefix, id)
}

/// Redis-backed video cache.
#[derive(Debug, Clone)]
pub struct RedisVideoCache {
    client: redis::Client,
    ttl: u64,
}

impl RedisVideoCache {
    /// Creates a new Redis video cache.
    pub async fn new(redis_url: impl Into<String>, ttl: Option<u64>) -> Result<Self> {
        let url = redis_url.into();

        tracing::debug!(redis_url = url, "🔧 Connecting to Redis for video cache");

        let client = redis::Client::open(url.as_str()).map_err(|e| crate::error::Error::redis("connect", e))?;

        // Test connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| crate::error::Error::redis("connect", e))?;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| crate::error::Error::redis("ping", e))?;

        Ok(Self {
            client,
            ttl: ttl.unwrap_or(DEFAULT_VIDEO_TTL),
        })
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection> {
        get_redis_connection(&self.client).await
    }
}

impl VideoBackend for RedisVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(url = url, "🔍 Looking for video in Redis cache by URL");

        let mut conn = self.conn().await?;
        let key = prefixed_url_key(PREFIX_VIDEO, url);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get video", e))?;

        if let Some(bytes) = data {
            let cached: CachedVideo = serde_json::from_slice(&bytes)?;
            return Ok(Some(cached.video()?));
        }

        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(url = url, video_id = video.id, "⚙️ Caching video to Redis backend");

        let mut conn = self.conn().await?;
        let cached = CachedVideo::new(url.clone(), &video)?;
        let bytes = serde_json::to_vec(&cached)?;

        let url_k = prefixed_url_key(PREFIX_VIDEO, &url);
        let id_k = prefixed_id_key(PREFIX_VIDEO_ID, &cached.id);

        // Store by URL and by ID atomically with TTL via pipeline
        redis::pipe()
            .atomic()
            .set_ex(&url_k, &bytes, self.ttl)
            .set_ex(&id_k, &bytes, self.ttl)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| crate::error::Error::redis("pipeline set video", e))?;

        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Removing video from Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_url_key(PREFIX_VIDEO, url);

        // Try to get the video ID for cleanup
        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get video for remove", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedVideo>(&bytes)
        {
            let id_k = prefixed_id_key(PREFIX_VIDEO_ID, &cached.id);
            conn.del::<_, ()>(&id_k)
                .await
                .map_err(|e| crate::error::Error::redis("del video by id", e))?;
        }

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| crate::error::Error::redis("del video by url", e))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        // Redis handles TTL expiration natively — nothing to do
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, "🔍 Looking up video by ID in Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_id_key(PREFIX_VIDEO_ID, id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get video by id", e))?;

        if let Some(bytes) = data {
            let cached: CachedVideo = serde_json::from_slice(&bytes)?;
            return Ok(cached);
        }

        Err(crate::error::Error::cache_miss(format!("video:{}", id)))
    }
}

/// Redis-backed playlist cache.
#[derive(Debug, Clone)]
pub struct RedisPlaylistCache {
    client: redis::Client,
    ttl: u64,
}

impl RedisPlaylistCache {
    /// Creates a new Redis playlist cache.
    pub async fn new(redis_url: impl Into<String>, ttl: Option<u64>) -> Result<Self> {
        let url = redis_url.into();
        let client = redis::Client::open(url.as_str()).map_err(|e| crate::error::Error::redis("connect", e))?;

        Ok(Self {
            client,
            ttl: ttl.unwrap_or(DEFAULT_PLAYLIST_TTL),
        })
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection> {
        get_redis_connection(&self.client).await
    }
}

impl PlaylistBackend for RedisPlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(url = url, "🔍 Looking for playlist in Redis cache by URL");

        let mut conn = self.conn().await?;
        let key = prefixed_url_key(PREFIX_PLAYLIST, url);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get playlist", e))?;

        if let Some(bytes) = data {
            let cached: CachedPlaylist = serde_json::from_slice(&bytes)?;
            return Ok(Some(cached.playlist()?));
        }

        Ok(None)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(playlist_id = id, "🔍 Looking up playlist by ID in Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_id_key(PREFIX_PLAYLIST_ID, id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get playlist by id", e))?;

        if let Some(bytes) = data {
            let cached: CachedPlaylist = serde_json::from_slice(&bytes)?;
            return Ok(Some(cached.playlist()?));
        }

        Ok(None)
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            "⚙️ Caching playlist to Redis backend"
        );

        let mut conn = self.conn().await?;
        let cached = CachedPlaylist::from((url.clone(), playlist));
        let bytes = serde_json::to_vec(&cached)?;

        let url_k = prefixed_url_key(PREFIX_PLAYLIST, &url);
        let id_k = prefixed_id_key(PREFIX_PLAYLIST_ID, &cached.id);

        // Store by URL and by ID atomically with TTL via pipeline
        redis::pipe()
            .atomic()
            .set_ex(&url_k, &bytes, self.ttl)
            .set_ex(&id_k, &bytes, self.ttl)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| crate::error::Error::redis("pipeline set playlist", e))?;

        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_url_key(PREFIX_PLAYLIST, url);

        // Get cached to remove ID key too
        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get playlist for invalidate", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedPlaylist>(&bytes)
        {
            let id_k = prefixed_id_key(PREFIX_PLAYLIST_ID, &cached.id);
            conn.del::<_, ()>(&id_k)
                .await
                .map_err(|e| crate::error::Error::redis("del playlist by id", e))?;
        }

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| crate::error::Error::redis("del playlist by url", e))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        // Redis handles TTL expiration natively
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from Redis cache");

        let mut conn = self.conn().await?;

        // Cursor-based scan for all playlist URL keys
        let pattern = format!("{}*", PREFIX_PLAYLIST);
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| crate::error::Error::redis("scan playlist keys", e))?;
            keys.extend(batch);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        let pattern_id = format!("{}*", PREFIX_PLAYLIST_ID);
        let mut keys_id: Vec<String> = Vec::new();
        cursor = 0;
        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern_id)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| crate::error::Error::redis("scan playlist id keys", e))?;
            keys_id.extend(batch);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        let all_keys: Vec<&str> = keys.iter().chain(keys_id.iter()).map(|s| s.as_str()).collect();
        if !all_keys.is_empty() {
            conn.del::<_, ()>(all_keys)
                .await
                .map_err(|e| crate::error::Error::redis("del all playlist keys", e))?;
        }

        Ok(())
    }
}

/// Redis-backed file cache.
///
/// File content is stored on disk; Redis stores only metadata and the path.
#[derive(Debug, Clone)]
pub struct RedisFileCache {
    client: redis::Client,
    cache_dir: PathBuf,
    ttl: u64,
}

impl RedisFileCache {
    /// Creates a new Redis file cache.
    pub async fn new(redis_url: impl Into<String>, cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let url = redis_url.into();
        let client = redis::Client::open(url.as_str()).map_err(|e| crate::error::Error::redis("connect", e))?;

        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        Ok(Self {
            client,
            cache_dir,
            ttl: ttl.unwrap_or(DEFAULT_FILE_TTL),
        })
    }

    async fn conn(&self) -> Result<redis::aio::MultiplexedConnection> {
        get_redis_connection(&self.client).await
    }
}

impl FileBackend for RedisFileCache {
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(hash = hash, "🔍 Looking for file in Redis cache by hash");

        let mut conn = self.conn().await?;
        let key = prefixed_id_key(PREFIX_FILE, hash);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get file by hash", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedFile>(&bytes)
        {
            let path = self.cache_dir.join(&cached.relative_path);
            return Ok(Some((cached, path)));
        }

        Ok(None)
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            "🔍 Looking for file by video and format in Redis cache"
        );

        let mut conn = self.conn().await?;
        let key = format!("{}vf:{}:{}", PREFIX_FILE, video_id, format_id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get file by video+format", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedFile>(&bytes)
        {
            let path = self.cache_dir.join(&cached.relative_path);
            return Ok(Some((cached, path)));
        }

        Ok(None)
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(video_id = video_id, "🔍 Looking for file by preferences in Redis cache");

        let mut conn = self.conn().await?;
        let pattern = format!("{}vf:{}:*", PREFIX_FILE, video_id);
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| crate::error::Error::redis("scan files by preferences", e))?;
            keys.extend(batch);
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        for key in keys {
            let data: Option<Vec<u8>> = conn
                .get(&key)
                .await
                .map_err(|e| crate::error::Error::redis("get file by preferences", e))?;
            if let Some(bytes) = data
                && let Ok(cached) = serde_json::from_slice::<CachedFile>(&bytes)
                && cached.matches_preferences(preferences)
            {
                let path = self.cache_dir.join(&cached.relative_path);
                return Ok(Some((cached, path)));
            }
        }

        Ok(None)
    }

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            file_id = file.id,
            filename = file.filename,
            "⚙️ Caching file to Redis backend"
        );

        let dest_path = copy_to_cache(&self.cache_dir, &file.relative_path, source_path).await?;

        let mut conn = self.conn().await?;
        let bytes = serde_json::to_vec(&file)?;

        // Store by hash
        let hash_key = prefixed_id_key(PREFIX_FILE, &file.id);
        conn.set_ex::<_, _, ()>(&hash_key, &bytes, self.ttl)
            .await
            .map_err(|e| crate::error::Error::redis("set file by hash", e))?;

        // Store by video+format for direct lookup
        if let Some(ref vid) = file.video_id
            && let Some(ref fid) = file.format_id
        {
            let vf_key = format!("{}vf:{}:{}", PREFIX_FILE, vid, fid);
            conn.set_ex::<_, _, ()>(&vf_key, &bytes, self.ttl)
                .await
                .map_err(|e| crate::error::Error::redis("set file by video+format", e))?;
        }

        // Store subtitle files by video+language for lookup via get_subtitle_by_language
        if file.file_type.eq_ignore_ascii_case("subtitle")
            && let Some(ref vid) = file.video_id
            && let Some(ref lang) = file.language_code
        {
            let sub_key = format!("{}sub:{}:{}", PREFIX_FILE, vid, lang);
            conn.set_ex::<_, _, ()>(&sub_key, &bytes, self.ttl)
                .await
                .map_err(|e| crate::error::Error::redis("set file by video+language", e))?;
        }

        Ok(dest_path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        tracing::debug!(file_id = id, "⚙️ Removing file from Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_id_key(PREFIX_FILE, id);

        // Get file metadata to remove physical file and secondary keys
        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get file for remove", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedFile>(&bytes)
        {
            let path = self.cache_dir.join(&cached.relative_path);
            let _ = tokio::fs::remove_file(path).await;

            // Remove video+format key
            if let Some(ref vid) = cached.video_id
                && let Some(ref fid) = cached.format_id
            {
                let vf_key = format!("{}vf:{}:{}", PREFIX_FILE, vid, fid);
                conn.del::<_, ()>(&vf_key)
                    .await
                    .map_err(|e| crate::error::Error::redis("del file by video+format", e))?;
            }
        }

        conn.del::<_, ()>(&key)
            .await
            .map_err(|e| crate::error::Error::redis("del file by hash", e))?;

        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        // Redis handles TTL expiration natively
        Ok(())
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        tracing::debug!(video_id = video_id, "🔍 Looking for thumbnail in Redis cache");

        let mut conn = self.conn().await?;
        let key = prefixed_id_key(PREFIX_THUMBNAIL, video_id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get thumbnail by video", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedThumbnail>(&bytes)
        {
            let path = self.cache_dir.join(&cached.relative_path);
            return Ok(Some((cached, path)));
        }

        Ok(None)
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            "⚙️ Caching thumbnail to Redis backend"
        );

        let dest_path = copy_to_cache(&self.cache_dir, &thumbnail.relative_path, source_path).await?;

        let mut conn = self.conn().await?;
        let bytes = serde_json::to_vec(&thumbnail)?;

        // Store by video_id (one thumbnail per video)
        let key = prefixed_id_key(PREFIX_THUMBNAIL, &thumbnail.video_id);
        conn.set_ex::<_, _, ()>(&key, &bytes, self.ttl)
            .await
            .map_err(|e| crate::error::Error::redis("set thumbnail", e))?;

        // Also store by thumbnail hash
        let hash_key = prefixed_id_key(PREFIX_THUMBNAIL, &thumbnail.id);
        conn.set_ex::<_, _, ()>(&hash_key, &bytes, self.ttl)
            .await
            .map_err(|e| crate::error::Error::redis("set thumbnail by hash", e))?;

        Ok(dest_path)
    }

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            "🔍 Looking for subtitle in Redis cache"
        );

        let mut conn = self.conn().await?;
        let key = format!("{}sub:{}:{}", PREFIX_FILE, video_id, language);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| crate::error::Error::redis("get subtitle by language", e))?;

        if let Some(bytes) = data
            && let Ok(cached) = serde_json::from_slice::<CachedFile>(&bytes)
        {
            let path = self.cache_dir.join(&cached.relative_path);
            return Ok(Some((cached, path)));
        }

        Ok(None)
    }
}
