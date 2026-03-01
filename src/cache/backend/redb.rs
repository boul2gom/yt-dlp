//! Redb embedded database cache backend.
//!
//! This module provides a persistent cache backed by redb, a pure-Rust ACID-compliant
//! embedded key-value store. Suitable for production use with crash-safe single-file storage.
//!
//! All redb operations are synchronous and wrapped in `tokio::task::spawn_blocking`
//! to avoid blocking the async runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

use super::{FileBackend, PlaylistBackend, VideoBackend};
use crate::cache::playlist::CachedPlaylist;
use crate::cache::video::{CachedFile, CachedThumbnail, CachedVideo};
use crate::error::Result;
use crate::model::Video;
use crate::model::playlist::Playlist;
use crate::model::selector::FormatPreferences;
use crate::utils::is_expired;

const VIDEOS: TableDefinition<&str, &[u8]> = TableDefinition::new("videos");
const PLAYLISTS: TableDefinition<&str, &[u8]> = TableDefinition::new("playlists");
const FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("files");
const THUMBNAILS: TableDefinition<&str, &[u8]> = TableDefinition::new("thumbnails");

const DEFAULT_VIDEO_TTL: u64 = 24 * 60 * 60;
const DEFAULT_PLAYLIST_TTL: u64 = 6 * 60 * 60;
const DEFAULT_FILE_TTL: u64 = 7 * 24 * 60 * 60;

/// Redb-backed video cache.
#[derive(Debug, Clone)]
pub struct RedbVideoCache {
    db: Arc<Database>,
    ttl: u64,
}

impl RedbVideoCache {
    /// Creates a new redb video cache.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let db_path = cache_dir.join("videos.redb");
        let db = tokio::task::spawn_blocking(move || Database::create(db_path))
            .await
            .map_err(|e| crate::error::Error::runtime("redb open", e))?
            .map_err(|e| crate::error::Error::database("open videos.redb", e))?;

        // Create table if it doesn't exist
        let db = Arc::new(db);
        let db_init = db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db_init.begin_write()?;
            {
                let _ = txn.open_table(VIDEOS)?;
            }
            txn.commit()?;
            Ok::<_, redb::Error>(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb init", e))?
        .map_err(|e| crate::error::Error::database("init videos table", e))?;

        Ok(Self {
            db,
            ttl: ttl.unwrap_or(DEFAULT_VIDEO_TTL),
        })
    }
}

impl VideoBackend for RedbVideoCache {
    async fn get(&self, url: &str) -> Result<Option<Video>> {
        tracing::debug!(url = url, "🔍 Looking for video in redb cache by URL");

        let db = self.db.clone();
        let url_owned = url.to_string();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read video", e))?;
            let table = txn
                .open_table(VIDEOS)
                .map_err(|e| crate::error::Error::database("open videos table", e))?;

            // Scan for matching URL
            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate videos", e))?;
            for entry in iter {
                let (_key, val) = entry.map_err(|e| crate::error::Error::database("read video entry", e))?;
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedVideo>(bytes)
                    && cached.url == url_owned
                    && !is_expired(cached.cached_at, ttl)
                {
                    return Ok(Some(cached.video()?));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get video", e))?
    }

    async fn put(&self, url: String, video: Video) -> Result<()> {
        tracing::debug!(url = url, video_id = video.id, "⚙️ Caching video to redb backend");

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let cached = CachedVideo::from((url, video));
            let bytes = serde_json::to_vec(&cached)?;
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write video", e))?;
            {
                let mut table = txn
                    .open_table(VIDEOS)
                    .map_err(|e| crate::error::Error::database("open videos table", e))?;
                table
                    .insert(cached.id.as_str(), bytes.as_slice())
                    .map_err(|e| crate::error::Error::database("insert video", e))?;
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit video", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb put video", e))?
    }

    async fn remove(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Removing video from redb cache");

        let db = self.db.clone();
        let url_owned = url.to_string();

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write video remove", e))?;
            {
                let table = txn
                    .open_table(VIDEOS)
                    .map_err(|e| crate::error::Error::database("open videos table", e))?;

                // Find the key matching this URL
                let mut key_to_remove = None;
                let iter = table
                    .iter()
                    .map_err(|e| crate::error::Error::database("iterate videos", e))?;
                for entry in iter {
                    let (_key, val) = entry.map_err(|e| crate::error::Error::database("read entry", e))?;
                    let bytes = val.value();
                    if let Ok(cached) = serde_json::from_slice::<CachedVideo>(bytes)
                        && cached.url == url_owned
                    {
                        key_to_remove = Some(cached.id);
                        break;
                    }
                }
                drop(table);

                if let Some(key) = key_to_remove {
                    let mut table = txn
                        .open_table(VIDEOS)
                        .map_err(|e| crate::error::Error::database("open videos table", e))?;
                    table
                        .remove(key.as_str())
                        .map_err(|e| crate::error::Error::database("remove video", e))?;
                }
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit video remove", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb remove video", e))?
    }

    async fn clean(&self) -> Result<()> {
        let db = self.db.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write video clean", e))?;
            {
                let table = txn
                    .open_table(VIDEOS)
                    .map_err(|e| crate::error::Error::database("open videos table", e))?;
                let mut expired_keys = Vec::new();

                let iter = table
                    .iter()
                    .map_err(|e| crate::error::Error::database("iterate videos", e))?;
                for entry in iter {
                    let (key_guard, val) = entry.map_err(|e| crate::error::Error::database("read entry", e))?;
                    let key = key_guard.value().to_string();
                    let bytes = val.value();
                    if let Ok(cached) = serde_json::from_slice::<CachedVideo>(bytes)
                        && is_expired(cached.cached_at, ttl)
                    {
                        expired_keys.push(key);
                    }
                }
                drop(table);

                if !expired_keys.is_empty() {
                    let mut table = txn
                        .open_table(VIDEOS)
                        .map_err(|e| crate::error::Error::database("open videos table", e))?;
                    for key in &expired_keys {
                        table
                            .remove(key.as_str())
                            .map_err(|e| crate::error::Error::database("remove expired video", e))?;
                    }
                }
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit video clean", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb clean videos", e))?
    }

    async fn get_by_id(&self, id: &str) -> Result<CachedVideo> {
        tracing::debug!(video_id = id, "🔍 Looking up video by ID in redb cache");

        let db = self.db.clone();
        let id_owned = id.to_string();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read video by id", e))?;
            let table = txn
                .open_table(VIDEOS)
                .map_err(|e| crate::error::Error::database("open videos table", e))?;

            if let Some(entry) = table
                .get(id_owned.as_str())
                .map_err(|e| crate::error::Error::database("get video by id", e))?
            {
                let bytes = entry.value();
                let cached: CachedVideo = serde_json::from_slice(bytes)?;
                if !is_expired(cached.cached_at, ttl) {
                    return Ok(cached);
                }
            }

            Err(crate::error::Error::cache_miss(format!("video:{}", id_owned)))
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get video by id", e))?
    }
}

/// Redb-backed playlist cache.
#[derive(Debug, Clone)]
pub struct RedbPlaylistCache {
    db: Arc<Database>,
    ttl: u64,
}

impl RedbPlaylistCache {
    /// Creates a new redb playlist cache.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let db_path = cache_dir.join("playlists.redb");
        let db = tokio::task::spawn_blocking(move || Database::create(db_path))
            .await
            .map_err(|e| crate::error::Error::runtime("redb open", e))?
            .map_err(|e| crate::error::Error::database("open playlists.redb", e))?;

        let db = Arc::new(db);
        let db_init = db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db_init.begin_write()?;
            {
                let _ = txn.open_table(PLAYLISTS)?;
            }
            txn.commit()?;
            Ok::<_, redb::Error>(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb init", e))?
        .map_err(|e| crate::error::Error::database("init playlists table", e))?;

        Ok(Self {
            db,
            ttl: ttl.unwrap_or(DEFAULT_PLAYLIST_TTL),
        })
    }
}

impl PlaylistBackend for RedbPlaylistCache {
    async fn get(&self, url: &str) -> Result<Option<Playlist>> {
        tracing::debug!(url = url, "🔍 Looking for playlist in redb cache by URL");

        let db = self.db.clone();
        let url_owned = url.to_string();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read playlist", e))?;
            let table = txn
                .open_table(PLAYLISTS)
                .map_err(|e| crate::error::Error::database("open playlists table", e))?;

            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate playlists", e))?;
            for entry in iter {
                let (_key, val) = entry.map_err(|e| crate::error::Error::database("read playlist entry", e))?;
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedPlaylist>(bytes)
                    && cached.url == url_owned
                    && !is_expired(cached.cached_at, ttl)
                {
                    return Ok(Some(cached.playlist()?));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get playlist", e))?
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Playlist>> {
        tracing::debug!(playlist_id = id, "🔍 Looking up playlist by ID in redb cache");

        let db = self.db.clone();
        let id_owned = id.to_string();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read playlist by id", e))?;
            let table = txn
                .open_table(PLAYLISTS)
                .map_err(|e| crate::error::Error::database("open playlists table", e))?;

            if let Some(entry) = table
                .get(id_owned.as_str())
                .map_err(|e| crate::error::Error::database("get playlist by id", e))?
            {
                let bytes = entry.value();
                let cached: CachedPlaylist = serde_json::from_slice(bytes)?;
                if !is_expired(cached.cached_at, ttl) {
                    return Ok(Some(cached.playlist()?));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get playlist by id", e))?
    }

    async fn put(&self, url: String, playlist: Playlist) -> Result<()> {
        tracing::debug!(
            url = url,
            playlist_id = playlist.id,
            "⚙️ Caching playlist to redb backend"
        );

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let cached = CachedPlaylist::from((url, playlist));
            let bytes = serde_json::to_vec(&cached)?;
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write playlist", e))?;
            {
                let mut table = txn
                    .open_table(PLAYLISTS)
                    .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                table
                    .insert(cached.id.as_str(), bytes.as_slice())
                    .map_err(|e| crate::error::Error::database("insert playlist", e))?;
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit playlist", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb put playlist", e))?
    }

    async fn invalidate(&self, url: &str) -> Result<()> {
        tracing::debug!(url = url, "⚙️ Invalidating playlist in redb cache");

        let db = self.db.clone();
        let url_owned = url.to_string();

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write playlist invalidate", e))?;
            {
                let table = txn
                    .open_table(PLAYLISTS)
                    .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                let mut key_to_remove = None;

                let iter = table
                    .iter()
                    .map_err(|e| crate::error::Error::database("iterate playlists", e))?;
                for entry in iter {
                    let (_key, val) = entry.map_err(|e| crate::error::Error::database("read entry", e))?;
                    let bytes = val.value();
                    if let Ok(cached) = serde_json::from_slice::<CachedPlaylist>(bytes)
                        && cached.url == url_owned
                    {
                        key_to_remove = Some(cached.id);
                        break;
                    }
                }
                drop(table);

                if let Some(key) = key_to_remove {
                    let mut table = txn
                        .open_table(PLAYLISTS)
                        .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                    table
                        .remove(key.as_str())
                        .map_err(|e| crate::error::Error::database("remove playlist", e))?;
                }
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit playlist invalidate", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb invalidate playlist", e))?
    }

    async fn clean(&self) -> Result<()> {
        let db = self.db.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write playlist clean", e))?;
            {
                let table = txn
                    .open_table(PLAYLISTS)
                    .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                let mut expired_keys = Vec::new();

                let iter = table
                    .iter()
                    .map_err(|e| crate::error::Error::database("iterate playlists", e))?;
                for entry in iter {
                    let (key_guard, val) = entry.map_err(|e| crate::error::Error::database("read entry", e))?;
                    let key = key_guard.value().to_string();
                    let bytes = val.value();
                    if let Ok(cached) = serde_json::from_slice::<CachedPlaylist>(bytes)
                        && is_expired(cached.cached_at, ttl)
                    {
                        expired_keys.push(key);
                    }
                }
                drop(table);

                if !expired_keys.is_empty() {
                    let mut table = txn
                        .open_table(PLAYLISTS)
                        .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                    for key in &expired_keys {
                        table
                            .remove(key.as_str())
                            .map_err(|e| crate::error::Error::database("remove expired playlist", e))?;
                    }
                }
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit playlist clean", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb clean playlists", e))?
    }

    async fn clear_all(&self) -> Result<()> {
        tracing::debug!("⚙️ Clearing all playlists from redb cache");

        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write playlist clear", e))?;
            {
                let mut table = txn
                    .open_table(PLAYLISTS)
                    .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                // Drain: collect all keys and remove
                let keys: Vec<String> = table
                    .iter()
                    .map_err(|e| crate::error::Error::database("iterate playlists", e))?
                    .filter_map(|entry| entry.ok().map(|(k, _)| k.value().to_string()))
                    .collect();
                for key in &keys {
                    table
                        .remove(key.as_str())
                        .map_err(|e| crate::error::Error::database("remove playlist", e))?;
                }
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit playlist clear", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb clear playlists", e))?
    }
}

/// Redb-backed file cache.
#[derive(Debug, Clone)]
pub struct RedbFileCache {
    db: Arc<Database>,
    cache_dir: PathBuf,
    ttl: u64,
}

impl RedbFileCache {
    /// Creates a new redb file cache.
    pub async fn new(cache_dir: PathBuf, ttl: Option<u64>) -> Result<Self> {
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(&cache_dir).await?;
        }

        let db_path = cache_dir.join("files.redb");
        let db = tokio::task::spawn_blocking(move || Database::create(db_path))
            .await
            .map_err(|e| crate::error::Error::runtime("redb open", e))?
            .map_err(|e| crate::error::Error::database("open files.redb", e))?;

        let db = Arc::new(db);
        let db_init = db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db_init.begin_write()?;
            {
                let _ = txn.open_table(FILES)?;
            }
            {
                let _ = txn.open_table(THUMBNAILS)?;
            }
            txn.commit()?;
            Ok::<_, redb::Error>(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb init", e))?
        .map_err(|e| crate::error::Error::database("init files/thumbnails tables", e))?;

        Ok(Self {
            db,
            cache_dir,
            ttl: ttl.unwrap_or(DEFAULT_FILE_TTL),
        })
    }
}

impl FileBackend for RedbFileCache {
    async fn get_by_hash(&self, hash: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(hash = hash, "🔍 Looking for file in redb cache by hash");

        let db = self.db.clone();
        let hash_owned = hash.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(FILES).ok()?;

            if let Some(entry) = table.get(hash_owned.as_str()).ok()? {
                let bytes = entry.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Some((cached, path));
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            format_id = format_id,
            "🔍 Looking for file by video and format in redb cache"
        );

        let db = self.db.clone();
        let vid = video_id.to_string();
        let fid = format_id.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(FILES).ok()?;

            let iter = table.iter().ok()?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.format_id.as_deref() == Some(&fid)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Some((cached, path));
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(video_id = video_id, "🔍 Looking for file by preferences in redb cache");

        let db = self.db.clone();
        let vid = video_id.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;
        let prefs = preferences.clone();

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(FILES).ok()?;

            let iter = table.iter().ok()?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.matches_preferences(&prefs)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Some((cached, path));
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            file_id = file.id,
            filename = file.filename,
            "⚙️ Caching file to redb backend"
        );

        let dest_path = self.cache_dir.join(&file.relative_path);
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &dest_path).await?;

        let db = self.db.clone();
        let ret_path = dest_path.clone();
        tokio::task::spawn_blocking(move || {
            let bytes = serde_json::to_vec(&file)?;
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write file", e))?;
            {
                let mut table = txn
                    .open_table(FILES)
                    .map_err(|e| crate::error::Error::database("open files table", e))?;
                table
                    .insert(file.id.as_str(), bytes.as_slice())
                    .map_err(|e| crate::error::Error::database("insert file", e))?;
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit file", e))?;
            Ok::<_, crate::error::Error>(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb put file", e))??;

        Ok(ret_path)
    }

    async fn remove(&self, id: &str) -> Result<()> {
        tracing::debug!(file_id = id, "⚙️ Removing file from redb cache");

        // Remove the physical file first
        let db = self.db.clone();
        let id_owned = id.to_string();
        let cache_dir = self.cache_dir.clone();

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write file remove", e))?;
            {
                let mut table = txn
                    .open_table(FILES)
                    .map_err(|e| crate::error::Error::database("open files table", e))?;

                // Get path before removing
                if let Some(entry) = table
                    .get(id_owned.as_str())
                    .map_err(|e| crate::error::Error::database("get file for remove", e))?
                {
                    let bytes = entry.value();
                    if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes) {
                        let path = cache_dir.join(&cached.relative_path);
                        let _ = std::fs::remove_file(path);
                    }
                }

                table
                    .remove(id_owned.as_str())
                    .map_err(|e| crate::error::Error::database("remove file", e))?;
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit file remove", e))?;
            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb remove file", e))?
    }

    async fn clean(&self) -> Result<()> {
        let db = self.db.clone();
        let ttl = self.ttl;
        let cache_dir = self.cache_dir.clone();

        tokio::task::spawn_blocking(move || {
            clean_redb_table(&db, FILES, ttl, &cache_dir, "file")?;
            clean_redb_table(&db, THUMBNAILS, ttl, &cache_dir, "thumbnail")?;

            Ok(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb clean files", e))?
    }

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Option<(CachedThumbnail, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            "🔍 Looking for thumbnail by video ID in redb cache"
        );

        let db = self.db.clone();
        let vid = video_id.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(THUMBNAILS).ok()?;

            let iter = table.iter().ok()?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedThumbnail>(bytes)
                    && cached.video_id == vid
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Some((cached, path));
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            "⚙️ Caching thumbnail to redb backend"
        );

        let dest_path = self.cache_dir.join(&thumbnail.relative_path);
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(source_path, &dest_path).await?;

        let db = self.db.clone();
        let ret_path = dest_path.clone();
        tokio::task::spawn_blocking(move || {
            let bytes = serde_json::to_vec(&thumbnail)?;
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write thumbnail", e))?;
            {
                let mut table = txn
                    .open_table(THUMBNAILS)
                    .map_err(|e| crate::error::Error::database("open thumbnails table", e))?;
                table
                    .insert(thumbnail.id.as_str(), bytes.as_slice())
                    .map_err(|e| crate::error::Error::database("insert thumbnail", e))?;
            }
            txn.commit()
                .map_err(|e| crate::error::Error::database("commit thumbnail", e))?;
            Ok::<_, crate::error::Error>(())
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb put thumbnail", e))??;

        Ok(ret_path)
    }

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Option<(CachedFile, PathBuf)> {
        tracing::debug!(
            video_id = video_id,
            language = language,
            "🔍 Looking for subtitle in redb cache"
        );

        let db = self.db.clone();
        let vid = video_id.to_string();
        let lang = language.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db.begin_read().ok()?;
            let table = txn.open_table(FILES).ok()?;

            let iter = table.iter().ok()?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.language_code.as_deref() == Some(&lang)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Some((cached, path));
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }
}

/// Clean expired entries from a redb table, removing associated files on disk.
fn clean_redb_table(
    db: &Database,
    table_def: TableDefinition<&str, &[u8]>,
    ttl: u64,
    cache_dir: &Path,
    label: &str,
) -> Result<()> {
    let txn = db
        .begin_write()
        .map_err(|e| crate::error::Error::database(format!("write {label} clean"), e))?;
    let table = txn
        .open_table(table_def)
        .map_err(|e| crate::error::Error::database(format!("open {label} table"), e))?;

    let iter = table
        .iter()
        .map_err(|e| crate::error::Error::database(format!("iterate {label}s"), e))?;

    let mut expired = Vec::new();
    for (key_guard, val) in iter.flatten() {
        let key = key_guard.value().to_string();
        let bytes = val.value();

        // Parse the JSON value to extract cached_at and relative_path
        let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            continue;
        };
        let cached_at = json_val.get("cached_at").and_then(|v| v.as_i64()).unwrap_or(0);
        let relative_path = json_val.get("relative_path").and_then(|v| v.as_str());

        if is_expired(cached_at, ttl) {
            if let Some(rel) = relative_path {
                let _ = std::fs::remove_file(cache_dir.join(rel));
            }
            expired.push(key);
        }
    }
    drop(table);

    if !expired.is_empty() {
        let mut table = txn
            .open_table(table_def)
            .map_err(|e| crate::error::Error::database(format!("open {label} table"), e))?;
        for key in &expired {
            table
                .remove(key.as_str())
                .map_err(|e| crate::error::Error::database(format!("remove expired {label}"), e))?;
        }
    }

    txn.commit()
        .map_err(|e| crate::error::Error::database(format!("commit {label} clean"), e))?;

    Ok(())
}
