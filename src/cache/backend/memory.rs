//! In-memory Moka cache backend.
//!
//! This module provides in-memory cache implementations backed by Moka's async cache
//! with built-in TTL eviction. Data is stored in RAM only and is not persisted between
//! process restarts.

use std::path::{Path, PathBuf};
use std::time::Duration;

use moka::future::Cache;

use super::{DEFAULT_FILE_TTL, DEFAULT_PLAYLIST_TTL, DEFAULT_VIDEO_TTL, FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;

const VIDEO_CAPACITY: u64 = 512;
const FILE_CAPACITY: u64 = 64;
const THUMBNAIL_CAPACITY: u64 = 256;
const PLAYLIST_CAPACITY: u64 = 128;

/// In-memory Moka video cache.
#[derive(Debug, Clone)]
pub struct MokaVideoCache {
    data: Cache<String, CachedVideo>,
}

impl MokaVideoCache {
    /// Creates a new in-memory Moka video cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let ttl_secs = ttl.unwrap_or(DEFAULT_VIDEO_TTL);

        Ok(Self {
            data: Cache::builder()
                .max_capacity(VIDEO_CAPACITY)
                .time_to_live(Duration::from_secs(ttl_secs))
                .build(),
        })
    }
}

impl VideoBackend for MokaVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(url = url, "🔍 Looking for video in memory cache by URL");

        if let Some(cached) = self.data.get(url).await {
            return Ok(Some(cached.video()?));
        }

        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(url = url, video_id = video.id, "⚙️ Caching video to memory backend");

        let cached = CachedVideo::new(url.clone(), &video)?;
        self.data.insert(url, cached).await;
        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Removing video from memory cache");

        self.data.remove(url).await;
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        self.data.run_pending_tasks().await;
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, "🔍 Looking up video by ID in memory cache");

        for (_, cached) in &self.data {
            if cached.id == id {
                return Ok(cached);
            }
        }

        Err(crate::error::Error::cache_miss(format!("video:{}", id)))
    }
}

/// In-memory Moka playlist cache.
#[derive(Debug, Clone)]
pub struct MokaPlaylistCache {
    data: Cache<String, CachedPlaylist>,
}

impl MokaPlaylistCache {
    /// Creates a new in-memory Moka playlist cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let ttl_secs = ttl.unwrap_or(DEFAULT_PLAYLIST_TTL);

        Ok(Self {
            data: Cache::builder()
                .max_capacity(PLAYLIST_CAPACITY)
                .time_to_live(Duration::from_secs(ttl_secs))
                .build(),
        })
    }
}

impl PlaylistBackend for MokaPlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(url = url, "🔍 Looking for playlist in memory cache by URL");

        if let Some(cached) = self.data.get(url).await {
            return Ok(Some(cached.playlist()?));
        }

        Ok(None)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(playlist_id = id, "🔍 Looking up playlist by ID in memory cache");

        for (_, cached) in &self.data {
            if cached.id == id {
                return Ok(Some(cached.playlist()?));
            }
        }

        Ok(None)
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            "⚙️ Caching playlist to memory backend"
        );

        let cached = CachedPlaylist::from((url.clone(), playlist));
        self.data.insert(url, cached).await;
        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in memory cache");

        self.data.remove(url).await;
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        self.data.run_pending_tasks().await;
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from memory cache");

        self.data.invalidate_all();
        self.data.run_pending_tasks().await;
        Ok(())
    }
}

/// In-memory Moka file cache.
#[derive(Debug, Clone)]
pub struct MokaFileCache {
    files: Cache<String, CachedFile>,
    thumbnails: Cache<String, CachedThumbnail>,
}

impl MokaFileCache {
    /// Creates a new in-memory Moka file cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let ttl_secs = ttl.unwrap_or(DEFAULT_FILE_TTL);
        let ttl_duration = Duration::from_secs(ttl_secs);

        Ok(Self {
            files: Cache::builder()
                .max_capacity(FILE_CAPACITY)
                .time_to_live(ttl_duration)
                .build(),
            thumbnails: Cache::builder()
                .max_capacity(THUMBNAIL_CAPACITY)
                .time_to_live(ttl_duration)
                .build(),
        })
    }
}

impl FileBackend for MokaFileCache {
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(hash = hash, "🔍 Looking for file in memory cache by hash");

        Ok(self.files.get(hash).await.map(|cached| {
            let path = PathBuf::from(&cached.relative_path);
            (cached, path)
        }))
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            "🔍 Looking for file by video and format in memory cache"
        );

        for (_, cached) in &self.files {
            if cached.video_id.as_deref() == Some(video_id) && cached.format_id.as_deref() == Some(format_id) {
                return Ok(Some((cached.clone(), PathBuf::from(&cached.relative_path))));
            }
        }

        Ok(None)
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            video_quality = ?preferences.video_quality,
            audio_quality = ?preferences.audio_quality,
            "🔍 Looking for file by preferences in memory cache"
        );

        for (_, cached) in &self.files {
            if cached.video_id.as_deref() == Some(video_id) && cached.matches_preferences(preferences) {
                return Ok(Some((cached.clone(), PathBuf::from(&cached.relative_path))));
            }
        }

        Ok(None)
    }

    async fn put(&self, file: CachedFile, _source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            filename = file.filename,
            file_id = file.id,
            "⚙️ Caching file metadata to memory backend"
        );

        let path = PathBuf::from(&file.relative_path);
        self.files.insert(file.id.clone(), file).await;
        Ok(path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        tracing::debug!(file_id = id, "⚙️ Removing file from memory cache");

        self.files.remove(id).await;
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        self.files.run_pending_tasks().await;
        self.thumbnails.run_pending_tasks().await;
        Ok(())
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            "🔍 Looking for thumbnail by video ID in memory cache"
        );

        for (_, cached) in &self.thumbnails {
            if cached.video_id == video_id {
                return Ok(Some((cached.clone(), PathBuf::from(&cached.relative_path))));
            }
        }

        Ok(None)
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, _source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            "⚙️ Caching thumbnail metadata to memory backend"
        );

        let path = PathBuf::from(&thumbnail.relative_path);
        self.thumbnails.insert(thumbnail.id.clone(), thumbnail).await;
        Ok(path)
    }

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            "🔍 Looking for subtitle by language in memory cache"
        );

        for (_, cached) in &self.files {
            if cached.video_id.as_deref() == Some(video_id) && cached.language_code.as_deref() == Some(language) {
                return Ok(Some((cached.clone(), PathBuf::from(&cached.relative_path))));
            }
        }

        Ok(None)
    }
}
