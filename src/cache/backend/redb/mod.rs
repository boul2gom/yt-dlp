//! Redb embedded database cache backend.
//!
//! This module provides a persistent cache backed by redb, a pure-Rust ACID-compliant
//! embedded key-value store. Suitable for production use with crash-safe single-file storage.
//!
//! All redb operations are synchronous and wrapped in `tokio::task::spawn_blocking`
//! to avoid blocking the async runtime.

mod file;
mod playlist;
mod video;

use std::path::Path;

pub use file::RedbFileCache;
pub use playlist::RedbPlaylistCache;
use redb::{Database, ReadableTable, TableDefinition};
pub use video::RedbVideoCache;

use crate::error::Result;
use crate::utils::is_expired;

pub(crate) const VIDEOS: TableDefinition<&str, &[u8]> = TableDefinition::new("videos");
/// Secondary index: URL hash → video ID for O(1) lookups.
pub(crate) const VIDEO_URL_INDEX: TableDefinition<&str, &str> = TableDefinition::new("video_url_index");
pub(crate) const PLAYLISTS: TableDefinition<&str, &[u8]> = TableDefinition::new("playlists");
/// Secondary index: URL hash → playlist ID for O(1) lookups.
pub(crate) const PLAYLIST_URL_INDEX: TableDefinition<&str, &str> = TableDefinition::new("playlist_url_index");
pub(crate) const FILES: TableDefinition<&str, &[u8]> = TableDefinition::new("files");
pub(crate) const THUMBNAILS: TableDefinition<&str, &[u8]> = TableDefinition::new("thumbnails");

pub(crate) use super::{DEFAULT_FILE_TTL, DEFAULT_PLAYLIST_TTL, DEFAULT_VIDEO_TTL, copy_to_cache, url_hash};

/// Clean expired entries from a redb table, removing associated files on disk.
pub(crate) fn clean_redb_table(
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
