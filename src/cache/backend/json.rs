//! JSON file-system based cache backend implementation.
//!
//! This module provides a simple file-system based cache where metadata is stored as JSON files.

use std::path::{Path, PathBuf};

use super::{
    DEFAULT_FILE_TTL, DEFAULT_PLAYLIST_TTL, DEFAULT_VIDEO_TTL, FileBackend, PlaylistBackend, VideoBackend,
    copy_to_cache, url_hash,
};

/// An expired JSON cache entry found during a directory scan.
struct ExpiredJsonEntry {
    /// Path to the JSON metadata file on disk.
    path: PathBuf,
    /// Original URL, used to derive and delete the companion `.url` index file.
    url: Option<String>,
}

/// Scans `dir` for `.json` files whose `cached_at` field indicates expiry under `ttl`.
///
/// Reads each file as a `serde_json::Value` to avoid a generic bound, extracting
/// `cached_at` (i64 Unix timestamp) and optionally `url` for companion index cleanup.
async fn list_expired_json_entries(dir: &Path, ttl: u64) -> Result<Vec<ExpiredJsonEntry>> {
    let mut expired = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        let Some(cached_at) = val.get("cached_at").and_then(|v| v.as_i64()) else {
            continue;
        };
        if is_expired(cached_at, ttl) {
            let url = val.get("url").and_then(|v| v.as_str()).map(str::to_string);
            expired.push(ExpiredJsonEntry { path, url });
        }
    }

    Ok(expired)
}
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;
use crate::utils::is_expired;

/// JSON-backed video cache implementation.
///
/// # Examples
///
/// ```rust
/// use std::path::PathBuf;
///
/// use yt_dlp::cache::backend::VideoBackend;
/// use yt_dlp::cache::backend::json::JsonVideoCache;
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
            ttl: ttl.unwrap_or(DEFAULT_VIDEO_TTL),
        })
    }
}

impl JsonVideoCache {
    /// Looks up a video by URL using the O(1) URL→ID index file.
    ///
    /// Returns `Ok(Some(video))` on a valid, non-expired cache hit, `Ok(None)` on a miss or expiry.
    /// Removes stale index and data files when an expired entry is found.
    async fn try_index_lookup(&self, url: &str) -> Result<Option<Video>> {
        let index_path = self.cache_dir.join(format!("{}.url", url_hash(url)));
        if !index_path.exists() {
            return Ok(None);
        }
        let Ok(id) = tokio::fs::read_to_string(&index_path).await else {
            let _ = tokio::fs::remove_file(&index_path).await;
            return Ok(None);
        };
        let file_path = self.cache_dir.join(format!("{}.json", id.trim()));
        if !file_path.exists() {
            let _ = tokio::fs::remove_file(&index_path).await;
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&file_path).await?;
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
                let _ = tokio::fs::remove_file(&file_path).await;
                let _ = tokio::fs::remove_file(&index_path).await;
                return Ok(None);
            }
            tracing::debug!(
                url = url,
                video_id = cached.id,
                video_title = cached.title,
                "✅ Cache hit for video (indexed)"
            );
            return Ok(Some(cached.video()?));
        }
        // Stale index entry (URL mismatch — hash collision or overwrite).
        let _ = tokio::fs::remove_file(&index_path).await;
        Ok(None)
    }

    /// Scans the cache directory for a video entry matching `url`.
    ///
    /// Used as a fallback when no URL→ID index exists (backward compatibility
    /// with entries written before index files were introduced).
    async fn fallback_dir_scan(&self, url: &str) -> Result<Option<Video>> {
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
}

impl VideoBackend for JsonVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            ttl = self.ttl,
            "🔍 Looking for video in JSON cache by URL"
        );

        // Try the URL→ID index first for O(1) lookup.
        if let Some(video) = self.try_index_lookup(url).await? {
            return Ok(Some(video));
        }

        // Fallback: directory scan for backward compatibility with pre-index entries.
        self.fallback_dir_scan(url).await
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(
            url = url,
            video_id = video.id,
            video_title = video.title,
            cache_dir = ?self.cache_dir,
            "⚙️ Caching video to JSON backend"
        );
        let id = video.id.clone();
        let cached = CachedVideo::new(url.clone(), &video)?;
        let file_path = self.cache_dir.join(format!("{}.json", cached.id));
        let content = serde_json::to_string(&cached)?;
        tokio::fs::write(file_path, content).await?;

        // Write URL→ID index for O(1) lookup
        let index_path = self.cache_dir.join(format!("{}.url", url_hash(&url)));
        tokio::fs::write(index_path, &id).await?;

        Ok(())
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(
            url = url,
            cache_dir = ?self.cache_dir,
            "⚙️ Removing video from JSON cache"
        );

        // Remove URL→ID index
        let index_path = self.cache_dir.join(format!("{}.url", url_hash(url)));
        let _ = tokio::fs::remove_file(&index_path).await;

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
        let expired = list_expired_json_entries(&self.cache_dir, self.ttl).await?;
        for entry in expired {
            let _ = tokio::fs::remove_file(&entry.path).await;
            if let Some(url) = entry.url {
                let url_index = self.cache_dir.join(format!("{}.url", url_hash(&url)));
                let _ = tokio::fs::remove_file(&url_index).await;
            }
        }
        Ok(())
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, cache_dir = ?self.cache_dir, "🔍 Looking up video by ID in JSON cache");

        let file_path = self.cache_dir.join(format!("{}.json", id));
        if file_path.exists() {
            let content = tokio::fs::read_to_string(file_path).await?;
            let cached: CachedVideo =
                serde_json::from_str(&content).map_err(|e| crate::error::Error::json("Deserialize cached video", e))?;

            if is_expired(cached.cached_at, self.ttl) {
                return Err(crate::error::Error::cache_expired(id));
            }
            return Ok(cached);
        }
        Err(crate::error::Error::cache_miss(id))
    }
}

/// JSON-backed playlist cache implementation.
///
/// # Examples
///
/// ```rust
/// use std::path::PathBuf;
///
/// use yt_dlp::cache::backend::PlaylistBackend;
/// use yt_dlp::cache::backend::json::JsonPlaylistCache;
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
            ttl: ttl.unwrap_or(DEFAULT_PLAYLIST_TTL),
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
        let expired = list_expired_json_entries(&self.cache_dir, self.ttl).await?;
        for entry in expired {
            let _ = tokio::fs::remove_file(&entry.path).await;
            if let Some(url) = entry.url {
                let url_index = self.cache_dir.join(format!("{}.url", url_hash(&url)));
                let _ = tokio::fs::remove_file(&url_index).await;
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
/// use std::path::PathBuf;
///
/// use yt_dlp::cache::backend::FileBackend;
/// use yt_dlp::cache::backend::json::JsonFileCache;
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
            ttl: ttl.unwrap_or(DEFAULT_FILE_TTL),
        })
    }

    async fn clean_expired_files(&self) -> Result<()> {
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = tokio::fs::read_dir(&meta_dir).await?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }

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

        Ok(())
    }

    async fn clean_expired_thumbnails(&self) -> Result<()> {
        let thumb_meta_dir = self.cache_dir.join("thumbnails_meta");
        let Ok(mut entries) = tokio::fs::read_dir(&thumb_meta_dir).await else {
            return Ok(());
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }

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

        Ok(())
    }
}

impl FileBackend for JsonFileCache {
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(
            hash = hash,
            cache_dir = ?self.cache_dir,
            ttl = self.ttl,
            "🔍 Looking for file in JSON cache by hash"
        );
        let meta_path = self.cache_dir.join("files_meta").join(format!("{}.json", hash));
        if meta_path.exists() {
            let content = tokio::fs::read_to_string(&meta_path).await?;
            let cached: CachedFile = serde_json::from_str(&content)?;

            if is_expired(cached.cached_at, self.ttl) {
                tracing::debug!(
                    hash = hash,
                    cached_at = cached.cached_at,
                    ttl = self.ttl,
                    "⚙️ Cache expired for file"
                );
                return Ok(None);
            }

            let file_path = self.cache_dir.join(&cached.relative_path);
            if file_path.exists() {
                tracing::debug!(
                    hash = hash,
                    filename = cached.filename,
                    file_path = ?file_path,
                    "✅ Cache hit for file"
                );
                return Ok(Some((cached, file_path)));
            }
        }
        Ok(None)
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(video_id = video_id, format_id = format_id, cache_dir = ?self.cache_dir, "🔍 Looking for file by video and format in JSON cache");

        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = match tokio::fs::read_dir(&meta_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let Ok(content) = tokio::fs::read_to_string(entry.path()).await else {
                    continue;
                };
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                    && cached.format_id.as_deref() == Some(format_id)
                {
                    if is_expired(cached.cached_at, self.ttl) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Ok(Some((cached, file_path)));
                    }
                }
            }
        }
        Ok(None)
    }

    #[cfg(cache)]
    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        // Scan and filter
        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = match tokio::fs::read_dir(&meta_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let Ok(content) = tokio::fs::read_to_string(entry.path()).await else {
                    continue;
                };
                if let Ok(cached) = serde_json::from_str::<CachedFile>(&content)
                    && cached.video_id.as_deref() == Some(video_id)
                    && cached.matches_preferences(preferences)
                {
                    if is_expired(cached.cached_at, self.ttl) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Ok(Some((cached, file_path)));
                    }
                }
            }
        }
        Ok(None)
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
        let file_path = copy_to_cache(&self.cache_dir, &file.relative_path, source_path).await?;

        // Write metadata
        let meta_path = self.cache_dir.join("files_meta").join(format!("{}.json", file.id));
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
        let meta_path = self.cache_dir.join("files_meta").join(format!("{}.json", id));
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

        self.clean_expired_files().await?;
        self.clean_expired_thumbnails().await?;

        Ok(())
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        tracing::debug!(video_id = video_id, cache_dir = ?self.cache_dir, "🔍 Looking for thumbnail by video ID in JSON cache");

        let meta_dir = self.cache_dir.join("thumbnails_meta");
        let mut entries = match tokio::fs::read_dir(&meta_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                let Ok(content) = tokio::fs::read_to_string(entry.path()).await else {
                    continue;
                };
                if let Ok(cached) = serde_json::from_str::<CachedThumbnail>(&content)
                    && cached.video_id == video_id
                {
                    if is_expired(cached.cached_at, self.ttl) {
                        continue;
                    }
                    let file_path = self.cache_dir.join(&cached.relative_path);
                    if file_path.exists() {
                        return Ok(Some((cached, file_path)));
                    }
                }
            }
        }
        Ok(None)
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &Path) -> Result<PathBuf> {
        let file_path = copy_to_cache(&self.cache_dir, &thumbnail.relative_path, source_path).await?;

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

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(video_id = video_id, language = language, cache_dir = ?self.cache_dir, "🔍 Looking for subtitle by language in JSON cache");

        let meta_dir = self.cache_dir.join("files_meta");
        let mut entries = match tokio::fs::read_dir(&meta_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(result) =
                try_match_subtitle_entry(&entry.path(), video_id, language, self.ttl, &self.cache_dir).await
            {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }
}

/// Attempts to load and validate a subtitle cache entry at `path`.
///
/// Returns `Some((cached, file_path))` when the entry matches the requested
/// `video_id` / `language`, has not expired, and the cached file exists on disk.
/// Returns `None` for any other case (wrong extension, parse error, mismatch, expired,
/// or missing file).
async fn try_match_subtitle_entry(
    path: &std::path::Path,
    video_id: &str,
    language: &str,
    ttl: u64,
    cache_dir: &std::path::Path,
) -> Option<(CachedFile, PathBuf)> {
    if path.extension().is_none_or(|ext| ext != "json") {
        return None;
    }
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let cached: CachedFile = serde_json::from_str(&content).ok()?;
    if cached.video_id.as_deref() != Some(video_id) || cached.language_code.as_deref() != Some(language) {
        return None;
    }
    if is_expired(cached.cached_at, ttl) {
        return None;
    }
    let file_path = cache_dir.join(&cached.relative_path);
    file_path.exists().then_some((cached, file_path))
}
