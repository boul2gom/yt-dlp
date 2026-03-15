//! Redb playlist cache backend.

use std::path::PathBuf;
use std::sync::Arc;

use redb::{Database, ReadableDatabase, ReadableTable};

use super::{DEFAULT_PLAYLIST_TTL, PLAYLIST_URL_INDEX, PLAYLISTS, clean_redb_table, url_hash};
use crate::cache::backend::PlaylistBackend;
use crate::cache::playlist::CachedPlaylist;
use crate::error::Result;
use crate::model::playlist::Playlist;
use crate::utils::is_expired;

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
                let _ = txn.open_table(PLAYLIST_URL_INDEX)?;
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

            let hash = url_hash(&url_owned);
            let index = txn
                .open_table(PLAYLIST_URL_INDEX)
                .map_err(|e| crate::error::Error::database("open playlist url index", e))?;

            if let Some(id_guard) = index
                .get(hash.as_str())
                .map_err(|e| crate::error::Error::database("read playlist url index", e))?
            {
                let id = id_guard.value();
                let table = txn
                    .open_table(PLAYLISTS)
                    .map_err(|e| crate::error::Error::database("open playlists table", e))?;

                if let Some(entry) = table
                    .get(id)
                    .map_err(|e| crate::error::Error::database("get playlist by indexed id", e))?
                {
                    let cached: CachedPlaylist = serde_json::from_slice(entry.value())?;
                    if !is_expired(cached.cached_at, ttl) {
                        return Ok(Some(cached.playlist()?));
                    }
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
            let hash = url_hash(&url);
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

                let mut index = txn
                    .open_table(PLAYLIST_URL_INDEX)
                    .map_err(|e| crate::error::Error::database("open playlist url index", e))?;
                index
                    .insert(hash.as_str(), cached.id.as_str())
                    .map_err(|e| crate::error::Error::database("insert playlist url index", e))?;
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
            let hash = url_hash(&url_owned);
            let txn = db
                .begin_write()
                .map_err(|e| crate::error::Error::database("write playlist invalidate", e))?;
            {
                let mut index = txn
                    .open_table(PLAYLIST_URL_INDEX)
                    .map_err(|e| crate::error::Error::database("open playlist url index", e))?;

                let id = index
                    .get(hash.as_str())
                    .map_err(|e| crate::error::Error::database("read playlist url index", e))?
                    .map(|g| g.value().to_string());

                if let Some(id) = id {
                    let mut table = txn
                        .open_table(PLAYLISTS)
                        .map_err(|e| crate::error::Error::database("open playlists table", e))?;
                    table
                        .remove(id.as_str())
                        .map_err(|e| crate::error::Error::database("remove playlist", e))?;
                    drop(table);

                    index
                        .remove(hash.as_str())
                        .map_err(|e| crate::error::Error::database("remove playlist url index", e))?;
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
            clean_redb_table(&db, PLAYLISTS, ttl, &std::path::PathBuf::new(), "playlist")
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
