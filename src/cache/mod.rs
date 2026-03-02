//! Cache module for storing video metadata and downloaded files.
//!
//! This module provides a tiered caching architecture with L1 (in-memory) and L2 (persistent)
//! layers. Users can enable any combination of:
//!
//! - **No cache**: Disable all cache features
//! - **Moka only** (`cache-memory`): Fast in-memory cache with TTL eviction
//! - **Persistent only** (`cache-json`, `cache-redb`, or `cache-redis`): Durable storage
//! - **Moka + persistent**: L1 memory cache backed by L2 persistent storage
//!
//! At most one persistent backend may be enabled at a time.

pub mod backend;
pub mod config;
pub mod layer;
pub mod stores;

// Re-export store modules for backward compatibility
pub use stores::{files, playlist, video};

// Safety net: at most one persistent backend allowed.
#[cfg(multiple_persistent_backends)]
compile_error!(
    "Enable at most one persistent cache backend at a time: \
     \"cache-json\", \"cache-redb\", or \"cache-redis\""
);

// Re-export main types
pub use config::CacheConfig;
pub use files::DownloadCache;
pub use layer::CacheLayer;
// Re-export common structures
pub use playlist::CachedPlaylist;
pub use playlist::PlaylistCache;
pub use video::{CachedFile, CachedThumbnail, CachedVideo, VideoCache};

// Common types and traits
pub use crate::model::selector::{
    AudioCodecPreference, AudioQuality, FormatPreferences, VideoCodecPreference, VideoQuality,
};
