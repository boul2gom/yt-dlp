#![doc = include_str!("../README.md")]

#[cfg(feature = "cache")]
pub mod cache;
pub mod downloader;
pub mod error;
pub mod executor;
pub mod extractor;
pub mod fetcher;
pub mod metadata;
pub mod model;
pub mod utils;

// Re-export of common traits to facilitate their use
pub use model::utils::{AllTraits, CommonTraits};

// Re-export the main downloader and extractor types
pub use downloader::MediaDownloader;
pub use extractor::{Extractor, ExtractorConfig, ExtractorDetector};

// Backward compatibility: alias for the old YouTube struct
/// Backward compatibility alias for MediaDownloader.
///
/// This type alias maintains compatibility with existing code that uses the `Youtube` struct.
/// All functionality has been moved to `MediaDownloader` which supports multiple platforms.
///
/// # Migration
///
/// Old code:
/// ```rust, no_run
/// use yt_dlp::Youtube;
/// let fetcher = Youtube::new(libraries, output_dir)?;
/// ```
///
/// New code (recommended):
/// ```rust, no_run
/// use yt_dlp::MediaDownloader;
/// let downloader = MediaDownloader::new(libraries, output_dir)?;
/// ```
///
/// Both approaches work identically, but `MediaDownloader` is the preferred name going forward.
pub type Youtube = MediaDownloader;
