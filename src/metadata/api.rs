//! Public API methods for metadata management.
//!
//! This module provides the high-level public API for adding metadata
//! and thumbnails to downloaded files.

use crate::error::Result;
use crate::model::Video;
use crate::model::format::Format;
use std::fmt::Debug;
use std::path::Path;

use super::MetadataManager;

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
    pub async fn add_metadata(
        file_path: impl AsRef<Path> + Send + Sync,
        video: &Video,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to file: {:?}", file_path.as_ref());

        let file_format = Self::get_file_extension(file_path.as_ref())?;

        match file_format.as_str() {
            "mp3" => Self::add_metadata_to_mp3(file_path.as_ref(), video, None, None),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => {
                Self::add_metadata_to_m4a(file_path.as_ref(), video, None, None, None)
            }
            "webm" | "mkv" => {
                Self::add_metadata_to_webm(file_path.as_ref(), video, None, None, None).await
            }
            _ => {
                Self::add_ffmpeg_metadata(file_path.as_ref(), video, &file_format, None, None, None)
                    .await
            }
        }
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
        file_path: impl AsRef<Path>,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Adding metadata with format to file: {:?}",
            file_path.as_ref()
        );

        let file_format = Self::get_file_extension(file_path.as_ref())?;

        match file_format.as_str() {
            "mp3" => Self::add_metadata_to_mp3(file_path.as_ref(), video, audio_format, None),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => Self::add_metadata_to_m4a(
                file_path.as_ref(),
                video,
                audio_format,
                video_format,
                None,
            ),
            "webm" | "mkv" => {
                Self::add_metadata_to_webm(
                    file_path.as_ref(),
                    video,
                    video_format,
                    audio_format,
                    None,
                )
                .await
            }
            _ => {
                Self::add_ffmpeg_metadata(
                    file_path.as_ref(),
                    video,
                    &file_format,
                    video_format,
                    audio_format,
                    None,
                )
                .await
            }
        }
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
    pub async fn add_thumbnail_to_file(
        file_path: impl AsRef<Path> + Debug + Copy,
        thumbnail_path: impl AsRef<Path>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to file: {:?}", file_path.as_ref());

        let file_format = Self::get_file_extension(file_path.as_ref())?;

        match file_format.as_str() {
            "mp3" => Self::add_thumbnail_to_mp3(file_path.as_ref(), thumbnail_path.as_ref()),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => {
                Self::add_thumbnail_to_m4a(file_path.as_ref(), thumbnail_path.as_ref())
            }
            "webm" | "mkv" => {
                Self::add_thumbnail_to_webm(file_path.as_ref(), thumbnail_path.as_ref()).await
            }
            _ => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Thumbnails not supported for file format: {}", file_format);
                Ok(())
            }
        }
    }
}
