//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types and traits,
//! allowing users to import everything they need with a single use statement.
//!
//! # Examples
//!
//! ```rust
//! use yt_dlp::prelude::*;
//! ```

// Core types
pub use crate::Youtube;
pub use crate::YoutubeBuilder;
pub use crate::error::{Error, Result};

// Client types (new architecture)
pub use crate::client::{DownloadBuilder, Libraries, LibraryInstaller, YoutubeConfig};

// Download types (new architecture)
pub use crate::download::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
pub use crate::download::{Fetcher, ProgressTracker};

// Model types
pub use crate::model::Video;
pub use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};

// Cache types (if enabled)
#[cfg(feature = "cache")]
pub use crate::cache::{DownloadCache, VideoCache};

// Utility types
pub use crate::utils::platform::Platform;
pub use crate::utils::retry::{RetryPolicy, is_http_error_retryable};
pub use crate::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

// Re-export common traits
pub use crate::model::utils::{AllTraits, CommonTraits};
