//! Cache module for storing video metadata and downloaded files.
//!
//! This module provides async-safe functionality for caching video metadata and downloaded files
//! to avoid making repeated requests for the same videos and re-downloading the same files.
//!
//! Uses `sqlx` for fully async SQLite operations that do not block the tokio runtime.

pub mod backend;
pub mod files;
pub mod playlist;
pub mod video;

// Safety net: cache-backend is internal and must not be enabled directly.
#[cfg(all(
    feature = "cache-backend",
    not(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))
))]
compile_error!(
    "Feature \"cache-backend\" is internal and must not be enabled directly; \
     use \"cache\", \"cache-json\", or \"cache-sqlite\""
);

// Priority order when multiple backends are enabled: cache-sqlite > cache-json > cache.

// Re-export main types
pub use files::DownloadCache;
pub use playlist::PlaylistCache;
pub use video::VideoCache;

// Re-export common structures
pub use playlist::CachedPlaylist;
pub use video::{CachedFile, CachedThumbnail, CachedVideo};

// Common types and traits
pub use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current timestamp in seconds since UNIX epoch.
///
/// # Returns
///
/// Unix timestamp in seconds as i64.
pub fn current_timestamp() -> i64 {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    tracing::debug!(timestamp = timestamp, "Retrieved current timestamp");

    timestamp
}

/// Checks if a timestamp is expired given a TTL.
///
/// # Arguments
///
/// * `cached_at` - The timestamp when the item was cached (Unix timestamp in seconds)
/// * `ttl` - Time-to-live in seconds
///
/// # Returns
///
/// `true` if the cached item has expired, `false` otherwise.
pub fn is_expired(cached_at: i64, ttl: u64) -> bool {
    let now = current_timestamp();
    let expired = (now - cached_at) > ttl as i64;

    tracing::debug!(
        cached_at = cached_at,
        ttl = ttl,
        now = now,
        expired = expired,
        "Checking cache expiration"
    );

    expired
}
