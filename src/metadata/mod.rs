//! Metadata management module for downloaded files.
//!
//! This module provides functionality to add metadata to downloaded files,
//! such as title, artist, album, genre, technical information, and thumbnails.
//!
//! ## Supported Formats
//!
//! - **MP3**: Title, artist, comment, genre (from tags), release year
//! - **M4A**: Title, artist, comment, genre (from tags), release year
//! - **MP4**: All basic metadata, plus technical information (resolution, FPS, video codec, video bitrate, audio codec, audio bitrate, audio channels, sample rate)
//! - **WebM**: All basic metadata (via Matroska format), plus technical information as with MP4
//! - **FLAC**: Title, artist, album, genre, date, description (via Vorbis comments through lofty), thumbnail embedding
//! - **OGG/Opus**: Title, artist, album, genre, date, description (via Vorbis comments through lofty)
//! - **WAV**: Title, artist, album, genre (via RIFF INFO through lofty)
//! - **AAC**: Title, artist, album, genre, date (via ID3v2 through lofty)
//! - **AIFF**: Title, artist, album, genre, date (via ID3v2 through lofty)
//! - **AVI/TS/FLV**: Basic metadata via FFmpeg fallback
//!
//! ## Intelligent Metadata Management
//!
//! The system intelligently manages metadata application:
//!
//! - **Standalone files** (audio or audio+video): Metadata applied immediately during download
//! - **Separate streams** (to be combined later): NO metadata applied to avoid redundant work
//! - **Combined files**: Complete metadata applied to final file, including info from both streams

use std::path::PathBuf;

use crate::error::Result;

pub mod api;
pub mod base;
pub mod chapters;
pub mod postprocess;
pub mod writers;

// Re-export the trait
pub use base::BaseMetadata;

/// Playlist metadata information for embedding in video files.
#[derive(Debug, Clone)]
pub struct PlaylistMetadata {
    /// The playlist title/name
    pub title: String,
    /// The playlist ID
    pub id: String,
    /// The track number/index in the playlist (1-based)
    pub index: usize,
    /// Total number of tracks in the playlist (optional)
    pub total: Option<usize>,
}

/// Metadata manager for handling file metadata.
///
/// This manager provides methods to add metadata and thumbnails to downloaded files
/// in various formats (MP3, M4A, MP4, WebM, MKV, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataManager {
    /// Path to ffmpeg executable
    ffmpeg_path: PathBuf,
}

impl MetadataManager {
    /// Create a new MetadataManager with default ffmpeg path.
    ///
    /// The default ffmpeg path is "ffmpeg" unless overridden by the `FFMPEG_PATH`
    /// environment variable.
    ///
    /// # Returns
    ///
    /// A new MetadataManager instance
    pub fn new() -> Self {
        let ffmpeg_path = Self::default_ffmpeg_path();

        tracing::debug!(
            ffmpeg_path = ?ffmpeg_path,
            "⚙️ Creating new MetadataManager"
        );

        Self { ffmpeg_path }
    }

    /// Create a new MetadataManager with custom ffmpeg path.
    ///
    /// # Arguments
    ///
    /// * `ffmpeg_path` - Path to the ffmpeg executable
    ///
    /// # Returns
    ///
    /// A new MetadataManager instance with custom ffmpeg path
    pub fn with_ffmpeg_path(ffmpeg_path: impl Into<PathBuf>) -> Self {
        let ffmpeg_path = ffmpeg_path.into();

        tracing::debug!(
            ffmpeg_path = ?ffmpeg_path,
            "⚙️ Creating MetadataManager with custom ffmpeg path"
        );

        Self { ffmpeg_path }
    }

    /// Get the default ffmpeg path.
    ///
    /// Can be overridden via the `FFMPEG_PATH` environment variable.
    ///
    /// # Returns
    ///
    /// PathBuf to the ffmpeg executable
    pub(crate) fn default_ffmpeg_path() -> PathBuf {
        std::env::var("FFMPEG_PATH")
            .map(|path| {
                tracing::debug!(
                    ffmpeg_path = %path,
                    "⚙️ Using ffmpeg path from FFMPEG_PATH environment variable"
                );
                PathBuf::from(path)
            })
            .unwrap_or_else(|_| {
                tracing::debug!("⚙️ Using default ffmpeg path");
                PathBuf::from("ffmpeg")
            })
    }

    /// Get the file extension from a path.
    ///
    /// Delegates to [`crate::utils::fs::try_extension`].
    pub(crate) fn get_file_extension(file_path: impl Into<PathBuf>) -> Result<String> {
        crate::utils::fs::try_extension(&file_path.into())
    }

    /// Create a temporary output path for metadata processing.
    ///
    /// Delegates to [`crate::utils::fs::create_temp_path`].
    pub(crate) fn create_temp_output_path(
        file_path: impl Into<PathBuf>,
        file_format: &str,
    ) -> crate::error::Result<PathBuf> {
        Ok(crate::utils::fs::create_temp_path(&file_path.into(), file_format))
    }
}

impl Default for MetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseMetadata for MetadataManager {}
