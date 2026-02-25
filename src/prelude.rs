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
pub use crate::Downloader;
pub use crate::DownloaderBuilder;
pub use crate::error::{Error, Result};

// Client types (new architecture)
pub use crate::client::{DownloadBuilder, Libraries, LibraryInstaller};

// Download types (new architecture)
pub use crate::download::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
pub use crate::download::{Fetcher, ProgressTracker};

// Model types
pub use crate::model::Video;
pub use crate::model::selector::{
    AudioCodecPreference, AudioQuality, FormatPreferences, StoryboardQuality, VideoCodecPreference,
    VideoQuality,
};

// Cache types (if enabled)
#[cfg(cache)]
pub use crate::cache::{CacheConfig, CacheLayer, DownloadCache, VideoCache};

// Utility types
pub use crate::utils::platform::Platform;
pub use crate::utils::retry::{RetryPolicy, is_http_error_retryable};
pub use crate::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

// Statistics types (if enabled)
#[cfg(feature = "statistics")]
pub use crate::stats::{
    ActiveDownloadSnapshot, DownloadOutcomeSnapshot, DownloadSnapshot, DownloadStats, FetchStats,
    GlobalSnapshot, PlaylistStats, PostProcessStats, StatisticsTracker, TrackerConfig,
};

// Event types
pub use crate::events::{DownloadEvent, EventBus, EventFilter};

#[cfg(feature = "hooks")]
pub use crate::events::{EventHook, HookRegistry};

#[cfg(feature = "webhooks")]
pub use crate::events::{RetryStrategy, WebhookConfig, WebhookDelivery};

// Re-export common traits
pub use crate::model::utils::{AllTraits, CommonTraits};
