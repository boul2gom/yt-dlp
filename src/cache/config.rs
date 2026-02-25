//! Cache configuration types.
//!
//! Provides `CacheConfig` for configuring the tiered cache system including
//! TTL values and backend-specific connection settings.

use std::path::PathBuf;
use typed_builder::TypedBuilder;

/// Configuration for the tiered cache system.
///
/// Uses `TypedBuilder` for ergonomic construction with sensible defaults.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::cache::CacheConfig;
/// use std::path::PathBuf;
///
/// let config = CacheConfig::builder()
///     .cache_dir(PathBuf::from("cache"))
///     .build();
/// ```
#[derive(Debug, Clone, TypedBuilder)]
pub struct CacheConfig {
    /// Directory where cache data will be stored.
    pub cache_dir: PathBuf,

    /// Connection URL for Redis backend (e.g. "redis://127.0.0.1/").
    /// Only used when `cache-redis` feature is enabled.
    #[builder(default)]
    pub redis_url: Option<String>,

    /// Time-to-live for video cache entries in seconds.
    /// Default: 24 hours (86400 seconds).
    #[builder(default)]
    pub video_ttl: Option<u64>,

    /// Time-to-live for playlist cache entries in seconds.
    /// Default: 6 hours (21600 seconds).
    #[builder(default)]
    pub playlist_ttl: Option<u64>,

    /// Time-to-live for download/file cache entries in seconds.
    /// Default: 7 days (604800 seconds).
    #[builder(default)]
    pub download_ttl: Option<u64>,
}

impl std::fmt::Display for CacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CacheConfig(dir={:?}, redis={}, video_ttl={:?}, playlist_ttl={:?}, download_ttl={:?})",
            self.cache_dir,
            self.redis_url.as_deref().unwrap_or("none"),
            self.video_ttl,
            self.playlist_ttl,
            self.download_ttl,
        )
    }
}
