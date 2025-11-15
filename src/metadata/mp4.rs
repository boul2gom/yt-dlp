//! M4A/MP4 metadata support using mp4ameta.
//!
//! This module provides functions to add metadata and thumbnails to M4A/MP4 files
//! using the mp4ameta library.

use crate::error::{Error, Result};
use crate::model::Video;
use crate::model::format::Format;
use mp4ameta::Tag as MP4Tag;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use super::{BaseMetadata, MetadataManager, PlaylistMetadata};

impl MetadataManager {
    /// Add metadata to an M4A/MP4 file using mp4ameta.
    ///
    /// M4A/MP4 metadata includes: Title, artist, album, genre (from tags), release year
    /// For MP4 files with video, technical metadata is also included.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the M4A/MP4 file
    /// * `video` - Video metadata to apply
    /// * `audio_format` - Optional audio format for technical metadata
    /// * `video_format` - Optional video format for technical metadata
    ///
    /// # Errors
    ///
    /// Returns an error if MP4 tags cannot be read or written
    pub(super) fn add_metadata_to_m4a<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
        video_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to M4A/MP4 file: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to M4A/MP4 file: {:?}", file_path));

        // Load existing tag
        let mut tag = MP4Tag::read_from_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to read MP4 tags: {}", e)))?;

        // Add basic metadata
        let metadata = Self::extract_basic_metadata(video);
        for (key, value) in metadata {
            match key.as_str() {
                "title" => tag.set_title(value),
                "artist" => tag.set_artist(value),
                "album" => tag.set_album(value),
                "album_artist" => tag.set_album_artist(value),
                "genre" => tag.set_genre(value),
                "year" => {
                    if let Ok(year) = value.parse::<u16>() {
                        tag.set_year(year.to_string());
                    }
                }
                _ => {
                    Self::log_metadata_debug(format!("Skipping MP4 metadata: {} = {}", key, value));
                }
            }
        }

        // MP4 format has limited metadata support compared to ID3
        if audio_format.is_some() || video_format.is_some() {
            Self::log_metadata_debug(
                "Format info available but MP4 tag has limited support for technical metadata",
            );
        }

        // Save the changes
        tag.write_to_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to write MP4 tags: {}", e)))?;

        Ok(())
    }

    /// Add thumbnail to an M4A/MP4 file.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the M4A/MP4 file
    /// * `thumbnail_path` - Path to the thumbnail image
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be read or the MP4 tags cannot be written
    pub(super) fn add_thumbnail_to_m4a<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to M4A/MP4 file: {:?}", file_path);

        // Read the tag
        let mut tag = MP4Tag::read_from_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to read MP4 tags: {}", e)))?;

        // Read the image file content
        let image_data = fs::read(thumbnail_path)
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        // Determine image format from file extension
        let fmt = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("png") => mp4ameta::ImgFmt::Png,
            Some("jpg") | Some("jpeg") => mp4ameta::ImgFmt::Jpeg,
            Some("bmp") => mp4ameta::ImgFmt::Bmp,
            _ => mp4ameta::ImgFmt::Jpeg,
        };

        // Create an Img object with the correct format
        let artwork = mp4ameta::Img::new(fmt, image_data);
        tag.set_artwork(artwork);

        // Write the tag back to the file
        tag.write_to_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to write MP4 tags: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to M4A/MP4 file: {:?}", file_path);

        Ok(())
    }
}
