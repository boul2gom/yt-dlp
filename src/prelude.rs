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
// Cache types (if enabled)
#[cfg(persistent_cache)]
pub use crate::cache::PersistentBackendKind;
#[cfg(cache)]
pub use crate::cache::{CacheConfig, CacheLayer, DownloadCache, VideoCache};
// Client types (new architecture)
pub use crate::client::{DownloadBuilder, Libraries, LibraryInstaller};
pub use crate::download::engine::partial::PartialRange;
// Download types (new architecture)
pub use crate::download::{DownloadManager, DownloadPriority, DownloadStatus, ManagerConfig};
pub use crate::download::{Fetcher, ProgressTracker};
pub use crate::error::{Error, Result};
// Live stream recording types (if enabled)
#[cfg(feature = "live-recording")]
pub use crate::events::RecordingMethod;
// Event types
pub use crate::events::{DownloadEvent, EventBus, EventFilter};
#[cfg(feature = "hooks")]
pub use crate::events::{EventHook, HookRegistry};
#[cfg(feature = "webhooks")]
pub use crate::events::{RetryStrategy, WebhookConfig, WebhookDelivery};
#[cfg(feature = "live-recording")]
pub use crate::live::{FfmpegLiveRecorder, LiveRecorder, LiveRecordingBuilder, RecordingResult};
#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
pub use crate::live::{HlsPlaylist, HlsSegment, HlsVariant};
#[cfg(feature = "live-streaming")]
pub use crate::live::{LiveFragment, LiveFragmentStream, LiveFragmentStreamer, LiveStreamBuilder};
// Model types
pub use crate::model::Video;
pub use crate::model::selector::{
    AudioCodecPreference, AudioQuality, FormatPreferences, StoryboardQuality, VideoCodecPreference, VideoQuality,
};
// Re-export common traits
pub use crate::model::utils::{AllTraits, CommonTraits};
// Statistics types (if enabled)
#[cfg(feature = "statistics")]
pub use crate::stats::{
    ActiveDownloadSnapshot, DownloadOutcomeSnapshot, DownloadSnapshot, DownloadStats, FetchStats, GlobalSnapshot,
    PlaylistStats, PostProcessStats, StatisticsTracker, TrackerConfig,
};
// Utility types
pub use crate::utils::platform::Platform;
pub use crate::utils::retry::{RetryPolicy, is_http_error_retryable};
pub use crate::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};
pub use crate::{Downloader, DownloaderBuilder};
