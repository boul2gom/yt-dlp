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
