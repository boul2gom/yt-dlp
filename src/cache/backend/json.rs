//! JSON file-system based cache backend implementation.
//!
//! This module provides a simple file-system based cache where metadata is stored as JSON files.

use super::{FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;
use crate::utils::is_expired;
use std::path::Path;
use std::path::PathBuf;

/// JSON-backed video cache implementation.
///
/// # Examples
///
/// ```rust
/// use yt_dlp::cache::backend::json::JsonVideoCache;
/// use yt_dlp::cache::backend::VideoBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = JsonVideoCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonVideoCache {
    cache_dir: PathBuf,
    ttl: u64,
}

impl JsonVideoCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let video_dir = cache_dir.join("videos");
        if !video_dir.exists() {
            tokio::fs::create_dir_all(&video_dir).await?;
        }
        Ok(Self {
            cache_dir: video_dir,
            ttl: ttl.unwrap_or(24 * 60 * 60),
        })
    }
}

impl VideoBackend for JsonVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            ttl = self.ttl,
            "🔍 Looking for video in JSON cache by URL"
        );
        // Implementation detail: We will use a simple directory traversal for `get` by URL if no index.
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedVideo>(&content)
                    && cached.url == url
                {
                    if is_expired(cached.cached_at, self.ttl) {
                        tracing::debug!(
                            url = url,
                            cached_at = cached.cached_at,
                            ttl = self.ttl,
                            "⚙️ Cache expired for video"
                        );
                        let _ = tokio::fs::remove_file(entry.path()).await;
                        return Ok(None);
                    }
                    tracing::debug!(
                        url = url,
                        video_id = cached.id,
                        video_title = cached.title,
                        "✅ Cache hit for video"
                    );
                    return Ok(Some(cached.video()?));
                }
            }
        }
        Ok(None)
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(
            url = url,
            video_id = video.id,
            video_title = video.title,
            cache_dir = ?self.cache_dir,
            "⚙️ Caching video to JSON backend"
        );
        let cached = CachedVideo::from((url, video));
        let file_path = self.cache_dir.join(format!("{}.json", cached.id));
        let content = serde_json::to_string(&cached)?;
        tokio::fs::write(file_path, content).await?;
        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            "⚙️ Removing video from JSON cache"
        );
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
        tracing::debug!(
            ttl = self.ttl,
            cache_dir = ?self.cache_dir,
            "⚙️ Cleaning JSON video cache"
        );
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedVideo>(&content)
                    && is_expired(cached.cached_at, self.ttl)
                {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, cache_dir = ?self.cache_dir, "🔍 Looking up video by ID in JSON cache");

        let file_path = self.cache_dir.join(format!("{}.json", id));
        if file_path.exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let cached: CachedVideo = serde_json::from_str(&content)
                .map_err(|e| crate::error::Error::json("Deserialize cached video", e))?;

            if is_expired(cached.cached_at, self.ttl) {
                return Err(crate::error::Error::Unknown("Expired".to_string()));
            }
            return Ok(cached);
        }
        Err(crate::error::Error::Unknown("Not found".to_string()))
    }
}

/// JSON-backed playlist cache implementation.
///
/// # Examples
///
/// ```rust
/// use yt_dlp::cache::backend::json::JsonPlaylistCache;
/// use yt_dlp::cache::backend::PlaylistBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = JsonPlaylistCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonPlaylistCache {
    cache_dir: PathBuf,
    ttl: u64,
}

impl JsonPlaylistCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        let list_dir = cache_dir.join("playlists");
        if !list_dir.exists() {
            tokio::fs::create_dir_all(&list_dir).await?;
        }
        Ok(Self {
            cache_dir: list_dir,
            ttl: ttl.unwrap_or(6 * 60 * 60), // 6 hours default
        })
    }
}

impl PlaylistBackend for JsonPlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            ttl = self.ttl,
            "🔍 Looking for playlist in JSON cache by URL"
        );
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedPlaylist>(&content)
                    && cached.url == url
                {
                    if is_expired(cached.cached_at, self.ttl) {
                        tracing::debug!(
                            url = url,
                            cached_at = cached.cached_at,
                            ttl = self.ttl,
                            "⚙️ Cache expired for playlist"
                        );
                        let _ = tokio::fs::remove_file(entry.path()).await;
                        return Ok(None);
                    }
                    tracing::debug!(
                        url = url,
                        playlist_id = cached.id,
                        playlist_title = cached.title,
                        "✅ Cache hit for playlist"
                    );
                    return Ok(Some(cached.playlist()?));
                }
            }
        }
        Ok(None)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(playlist_id = id, cache_dir = ?self.cache_dir, "🔍 Looking up playlist by ID in JSON cache");

        let file_path = self.cache_dir.join(format!("{}.json", id));
        if file_path.exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let cached: CachedPlaylist = serde_json::from_str(&content)
                .map_err(|e| crate::error::Error::json("Deserialize cached playlist", e))?;

            if is_expired(cached.cached_at, self.ttl) {
                return Ok(None);
            }
            return Ok(Some(cached.playlist()?));
        }
        Ok(None)
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            playlist_title = playlist.title,
            entry_count = playlist.entries.len(),
            cache_dir = ?self.cache_dir,
            "⚙️ Caching playlist to JSON backend"
        );
        let cached = CachedPlaylist::from((url, playlist));
        let file_path = self.cache_dir.join(format!("{}.json", cached.id));
        let content = serde_json::to_string(&cached)?;
        tokio::fs::write(file_path, content).await?;
        Ok(())
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            "⚙️ Invalidating playlist in JSON cache"
        );
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
        tracing::debug!(
            ttl = self.ttl,
            cache_dir = ?self.cache_dir,
            "⚙️ Cleaning JSON playlist cache"
        );
        let mut entries = tokio::fs::read_dir(&self.cache_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedPlaylist>(&content)
                    && is_expired(cached.cached_at, self.ttl)
                {
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
        }
        Ok(())
    }

    async fn clear_all(&self) -> Result<()> {
        tracing::debug!(cache_dir = ?self.cache_dir, "⚙️ Clearing all playlists from JSON cache");

        tokio::fs::remove_dir_all(&self.cache_dir).await?;
        tokio::fs::create_dir_all(&self.cache_dir).await?;
        Ok(())
    }
}

/// JSON-backed file cache implementation.
///
/// # Examples
///
/// ```rust
/// use yt_dlp::cache::backend::json::JsonFileCache;
/// use yt_dlp::cache::backend::FileBackend;
/// use std::path::PathBuf;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let cache = JsonFileCache::new(PathBuf::from("/tmp/cache"), None).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct JsonFileCache {
    cache_dir: PathBuf,
    ttl: u64,
}

impl JsonFileCache {
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
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
}

impl FileBackend for JsonFileCache {
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            hash = hash,
            cache_dir = ?self.cache_dir,
            ttl = self.ttl,
            "🔍 Looking for file in JSON cache by hash"
        );
        let meta_path = self
            .cache_dir
            .join("files_meta")
            .join(format!("{}.json", hash));
        if meta_path.exists() {
            let content = tokio::fs::read_to_string(meta_path).await.ok()?;
            let cached: CachedFile = serde_json::from_str(&content).ok()?;

            if is_expired(cached.cached_at, self.ttl) {
                tracing::debug!(
                    hash = hash,
                    cached_at = cached.cached_at,
                    ttl = self.ttl,
                    "⚙️ Cache expired for file"
                );
                return None;
            }

            let file_path = self.cache_dir.join(&cached.relative_path);
            if file_path.exists() {
                tracing::debug!(
                    hash = hash,
                    filename = cached.filename,
                    file_path = ?file_path,
                    "✅ Cache hit for file"
                );
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
        tracing::debug!(video_id = video_id, format_id = format_id, cache_dir = ?self.cache_dir, "🔍 Looking for file by video and format in JSON cache");

        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                    && cached.format_id.as_deref() == Some(format_id)
                {
                    if is_expired(cached.cached_at, self.ttl) {
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

    #[cfg(cache)]
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Option<(CachedFile, PathBuf)> {
        // Scan and filter
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                    && cached.matches_preferences(preferences)
                {
                    if is_expired(cached.cached_at, self.ttl) {
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
        tracing::debug!(
            filename = file.filename,
            file_id = file.id,
            source_path = ?source_path,
            video_id = ?file.video_id,
            format_id = ?file.format_id,
            cache_dir = ?self.cache_dir,
            "⚙️ Caching file to JSON backend"
        );
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
        tracing::debug!(
            file_id = id,
            cache_dir = ?self.cache_dir,
            "⚙️ Removing file from JSON cache"
        );
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
        tracing::debug!(
            ttl = self.ttl,
            cache_dir = ?self.cache_dir,
            "⚙️ Cleaning JSON file cache"
        );
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await?;
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && is_expired(cached.cached_at, self.ttl)
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
                        && is_expired(cached.cached_at, self.ttl)
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
        tracing::debug!(video_id = video_id, cache_dir = ?self.cache_dir, "🔍 Looking for thumbnail by video ID in JSON cache");

        let meta_dir = self.cache_dir.join("thumbnails_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await.ok()?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let content = tokio::fs::read_to_string(entry.path()).await.ok()?;
                if let Ok(cached) = serde_json::from_str::<CachedThumbnail>(&content)
                    && cached.video_id == video_id
                {
                    if is_expired(cached.cached_at, self.ttl) {
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
            .map_err(|e| crate::error::Error::json("Serialize cached thumbnail", e))?;
        tokio::fs::write(&meta_path, json).await?;

        Ok(file_path)
    }

    async fn get_subtitle_by_language(
        &self,
        video_id: &str,
        language: &str,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(video_id = video_id, language = language, cache_dir = ?self.cache_dir, "🔍 Looking for subtitle by language in JSON cache");

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
                    if is_expired(cached.cached_at, self.ttl) {
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
