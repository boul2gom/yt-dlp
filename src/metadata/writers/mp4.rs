//! M4A/MP4 metadata support using mp4ameta.
//!
//! This module provides functions to add metadata and thumbnails to M4A/MP4 files
//! using the mp4ameta library.

use std::path::{Path, PathBuf};

use mp4ameta::Tag as MP4Tag;

use crate::error::{Error, Result};
use crate::metadata::{BaseMetadata, MetadataManager, PlaylistMetadata};
use crate::model::Video;
use crate::model::format::Format;

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
    /// * `playlist` - Optional playlist metadata (currently unused for MP4 format due to library limitations)
    ///
    /// # Errors
    ///
    /// Returns an error if MP4 tags cannot be read or written
    pub(crate) async fn add_metadata_to_m4a(
        file_path: impl Into<PathBuf>,
        video: &Video,
        audio_format: Option<&Format>,
        video_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let file_path = file_path.into();

        {
            let audio_bitrate = audio_format.and_then(|f| f.rates_info.audio_rate);
            let audio_codec = audio_format.and_then(|f| f.codec_info.audio_codec.as_deref());
            let video_resolution =
                video_format.and_then(|f| match (f.video_resolution.width, f.video_resolution.height) {
                    (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                    _ => None,
                });
            let playlist_title = _playlist.map(|p| &p.title);

            tracing::debug!(
                file_path = ?file_path,
                video_id = %video.id,
                title = %video.title,
                has_audio_format = audio_format.is_some(),
                audio_bitrate = ?audio_bitrate,
                audio_codec = ?audio_codec,
                has_video_format = video_format.is_some(),
                video_resolution = ?video_resolution,
                has_playlist = _playlist.is_some(),
                playlist_title = ?playlist_title,
                "🏷️ Adding metadata to M4A/MP4 file"
            );
        }

        // Prepare data for blocking thread
        let metadata = Self::extract_basic_metadata(video).into_iter().collect::<Vec<_>>();
        let has_format_info = audio_format.is_some() || video_format.is_some();
        let file_path_for_tracing = file_path.clone();
        let file_path_clone = file_path.clone();

        tokio::task::spawn_blocking(move || {
            // Load existing tag
            let mut tag = MP4Tag::read_from_path(&file_path_clone)
                .map_err(|e| Error::metadata("read MP4 tags", &file_path_clone, e.to_string()))?;

            // Add basic metadata
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
                    _ => {}
                }
            }

            // MP4 format has limited metadata support compared to ID3
            if has_format_info {
                tracing::debug!("⚙️ MP4 tag has limited support for technical metadata");
            }

            // Save the changes
            tag.write_to_path(&file_path)
                .map_err(|e| Error::metadata("write MP4 tags", &file_path, e.to_string()))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::runtime("write M4A/MP4 metadata", e))??;

        tracing::debug!(
            file_path = ?file_path_for_tracing,
            video_id = %video.id,
            "✅ Metadata added successfully to M4A/MP4 file"
        );

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
    pub(crate) async fn add_thumbnail_to_m4a(file_path: impl Into<PathBuf>, thumbnail_path: &Path) -> Result<()> {
        let file_path = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            "🏷️ Adding thumbnail to M4A/MP4 file"
        );

        // Read the image file content
        let image_data = tokio::fs::read(thumbnail_path)
            .await
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        // Determine image format from file extension
        let fmt = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("png") => mp4ameta::ImgFmt::Png,
            Some("jpg") | Some("jpeg") => mp4ameta::ImgFmt::Jpeg,
            Some("bmp") => mp4ameta::ImgFmt::Bmp,
            _ => mp4ameta::ImgFmt::Jpeg,
        };

        tracing::trace!(
            thumbnail_path = ?thumbnail_path,
            image_format = ?fmt,
            image_size_bytes = image_data.len(),
            "⚙️ Thumbnail loaded"
        );

        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            // Read the tag
            let mut tag = MP4Tag::read_from_path(&file_path_clone)
                .map_err(|e| Error::metadata("read MP4 tags", &file_path_clone, e.to_string()))?;

            // Create an Img object with the correct format
            let artwork = mp4ameta::Img::new(fmt, image_data);
            tag.set_artwork(artwork);

            // Write the tag back to the file
            tag.write_to_path(&file_path_clone)
                .map_err(|e| Error::metadata("write MP4 tags", &file_path_clone, e.to_string()))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::runtime("write M4A/MP4 thumbnail", e))??;

        tracing::debug!(
            file_path = ?file_path,
            "✅ Thumbnail added successfully to M4A/MP4 file"
        );

        Ok(())
    }
}
