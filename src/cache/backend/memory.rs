//! In-memory backend implementation for testing.
//!
//! This module provides simple in-memory cache implementations for testing purposes.
//! Data is stored in HashMap and is not persisted.

use super::{FileBackend, VideoBackend};
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[cfg(feature = "cache")]
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

/// Type alias for file cache storage.
type FileStorage = Arc<RwLock<HashMap<String, (CachedFile, Vec<u8>)>>>;
/// Type alias for thumbnail cache storage.
type ThumbnailStorage = Arc<RwLock<HashMap<String, (CachedThumbnail, Vec<u8>)>>>;

/// In-memory video cache implementation for testing.
#[derive(Debug, Clone)]
pub struct MemoryVideoCache {
    data: Arc<RwLock<HashMap<String, CachedVideo>>>,
    ttl: i64,
}

#[async_trait::async_trait]
impl VideoBackend for MemoryVideoCache {
    async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        Ok(Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            ttl: ttl.unwrap_or(24 * 60 * 60) as i64,
        })
    }

    async fn get(&self, url: &str) -> Result<Option<Video>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = url,
            ttl = self.ttl,
            "Looking for video in memory cache by URL"
        );
        let data = self.data.read().await;

        if let Some(cached) = data.get(url) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if cached.cached_at + self.ttl > now {
                return Ok(Some(cached.video()?));
            }
        }

        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            url = %url,
            video_id = %video.id,
            video_title = %video.title,
            "Caching video to memory backend"
        );
        let mut data = self.data.write().await;
        let cached = CachedVideo::from((url.clone(), video));
        data.insert(url, cached);
        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(url = url, "Removing video from memory cache");
        let mut data = self.data.write().await;
        data.remove(url);
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(ttl = self.ttl, "Cleaning memory video cache");
        let mut data = self.data.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        data.retain(|_, cached| cached.cached_at + self.ttl > now);
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        let data = self.data.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for cached in data.values() {
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

/// In-memory file cache implementation for testing.
#[derive(Debug, Clone)]
pub struct MemoryFileCache {
    files: FileStorage,
    thumbnails: ThumbnailStorage,
    ttl: i64,
}

#[async_trait::async_trait]
impl FileBackend for MemoryFileCache {
    async fn new(_cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        Ok(Self {
            files: Arc::new(RwLock::new(HashMap::new())),
            thumbnails: Arc::new(RwLock::new(HashMap::new())),
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60) as i64,
        })
    }

    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            hash = hash,
            ttl = self.ttl,
            "Looking for file in memory cache by hash"
        );
        let files = self.files.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        files.get(hash).and_then(|(cached, _)| {
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
        let files = self.files.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for (cached, _) in files.values() {
            if cached.video_id.as_deref() == Some(video_id)
                && cached.format_id.as_deref() == Some(format_id)
                && cached.cached_at + self.ttl > now
            {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }

        None
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
        let files = self.files.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let vq = video_quality.and_then(|q| serde_json::to_string(&q).ok());
        let aq = audio_quality.and_then(|q| serde_json::to_string(&q).ok());
        let vc = video_codec.and_then(|c| serde_json::to_string(&c).ok());
        let ac = audio_codec.and_then(|c| serde_json::to_string(&c).ok());

        for (cached, _) in files.values() {
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

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            filename = %file.filename,
            file_id = %file.id,
            source_path = ?source_path,
            video_id = ?file.video_id,
            format_id = ?file.format_id,
            filesize = file.filesize,
            "Caching file to memory backend"
        );
        let mut files = self.files.write().await;
        let path = PathBuf::from(&file.relative_path);

        let content = tokio::fs::read(source_path).await?;

        files.insert(file.id.clone(), (file, content));
        Ok(path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(file_id = id, "Removing file from memory cache");
        let mut files = self.files.write().await;
        files.remove(id);
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(ttl = self.ttl, "Cleaning memory file cache");
        let mut files = self.files.write().await;
        let mut thumbnails = self.thumbnails.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        files.retain(|_, (cached, _)| cached.cached_at + self.ttl > now);
        thumbnails.retain(|_, (cached, _)| cached.cached_at + self.ttl > now);
        Ok(())
    }

    async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        let thumbnails = self.thumbnails.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for (cached, _) in thumbnails.values() {
            if cached.video_id == video_id && cached.cached_at + self.ttl > now {
                return Some((cached.clone(), PathBuf::from(&cached.relative_path)));
            }
        }
        None
    }

    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &Path,
    ) -> Result<PathBuf> {
        let mut thumbnails = self.thumbnails.write().await;
        let path = PathBuf::from(&thumbnail.relative_path);
        let content = tokio::fs::read(source_path).await?;
        thumbnails.insert(thumbnail.id.clone(), (thumbnail, content));
        Ok(path)
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        let files = self.files.read().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        for (cached, _) in files.values() {
            // Check if it's a subtitle file and matches video_id and language
            // Assuming file_type or format_id discriminates subtitles?
            // CachedFile has language_code.
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
