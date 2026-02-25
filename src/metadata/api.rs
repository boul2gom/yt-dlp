//! Public API methods for metadata management.
//!
//! This module provides the high-level public API for adding metadata
//! and thumbnails to downloaded files.

use std::path::PathBuf;
use std::str::FromStr;

use super::MetadataManager;
use crate::error::Result;
use crate::model::Video;
use crate::model::format::{Extension, Format};

impl MetadataManager {
    /// Add metadata to a file based on its format.
    ///
    /// This method automatically detects the file format and applies appropriate metadata.
    /// Use this for standalone files when you don't have format details.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to add metadata to
    /// * `video` - Video metadata to apply
    ///
    /// # Errors
    ///
    /// Returns an error if the file format is unsupported or if metadata writing fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::metadata::MetadataManager;
    /// # use yt_dlp::model::Video;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = MetadataManager::new();
    /// # let video: Video = todo!();
    /// // video obtained from a fetch_video_infos call
    /// manager.add_metadata("video.mp4", &video).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_metadata(&self, file_path: impl Into<PathBuf>, video: &Video) -> Result<()> {
        let file_path: std::path::PathBuf = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            video_id = %video.id,
            title = %video.title,
            "🏷️ Adding metadata to file"
        );

        let file_format = Self::get_file_extension(&file_path)?;

        let extension = Extension::from_str(&file_format).unwrap_or(Extension::Unknown);

        tracing::debug!(
            file_path = ?file_path,
            file_format = %file_format,
            extension = ?extension,
            "⚙️ Detected file format and extension"
        );

        let result = match extension {
            Extension::Mp3 => Self::add_metadata_to_mp3(&file_path, video, None, None).await,
            Extension::M4A | Extension::Mp4 => Self::add_metadata_to_m4a(&file_path, video, None, None, None).await,
            Extension::Webm => self.add_metadata_to_webm(&file_path, video, None, None, None).await,
            Extension::Flac | Extension::Ogg | Extension::Wav | Extension::Aac | Extension::Aiff => {
                Self::add_metadata_with_lofty(&file_path, video, None, None, &file_format).await
            }
            _ => {
                self.add_ffmpeg_metadata(&file_path, video, &file_format, None, None, None)
                    .await
            }
        };

        match &result {
            Ok(()) => tracing::debug!(
                file_path = ?file_path,
                video_id = %video.id,
                "✅ Metadata added successfully"
            ),
            Err(e) => tracing::warn!(
                file_path = ?file_path,
                video_id = %video.id,
                error = %e,
                "Failed to add metadata"
            ),
        }

        result
    }

    /// Add metadata to a file with format details for audio and video.
    ///
    /// This method should be used when you have detailed format information,
    /// typically for combined audio+video files. Technical metadata (resolution,
    /// codecs, bitrates) will be included for MP4 and WebM formats.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to add metadata to
    /// * `video` - Video metadata to apply
    /// * `video_format` - Optional video format details (for technical metadata)
    /// * `audio_format` - Optional audio format details (for technical metadata)
    ///
    /// # Errors
    ///
    /// Returns an error if the file format is unsupported or if metadata writing fails
    pub async fn add_metadata_with_format(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        let file_path: PathBuf = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            video_id = %video.id,
            title = %video.title,
            has_video_format = video_format.is_some(),
            has_audio_format = audio_format.is_some(),
            "🏷️ Adding metadata with format details to file"
        );

        let file_format = Self::get_file_extension(&file_path)?;

        let extension = Extension::from_str(&file_format).unwrap_or(Extension::Unknown);

        tracing::debug!(
            file_path = ?file_path,
            file_format = %file_format,
            extension = ?extension,
            "⚙️ Detected file format and extension"
        );

        let result = match extension {
            Extension::Mp3 => Self::add_metadata_to_mp3(&file_path, video, audio_format, None).await,
            Extension::M4A | Extension::Mp4 => {
                Self::add_metadata_to_m4a(&file_path, video, audio_format, video_format, None).await
            }
            Extension::Webm => {
                self.add_metadata_to_webm(&file_path, video, video_format, audio_format, None)
                    .await
            }
            Extension::Flac | Extension::Ogg | Extension::Wav | Extension::Aac | Extension::Aiff => {
                Self::add_metadata_with_lofty(&file_path, video, audio_format, None, &file_format).await
            }
            _ => {
                self.add_ffmpeg_metadata(&file_path, video, &file_format, video_format, audio_format, None)
                    .await
            }
        };

        match &result {
            Ok(()) => tracing::debug!(
                file_path = ?file_path,
                video_id = %video.id,
                "✅ Metadata with format added successfully"
            ),
            Err(e) => tracing::warn!(
                file_path = ?file_path,
                video_id = %video.id,
                error = %e,
                "Failed to add metadata with format"
            ),
        }

        result
    }

    /// Add a thumbnail to a file based on its format.
    ///
    /// Thumbnails are embedded in the file metadata. Supported formats: MP3, M4A, MP4, WebM, MKV
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to add thumbnail to
    /// * `thumbnail_path` - Path to the thumbnail image file
    ///
    /// # Errors
    ///
    /// Returns an error if the file format doesn't support thumbnails or if embedding fails
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::metadata::MetadataManager;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let manager = MetadataManager::new();
    /// manager
    ///     .add_thumbnail_to_file("video.mp3", "cover.jpg")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_thumbnail_to_file(
        &self,
        file_path: impl Into<PathBuf>,
        thumbnail_path: impl Into<PathBuf>,
    ) -> Result<()> {
        let file_path: PathBuf = file_path.into();
        let thumbnail_path: PathBuf = thumbnail_path.into();

        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            "🏷️ Adding thumbnail to file"
        );

        let file_format = Self::get_file_extension(&file_path)?;

        let extension = Extension::from_str(&file_format).unwrap_or(Extension::Unknown);

        tracing::debug!(
            file_path = ?file_path,
            file_format = %file_format,
            extension = ?extension,
            "⚙️ Detected file format for thumbnail"
        );

        let result = match extension {
            Extension::Mp3 => Self::add_thumbnail_to_mp3(&file_path, &thumbnail_path).await,
            Extension::M4A | Extension::Mp4 => Self::add_thumbnail_to_m4a(&file_path, &thumbnail_path).await,
            Extension::Webm => self.add_thumbnail_to_webm(&file_path, &thumbnail_path).await,
            Extension::Flac | Extension::Ogg | Extension::Wav | Extension::Aac | Extension::Aiff => {
                Self::add_thumbnail_with_lofty(&file_path, &thumbnail_path, &file_format).await
            }
            _ => {
                tracing::debug!(
                    file_format = %file_format,
                    "⚙️ Thumbnails not supported for file format"
                );
                Ok(())
            }
        };

        match &result {
            Ok(()) => tracing::debug!(
                file_path = ?file_path,
                "✅ Thumbnail added successfully"
            ),
            Err(e) => tracing::warn!(
                file_path = ?file_path,
                error = %e,
                "Failed to add thumbnail"
            ),
        }

        result
    }
}
