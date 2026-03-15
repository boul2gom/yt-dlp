//! Redb video cache backend.

use std::path::PathBuf;
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable};

use super::{DEFAULT_VIDEO_TTL, VIDEO_URL_INDEX, VIDEOS, clean_redb_table, url_hash};
use crate::cache::backend::VideoBackend;
use crate::cache::video::CachedVideo;
use crate::error::Result;
use crate::model::Video;
use crate::utils::is_expired;

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

        let db = Arc::new(db);
        let db_init = db.clone();
        tokio::task::spawn_blocking(move || {
            let txn = db_init.begin_write()?;
            {
                let _ = txn.open_table(VIDEOS)?;
                let _ = txn.open_table(VIDEO_URL_INDEX)?;
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

            let hash = url_hash(&url_owned);
            let index = txn
                .open_table(VIDEO_URL_INDEX)
                .map_err(|e| crate::error::Error::database("open video url index", e))?;

            if let Some(id_guard) = index
                .get(hash.as_str())
                .map_err(|e| crate::error::Error::database("read video url index", e))?
            {
                let id = id_guard.value();
                let table = txn
                    .open_table(VIDEOS)
                    .map_err(|e| crate::error::Error::database("open videos table", e))?;

                if let Some(entry) = table
                    .get(id)
                    .map_err(|e| crate::error::Error::database("get video by indexed id", e))?
                {
                    let cached: CachedVideo = serde_json::from_slice(entry.value())?;
                    if !is_expired(cached.cached_at, ttl) {
                        return Ok(Some(cached.video()?));
                    }
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
            let hash = url_hash(&url);
            let cached = CachedVideo::new(url, &video)?;
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

                let mut index = txn
                    .open_table(VIDEO_URL_INDEX)
                    .map_err(|e| crate::error::Error::database("open video url index", e))?;
                index
                    .insert(hash.as_str(), cached.id.as_str())
                    .map_err(|e| crate::error::Error::database("insert video url index", e))?;
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
            let hash = url_hash(&url_owned);
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write video remove", e))?;
            {
                let mut index = txn
                    .open_table(VIDEO_URL_INDEX)
                    .map_err(|e| crate::error::Error::database("open video url index", e))?;

                let id = index
                    .get(hash.as_str())
                    .map_err(|e| crate::error::Error::database("read video url index", e))?
                    .map(|g| g.value().to_string());

                if let Some(id) = id {
                    let mut table = txn
                        .open_table(VIDEOS)
                        .map_err(|e| crate::error::Error::database("open videos table", e))?;
                    table
                        .remove(id.as_str())
                        .map_err(|e| crate::error::Error::database("remove video", e))?;
                    drop(table);

                    index
                        .remove(hash.as_str())
                        .map_err(|e| crate::error::Error::database("remove video url index", e))?;
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

        tokio::task::spawn_blocking(move || clean_redb_table(&db, VIDEOS, ttl, &std::path::PathBuf::new(), "video"))
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
