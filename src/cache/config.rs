//! Cache configuration types.
//!
//! Provides `CacheConfig` for configuring the tiered cache system including
//! TTL values and backend-specific connection settings.

use std::fmt;
use std::path::PathBuf;

use typed_builder::TypedBuilder;

#[cfg(persistent_cache)]
use crate::error::{Error, Result};

/// Number of persistent backends compiled into this binary.
///
/// Used by `PersistentBackendKind::resolve` to detect ambiguity when
/// `persistent_backend` is left as `None` in `CacheConfig`.
#[cfg(persistent_cache)]
const PERSISTENT_BACKEND_COUNT: usize = cfg!(feature = "cache-json") as usize
    + cfg!(feature = "cache-redb") as usize
    + cfg!(feature = "cache-redis") as usize;

/// Selects which persistent L2 cache backend to use at runtime.
///
/// When exactly one persistent feature is compiled in, the backend is
/// deduced automatically and this field can be left as `None` in `CacheConfig`.
/// When several features are enabled simultaneously, the caller **must** set
/// `CacheConfig::persistent_backend` explicitly; leaving it as `None` causes
/// `CacheLayer::from_config` to return `Error::AmbiguousCacheBackend`.
#[cfg(persistent_cache)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PersistentBackendKind {
    /// JSON-file backend (`cache-json` feature).
    #[cfg(feature = "cache-json")]
    Json,
    /// Embedded redb backend (`cache-redb` feature).
    #[cfg(feature = "cache-redb")]
    Redb,
    /// Distributed Redis backend (`cache-redis` feature).
    #[cfg(feature = "cache-redis")]
    Redis,
}

#[cfg(persistent_cache)]
impl PersistentBackendKind {
    /// Resolves the backend to use.
    ///
    /// # Arguments
    ///
    /// * `kind` - An explicit selection, or `None` to auto-detect.
    ///
    /// # Errors
    ///
    /// Returns `Error::AmbiguousCacheBackend` when `kind` is `None` and
    /// more than one persistent feature is compiled in.
    ///
    /// # Returns
    ///
    /// The resolved `PersistentBackendKind`.
    pub fn resolve(kind: Option<Self>) -> Result<Self> {
        if let Some(k) = kind {
            return Ok(k);
        }
        if PERSISTENT_BACKEND_COUNT > 1 {
            return Err(Error::ambiguous_cache_backend(PERSISTENT_BACKEND_COUNT));
        }
        // Exactly one persistent feature compiled in — auto-detect via exclusive cfg guards.
        // Each combination compiles exactly one `let backend` binding.
        #[cfg(feature = "cache-json")]
        let backend = Self::Json;
        #[cfg(all(feature = "cache-redb", not(feature = "cache-json")))]
        let backend = Self::Redb;
        #[cfg(all(feature = "cache-redis", not(feature = "cache-json"), not(feature = "cache-redb")))]
        let backend = Self::Redis;

        Ok(backend)
    }
}

#[cfg(persistent_cache)]
impl fmt::Display for PersistentBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "cache-json")]
            Self::Json => f.write_str("Json"),
            #[cfg(feature = "cache-redb")]
            Self::Redb => f.write_str("Redb"),
            #[cfg(feature = "cache-redis")]
            Self::Redis => f.write_str("Redis"),
        }
    }
}

/// Configuration for the tiered cache system.
///
/// Uses `TypedBuilder` for ergonomic construction with sensible defaults.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
///
/// use yt_dlp::cache::CacheConfig;
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

    /// Which persistent backend to activate at runtime.
    ///
    /// Leave as `None` when exactly one persistent feature is compiled in
    /// (it is deduced automatically). Must be set explicitly when several
    /// persistent features (`cache-json`, `cache-redb`, `cache-redis`) are
    /// compiled in simultaneously.
    #[cfg(persistent_cache)]
    #[builder(default)]
    pub persistent_backend: Option<PersistentBackendKind>,
}

impl fmt::Display for CacheConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CacheConfig(dir={:?}, redis={}, video_ttl={:?}, playlist_ttl={:?}, download_ttl={:?}",
            self.cache_dir,
            self.redis_url.as_deref().unwrap_or("none"),
            self.video_ttl,
            self.playlist_ttl,
            self.download_ttl,
        )?;
        #[cfg(persistent_cache)]
        write!(
            f,
            ", backend={}",
            self.persistent_backend.map_or("auto".to_string(), |k| k.to_string()),
        )?;
        f.write_str(")")
    }
}
