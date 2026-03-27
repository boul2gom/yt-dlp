//! Redb file cache backend.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable};

use super::{DEFAULT_FILE_TTL, FILES, THUMBNAILS, clean_redb_table, copy_to_cache};
use crate::cache::backend::FileBackend;
use crate::cache::video::{CachedFile, CachedThumbnail};
use crate::error::Result;
use crate::model::selector::FormatPreferences;
use crate::utils::is_expired;

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
    async fn get_by_hash(&self, hash: &str) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(hash = hash, "🔍 Looking for file in redb cache by hash");

        let db = self.db.clone();
        let hash_owned = hash.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read file by hash", e))?;
            let table = txn
                .open_table(FILES)
                .map_err(|e| crate::error::Error::database("open files table", e))?;

            if let Some(entry) = table
                .get(hash_owned.as_str())
                .map_err(|e| crate::error::Error::database("get file by hash", e))?
            {
                let bytes = entry.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Ok(Some((cached, path)));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get file by hash", e))?
    }

    async fn get_by_video_and_format(&self, video_id: &str, format_id: &str) -> Result<Option<(CachedFile, PathBuf)>> {
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
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read file by video+format", e))?;
            let table = txn
                .open_table(FILES)
                .map_err(|e| crate::error::Error::database("open files table", e))?;

            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate files table", e))?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.format_id.as_deref() == Some(&fid)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Ok(Some((cached, path)));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get file by video+format", e))?
    }

    async fn get_by_video_and_preferences(
        &self,
        video_id: &str,
        preferences: &FormatPreferences,
    ) -> Result<Option<(CachedFile, PathBuf)>> {
        tracing::debug!(video_id = video_id, "🔍 Looking for file by preferences in redb cache");

        let db = self.db.clone();
        let vid = video_id.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;
        let prefs = preferences.clone();

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read file by preferences", e))?;
            let table = txn
                .open_table(FILES)
                .map_err(|e| crate::error::Error::database("open files table", e))?;

            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate files table", e))?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.matches_preferences(&prefs)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Ok(Some((cached, path)));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get file by preferences", e))?
    }

    async fn put(&self, file: CachedFile, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            file_id = file.id,
            filename = file.filename,
            "⚙️ Caching file to redb backend"
        );

        let dest_path = copy_to_cache(&self.cache_dir, &file.relative_path, source_path).await?;

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

    async fn get_thumbnail_by_video_id(&self, video_id: &str) -> Result<Option<(CachedThumbnail, PathBuf)>> {
        tracing::debug!(
            video_id = video_id,
            "🔍 Looking for thumbnail by video ID in redb cache"
        );

        let db = self.db.clone();
        let vid = video_id.to_string();
        let cache_dir = self.cache_dir.clone();
        let ttl = self.ttl;

        tokio::task::spawn_blocking(move || {
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read thumbnail by video", e))?;
            let table = txn
                .open_table(THUMBNAILS)
                .map_err(|e| crate::error::Error::database("open thumbnails table", e))?;

            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate thumbnails table", e))?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedThumbnail>(bytes)
                    && cached.video_id == vid
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Ok(Some((cached, path)));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get thumbnail by video", e))?
    }

    async fn put_thumbnail(&self, thumbnail: CachedThumbnail, source_path: &Path) -> Result<PathBuf> {
        tracing::debug!(
            thumbnail_id = thumbnail.id,
            video_id = thumbnail.video_id,
            "⚙️ Caching thumbnail to redb backend"
        );

        let dest_path = copy_to_cache(&self.cache_dir, &thumbnail.relative_path, source_path).await?;

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

    async fn get_subtitle_by_language(&self, video_id: &str, language: &str) -> Result<Option<(CachedFile, PathBuf)>> {
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
            let txn = db
                .begin_read()
                .map_err(|e| crate::error::Error::database("read subtitle by language", e))?;
            let table = txn
                .open_table(FILES)
                .map_err(|e| crate::error::Error::database("open files table", e))?;

            let iter = table
                .iter()
                .map_err(|e| crate::error::Error::database("iterate files table", e))?;
            for (_key, val) in iter.flatten() {
                let bytes = val.value();
                if let Ok(cached) = serde_json::from_slice::<CachedFile>(bytes)
                    && cached.video_id.as_deref() == Some(&vid)
                    && cached.language_code.as_deref() == Some(&lang)
                    && !is_expired(cached.cached_at, ttl)
                {
                    let path = cache_dir.join(&cached.relative_path);
                    return Ok(Some((cached, path)));
                }
            }
            Ok(None)
        })
        .await
        .map_err(|e| crate::error::Error::runtime("redb get subtitle by language", e))?
    }
}
