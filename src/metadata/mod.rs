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
//!
//! ## Intelligent Metadata Management
//!
//! The system intelligently manages metadata application:
//!
//! - **Standalone files** (audio or audio+video): Metadata applied immediately during download
//! - **Separate streams** (to be combined later): NO metadata applied to avoid redundant work
//! - **Combined files**: Complete metadata applied to final file, including info from both streams

use std::path::{Path, PathBuf};

mod api;
mod base;
mod chapters;
mod ffmpeg;
mod mp3;
mod mp4;
pub mod postprocess;

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
    pub fn new() -> Self {
        Self {
            ffmpeg_path: Self::default_ffmpeg_path(),
        }
    }

    /// Create a new MetadataManager with custom ffmpeg path.
    ///
    /// # Arguments
    ///
    /// * `ffmpeg_path` - Path to the ffmpeg executable
    pub fn with_ffmpeg_path(ffmpeg_path: impl AsRef<Path>) -> Self {
        Self {
            ffmpeg_path: ffmpeg_path.as_ref().to_path_buf(),
        }
    }

    /// Get the default ffmpeg path.
    ///
    /// Can be overridden via the `FFMPEG_PATH` environment variable.
    pub(crate) fn default_ffmpeg_path() -> PathBuf {
        std::env::var("FFMPEG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("ffmpeg"))
    }

    /// Get the file extension from a path.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to extract extension from
    ///
    /// # Returns
    ///
    /// Lowercase file extension
    ///
    /// # Errors
    ///
    /// Returns an error if the file has no extension or contains invalid characters
    pub(crate) fn get_file_extension(file_path: impl AsRef<Path>) -> crate::error::Result<String> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Getting file extension for {:?}", file_path.as_ref());

        let path = file_path.as_ref();
        let ext = path
            .extension()
            .ok_or_else(|| crate::error::Error::path_validation(path, "File has no extension"))?
            .to_str()
            .ok_or_else(|| {
                crate::error::Error::path_validation(path, "Invalid characters in file extension")
            })?
            .to_lowercase();

        Ok(ext)
    }

    /// Create a temporary output path for metadata processing.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Original file path
    /// * `file_format` - File extension for the temporary file
    ///
    /// # Returns
    ///
    /// PathBuf to a unique temporary file in the same directory
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be created
    pub(crate) fn create_temp_output_path(
        file_path: impl AsRef<Path>,
        file_format: &str,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Creating temporary output path for {:?}",
            file_path.as_ref()
        );

        let path = file_path.as_ref();
        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
        let uuid = uuid::Uuid::new_v4();

        if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            Ok(parent_dir.join(format!("{}_{}_temp.{}", file_stem, uuid, file_format)))
        } else {
            Ok(parent_dir.join(format!("output_{}_temp.{}", uuid, file_format)))
        }
    }

    /// Log metadata debug messages if tracing is enabled.
    pub(crate) fn log_metadata_debug<S: AsRef<str>>(_message: S) {
        #[cfg(feature = "tracing")]
        tracing::debug!("{}", _message.as_ref());
    }
}

impl Default for MetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseMetadata for MetadataManager {}
