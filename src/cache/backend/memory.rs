//! In-memory LRU cache backend.
//!
//! This module provides in-memory cache implementations backed by LRU eviction.
//! Data is stored in RAM only and is not persisted between process restarts.

use super::{FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::current_timestamp;
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::model::utils::serde::serialize_json_opt;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

// LruCache::get() takes &mut self, so Mutex is required instead of RwLock.
const VIDEO_CAPACITY: usize = 512;
const FILE_CAPACITY: usize = 64;
const THUMBNAIL_CAPACITY: usize = 256;
const PLAYLIST_CAPACITY: usize = 128;

/// In-memory LRU video cache.
#[derive(Debug, Clone)]
pub struct MemoryVideoCache {
    data: Arc<Mutex<LruCache<String, CachedVideo>>>,
    ttl: i64,
}

impl MemoryVideoCache {
    /// Creates a new in-memory LRU video cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        Ok(Self {
            data: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(VIDEO_CAPACITY).unwrap(),
            ))),
            ttl: ttl.unwrap_or(24 * 60 * 60) as i64,
        })
    }
}

impl VideoBackend for MemoryVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = url,
            ttl = self.ttl,
            "Looking for video in memory cache by URL"
        );

        let mut data = self.data.lock().await;
        let now = current_timestamp();

        if let Some(cached) = data.get(url)
            && cached.cached_at + self.ttl > now
        {
            return Ok(Some(cached.video()?));
        }

        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(url = %url, video_id = %video.id, "Caching video to memory backend");

        let mut data = self.data.lock().await;
        let cached = CachedVideo::from((url.clone(), video));
        data.put(url, cached);
        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Removing video from memory cache");

        let mut data = self.data.lock().await;
        data.pop(url);
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            ttl = self.ttl,
            "Cleaning expired entries from memory video cache"
        );

        let mut data = self.data.lock().await;
        let now = current_timestamp();

        let expired: Vec<String> = data
            .iter()
            .filter(|(_, cached)| cached.cached_at + self.ttl <= now)
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            data.pop(&key);
        }

        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        let data = self.data.lock().await;
        let now = current_timestamp();

        for (_, cached) in data.iter() {
            if cached.id == id && cached.cached_at + self.ttl > now {
                return Ok(cached.clone());
            }
        }

        Err(crate::error::Error::Unknown(format!(
            "Video with ID {} not found or expired in cache",
            id
        )))
    }
}

/// In-memory LRU file cache.
#[derive(Debug, Clone)]
pub struct MemoryFileCache {
    // Stores file metadata only — no file bytes are read into memory.
    files: Arc<Mutex<LruCache<String, CachedFile>>>,
    thumbnails: Arc<Mutex<LruCache<String, CachedThumbnail>>>,
    ttl: i64,
}

impl MemoryFileCache {
    /// Creates a new in-memory LRU file cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        Ok(Self {
            files: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(FILE_CAPACITY).unwrap(),
            ))),
            thumbnails: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(THUMBNAIL_CAPACITY).unwrap(),
            ))),
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60) as i64,
        })
    }
}

impl FileBackend for MemoryFileCache {
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            hash = hash,
            ttl = self.ttl,
            "Looking for file in memory cache by hash"
        );

        let mut files = self.files.lock().await;
        let now = current_timestamp();

        files.get(hash).and_then(|cached| {
            if cached.cached_at + self.ttl > now {
                Some((cached.clone(), PathBuf::from(&cached.relative_path)))
            } else {
                None
            }
        })
    }

    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        let files = self.files.lock().await;
        let now = current_timestamp();

        for (_, cached) in files.iter() {
            if cached.video_id.as_deref() == Some(video_id)
                && cached.format_id.as_deref() == Some(format_id)
                && cached.cached_at + self.ttl > now
            {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }

        None
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        video_quality: Option<VideoQuality>,
        audio_quality: Option<AudioQuality>,
        video_codec: Option<VideoCodecPreference>,
        audio_codec: Option<AudioCodecPreference>,
    ) -> Option<(CachedFile, PathBuf)> {
        let files = self.files.lock().await;
        let now = current_timestamp();

        let vq = serialize_json_opt(video_quality);
        let aq = serialize_json_opt(audio_quality);
        let vc = serialize_json_opt(video_codec);
        let ac = serialize_json_opt(audio_codec);

        for (_, cached) in files.iter() {
            if cached.video_id.as_deref() == Some(video_id)
                && (vq.is_none() || cached.video_quality == vq)
                && (aq.is_none() || cached.audio_quality == aq)
                && (vc.is_none() || cached.video_codec == vc)
                && (ac.is_none() || cached.audio_codec == ac)
                && cached.cached_at + self.ttl > now
            {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }

        None
    }

    async fn put(&self, file: CachedFile, _source_path: &Path) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            filename = %file.filename,
            file_id = %file.id,
            "Caching file metadata to memory backend"
        );

        let mut files = self.files.lock().await;
        let path = PathBuf::from(&file.relative_path);
        files.put(file.id.clone(), file);
        Ok(path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(file_id = id, "Removing file from memory cache");

        let mut files = self.files.lock().await;
        files.pop(id);
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            ttl = self.ttl,
            "Cleaning expired entries from memory file cache"
        );

        let now = current_timestamp();

        {
            let mut files = self.files.lock().await;
            let expired: Vec<String> = files
                .iter()
                .filter(|(_, cached)| cached.cached_at + self.ttl <= now)
                .map(|(k, _)| k.clone())
                .collect();
            for key in expired {
                files.pop(&key);
            }
        }

        {
            let mut thumbnails = self.thumbnails.lock().await;
            let expired: Vec<String> = thumbnails
                .iter()
                .filter(|(_, cached)| cached.cached_at + self.ttl <= now)
                .map(|(k, _)| k.clone())
                .collect();
            for key in expired {
                thumbnails.pop(&key);
            }
        }

        Ok(())
    }

    async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        let thumbnails = self.thumbnails.lock().await;
        let now = current_timestamp();

        for (_, cached) in thumbnails.iter() {
            if cached.video_id == video_id && cached.cached_at + self.ttl > now {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }

        None
    }

    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        _source_path: &Path,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            thumbnail_id = %thumbnail.id,
            video_id = %thumbnail.video_id,
            "Caching thumbnail metadata to memory backend"
        );

        let mut thumbnails = self.thumbnails.lock().await;
        let path = PathBuf::from(&thumbnail.relative_path);
        thumbnails.put(thumbnail.id.clone(), thumbnail);
        Ok(path)
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        let files = self.files.lock().await;
        let now = current_timestamp();

        for (_, cached) in files.iter() {
            if cached.video_id.as_deref() == Some(video_id)
                && cached.language_code.as_deref() == Some(language)
                && cached.cached_at + self.ttl > now
            {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }

        None
    }
}

/// In-memory LRU playlist cache.
#[derive(Debug, Clone)]
pub struct MemoryPlaylistCache {
    data: Arc<Mutex<LruCache<String, CachedPlaylist>>>,
    ttl: i64,
}

impl MemoryPlaylistCache {
    /// Creates a new in-memory LRU playlist cache.
    pub async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        Ok(Self {
            data: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(PLAYLIST_CAPACITY).unwrap(),
            ))),
            ttl: ttl.unwrap_or(6 * 60 * 60) as i64,
        })
    }
}

impl PlaylistBackend for MemoryPlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        let mut data = self.data.lock().await;
        let now = current_timestamp();

        if let Some(cached) = data.get(url)
            && cached.cached_at + self.ttl > now
        {
            return Ok(Some(cached.playlist()?));
        }

        Ok(None)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        let data = self.data.lock().await;
        let now = current_timestamp();

        for (_, cached) in data.iter() {
            if cached.id == id && cached.cached_at + self.ttl > now {
                return Ok(Some(cached.playlist()?));
            }
        }

        Ok(None)
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        let mut data = self.data.lock().await;
        let cached = CachedPlaylist::from((url.clone(), playlist));
        data.put(url, cached);
        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        let mut data = self.data.lock().await;
        data.pop(url);
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        let mut data = self.data.lock().await;
        let now = current_timestamp();

        let expired: Vec<String> = data
            .iter()
            .filter(|(_, cached)| cached.cached_at + self.ttl <= now)
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            data.pop(&key);
        }

        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        let mut data = self.data.lock().await;
        data.clear();
        Ok(())
    }
}
