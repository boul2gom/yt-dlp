//! JSON file-system based cache backend implementation.
//!
//! This module provides a simple file-system based cache where metadata is stored as JSON files.

use super::{FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// JSON-backed video cache implementation.
#[derive(Debug, Clone)]
pub struct JsonVideoCache {
    cache_dir: PathBuf,
    ttl: u64,
}

#[async_trait::async_trait]
impl VideoBackend for JsonVideoCache {
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let video_dir = cache_dir.join("videos");
        if !video_dir.exists() {
            tokio::fs::create_dir_all(&video_dir).await?;
        }
        Ok(Self {
            cache_dir: video_dir,
            ttl: ttl.unwrap_or(24 * 60 * 60),
        })
    }

    async fn get(&self, url: &str) -> Result<Option<Video>> {
        // Implementation detail: We will use a simple directory traversal for `get` by URL if no index.
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedVideo>(&content)
                    && cached.url == url
                {
                    if self.is_expired(cached.cached_at) {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                        return Ok(None);
                    }
                    return Ok(Some(cached.video()?));
                }
            }
        }
        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        let cached = CachedVideo::from((url, video));
        let file_path = self.cache_dir.join(format!("{}.json", cached.id));
        let content = serde_json::to_string(&cached)?;
        tokio::fs::write(file_path, content).await?;
        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedVideo>(&content)
                    && cached.url == url
                {
                    tokio::fs::remove_file(entry.path()).await?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedVideo>(&content)
                    && self.is_expired(cached.cached_at)
                {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        let file_path = self.cache_dir.join(format!("{}.json", id));
        if file_path.exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let cached: CachedVideo = serde_json::from_str(&content)
                .map_err(|e| crate::error::Error::Unknown(format!("Cache corruption: {}", e)))?;

            if self.is_expired(cached.cached_at) {
                return Err(crate::error::Error::Unknown("Expired".to_string()));
            }
            return Ok(cached);
        }
        Err(crate::error::Error::Unknown("Not found".to_string()))
    }
}

impl JsonVideoCache {
    fn is_expired(&self, cached_at: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        (now - cached_at) > self.ttl as i64
    }
}

/// JSON-backed playlist cache implementation.
#[derive(Debug, Clone)]
pub struct JsonPlaylistCache {
    cache_dir: PathBuf,
    ttl: u64,
}

#[async_trait::async_trait]
impl PlaylistBackend for JsonPlaylistCache {
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let list_dir = cache_dir.join("playlists");
        if !list_dir.exists() {
            tokio::fs::create_dir_all(&list_dir).await?;
        }
        Ok(Self {
            cache_dir: list_dir,
            ttl: ttl.unwrap_or(6 * 60 * 60), // 6 hours default
        })
    }

    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedPlaylist>(&content)
                    && cached.url == url
                {
                    if self.is_expired(cached.cached_at) {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                        return Ok(None);
                    }
                    return Ok(Some(cached.playlist()?));
                }
            }
        }
        Ok(None)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        let file_path = self.cache_dir.join(format!("{}.json", id));
        if file_path.exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let cached: CachedPlaylist = serde_json::from_str(&content)
                .map_err(|e| crate::error::Error::Unknown(format!("Cache corruption: {}", e)))?;

            if self.is_expired(cached.cached_at) {
                return Ok(None);
            }
            return Ok(Some(cached.playlist()?));
        }
        Ok(None)
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        let cached = CachedPlaylist::from((url, playlist));
        let file_path = self.cache_dir.join(format!("{}.json", cached.id));
        let content = serde_json::to_string(&cached)?;
        tokio::fs::write(file_path, content).await?;
        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedPlaylist>(&content)
                    && cached.url == url
                {
                    tokio::fs::remove_file(entry.path()).await?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedPlaylist>(&content)
                    && self.is_expired(cached.cached_at)
                {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        tokio::fs::remove_dir_all(&self.cache_dir).await?;
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        Ok(())
    }
}

impl JsonPlaylistCache {
    fn is_expired(&self, cached_at: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        (now - cached_at) > self.ttl as i64
    }
}

/// JSON-backed file cache implementation.
#[derive(Debug, Clone)]
pub struct JsonFileCache {
    cache_dir: PathBuf,
    ttl: u64,
}

#[async_trait::async_trait]
impl FileBackend for JsonFileCache {
    async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let files_dir = cache_dir.join("files_meta");
        if !files_dir.exists() {
            tokio::fs::create_dir_all(&files_dir).await?;
        }
        // Also ensure actual file storage exists
        let storage_dir = cache_dir.join("files");
        if !storage_dir.exists() {
            tokio::fs::create_dir_all(&storage_dir).await?;
        }

        // Ensure thumbnail directories exist
        let thumbnails_meta = cache_dir.join("thumbnails_meta");
        if !thumbnails_meta.exists() {
            tokio::fs::create_dir_all(&thumbnails_meta).await?;
        }
        let thumbnails_dir = cache_dir.join("thumbnails");
        if !thumbnails_dir.exists() {
            tokio::fs::create_dir_all(&thumbnails_dir).await?;
        }

        Ok(Self {
            cache_dir, // We keep root cache dir to access subdirectories
            ttl: ttl.unwrap_or(7 * 24 * 60 * 60),
        })
    }

    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        let meta_path = self
            .cache_dir
            .join("files_meta")
            .join(format!("{}.json", hash));
        if meta_path.exists() {
            let content = tokio::fs::read_to_string(meta_path).await.ok()?;
            let cached: CachedFile = serde_json::from_str(&content).ok()?;

            if self.is_expired(cached.cached_at) {
                return None;
            }

            let file_path = self.cache_dir.join(&cached.relative_path);
            if file_path.exists() {
                return Some((cached, file_path));
            }
        }
        None
    }

    async fn get_by_video_and_format(
        &self,
        video_id: &str,
        format_id: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                    && cached.format_id.as_deref() == Some(format_id)
                {
                    if self.is_expired(cached.cached_at) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Some((cached, file_path));
                    }
                }
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
        // Scan and filter
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        let vq_str = video_quality.map(|q| serde_json::to_string(&q).unwrap_or_default());
        let aq_str = audio_quality.map(|q| serde_json::to_string(&q).unwrap_or_default());
        let vc_str = video_codec.map(|c| serde_json::to_string(&c).unwrap_or_default());
        let ac_str = audio_codec.map(|c| serde_json::to_string(&c).unwrap_or_default());

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                {
                    // Check preferences
                    if vq_str.is_some() && cached.video_quality != vq_str {
                        continue;
                    }
                    if aq_str.is_some() && cached.audio_quality != aq_str {
                        continue;
                    }
                    if vc_str.is_some() && cached.video_codec != vc_str {
                        continue;
                    }
                    if ac_str.is_some() && cached.audio_codec != ac_str {
                        continue;
                    }

                    if self.is_expired(cached.cached_at) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Some((cached, file_path));
                    }
                }
            }
        }
        None
    }

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        // Write file content (copy from source)
        let file_path = self.cache_dir.join(&file.relative_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &file_path).await?;

        // Write metadata
        let meta_path = self
            .cache_dir
            .join("files_meta")
            .join(format!("{}.json", file.id));
        let meta_json = serde_json::to_string(&file)?;
        tokio::fs::write(meta_path, meta_json).await?;

        Ok(file_path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        let meta_path = self
            .cache_dir
            .join("files_meta")
            .join(format!("{}.json", id));
        if meta_path.exists() {
            // Read to get relative path and delete file
            let content = tokio::fs::read_to_string(&meta_path).await?;
            if let Ok(cached) = serde_json::from_str::<CachedFile>(&content) {
                let file_path = self.cache_dir.join(&cached.relative_path);
                if file_path.exists() {
                    tokio::fs::remove_file(file_path).await?;
                }
            }
            tokio::fs::remove_file(meta_path).await?;
        }
        Ok(())
    }

    async fn clean(&self) -> Result<()> {
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && self.is_expired(cached.cached_at)
                {
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        let _ = tokio::fs::remove_file(file_path).await;
                    }
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }

        // Also clean thumbnails
        let thumb_meta_dir = self.cache_dir.join("thumbnails_meta");
        if let Ok(mut entries) = tokio::fs::read_dir(&thumb_meta_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().extension().is_some_and(|ext| ext == "json") {
                    let content = tokio::fs::read_to_string(entry.path()).await?;
                    if let Ok(cached) = serde_json::from_str::<CachedThumbnail>(&content)
                        && self.is_expired(cached.cached_at)
                    {
                        let file_path = self.cache_dir.join(&cached.relative_path);
                        if file_path.exists() {
                            let _ = tokio::fs::remove_file(file_path).await;
                        }
                        let _ = tokio::fs::remove_file(entry.path()).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_thumbnail_by_video_id(
        &self,
        video_id: &str,
    ) -> Option<(CachedThumbnail, PathBuf)> {
        let meta_dir = self.cache_dir.join("thumbnails_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedThumbnail>(&content)
                    && cached.video_id == video_id
                {
                    if self.is_expired(cached.cached_at) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Some((cached, file_path));
                    }
                }
            }
        }
        None
    }

    async fn put_thumbnail(
        &self,
        thumbnail: CachedThumbnail,
        source_path: &Path,
    ) -> Result<PathBuf> {
        let file_path = self.cache_dir.join(&thumbnail.relative_path);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &file_path).await?;

        // Store metadata
        let meta_path = self
            .cache_dir
            .join("thumbnails_meta")
            .join(format!("{}.json", thumbnail.id));
        let json = serde_json::to_string(&thumbnail)
            .map_err(|e| crate::error::Error::Unknown(format!("Serialization error: {}", e)))?;
        tokio::fs::write(&meta_path, json).await?;

        Ok(file_path)
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                let Ok(cached) = serde_json::from_str::<CachedFile>(&content) else {
                    continue;
                };

                if cached.video_id.as_deref() == Some(video_id)
                    && cached.language_code.as_deref() == Some(language)
                {
                    if self.is_expired(cached.cached_at) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Some((cached, file_path));
                    }
                }
            }
        }
        None
    }
}

impl JsonFileCache {
    fn is_expired(&self, cached_at: i64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        (now - cached_at) > self.ttl as i64
    }
}
