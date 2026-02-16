//! MP3 metadata support using ID3 tags.
//!
//! This module provides functions to add metadata and thumbnails to MP3 files
//! using the ID3 tag format.

use crate::error::{Error, Result};
use crate::model::Video;
use crate::model::format::Format;
use id3::{Frame as ID3Frame, Tag as ID3Tag, TagLike, Version as ID3Version};
use std::fmt::Debug;
use std::path::Path;

use super::{BaseMetadata, MetadataManager, PlaylistMetadata};

impl MetadataManager {
    /// Add metadata to an MP3 file using ID3 tags.
    ///
    /// MP3 metadata includes: Title, artist, album, genre (from tags), release year
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the MP3 file
    /// * `video` - Video metadata to apply
    /// * `audio_format` - Optional audio format for technical metadata
    /// * `playlist` - Optional playlist metadata (album=playlist title, track=index)
    ///
    /// # Errors
    ///
    /// Returns an error if ID3 tags cannot be read or written
    pub(super) async fn add_metadata_to_mp3<P: AsRef<Path> + Debug + Copy + Send + Sync>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
        playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to MP3 file: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to MP3 file: {:?}", file_path));

        // Prepare data for the blocking thread (to avoid cloning the whole Video struct)
        let metadata = Self::extract_basic_metadata(video)
            .into_iter()
            .collect::<Vec<_>>();
        let playlist_info =
            playlist.map(|pl| (pl.title.clone(), pl.index, pl.total, pl.id.clone()));
        let audio_info =
            audio_format.map(|f| (f.rates_info.audio_rate, f.codec_info.audio_codec.clone()));
        let path = file_path.as_ref().to_path_buf();

        tokio::task::spawn_blocking(move || {
            // Load existing tag or create a new one
            let mut tag = match ID3Tag::read_from_path(&path) {
                Ok(tag) => tag,
                Err(_) => ID3Tag::new(),
            };

            // Add basic metadata
            for (key, value) in metadata {
                match key.as_str() {
                    "title" => tag.set_title(value),
                    "artist" => tag.set_artist(value),
                    "album" => {
                        // Override with playlist title if available
                        if let Some((ref pl_title, _, _, _)) = playlist_info {
                            tag.set_album(pl_title);
                        } else {
                            tag.set_album(value);
                        }
                    }
                    "album_artist" => tag.set_album_artist(value),
                    "genre" => tag.set_genre(value),
                    "year" => {
                        if let Ok(year) = value.parse::<i32>() {
                            tag.set_year(year)
                        }
                    }
                    _ => {
                        Self::log_metadata_debug(format!(
                            "Skipping ID3 metadata: {} = {}",
                            key, value
                        ));
                    }
                }
            }

            // Add playlist metadata if available
            if let Some((_, index, total, id)) = playlist_info {
                // Set track number (1-based index)
                tag.set_track(index as u32);
                if let Some(total) = total {
                    tag.set_total_tracks(total as u32);
                }

                // Add playlist ID as custom frame
                let frame = ID3Frame::text("TXXX", format!("Playlist ID: {}", id));
                tag.add_frame(frame);
            }

            // Add technical metadata if available (as custom frames)
            if let Some((audio_rate, audio_codec)) = audio_info {
                if let Some(rate) = audio_rate {
                    let frame = ID3Frame::text("TXXX", format!("Audio Bitrate: {}", rate));
                    tag.add_frame(frame);
                }

                if let Some(codec) = audio_codec {
                    let frame = ID3Frame::text("TXXX", format!("Audio Codec: {}", codec));
                    tag.add_frame(frame);
                }
            }

            // Save changes
            tag.write_to_path(&path, ID3Version::Id3v24)
                .map_err(|e| Error::Unknown(format!("Failed to write ID3 tags: {}", e)))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::Unknown(e.to_string()))??;

        Ok(())
    }

    /// Add thumbnail to an MP3 file using ID3 picture frame.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the MP3 file
    /// * `thumbnail_path` - Path to the thumbnail image
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be read or the ID3 tags cannot be written
    pub(super) async fn add_thumbnail_to_mp3<P: AsRef<Path> + Debug + Copy + Send + Sync>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to MP3 file: {:?}", file_path);

        // Read thumbnail content
        let image_data = tokio::fs::read(thumbnail_path)
            .await
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        // Determine MIME type based on file extension
        let mime_type = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("png") => "image/png",
            _ => "image/jpeg",
        };
        let mime_type = mime_type.to_string();

        let path = file_path.as_ref().to_path_buf();

        tokio::task::spawn_blocking(move || {
            // Load existing tag or create a new one
            let mut tag = match ID3Tag::read_from_path(&path) {
                Ok(tag) => tag,
                Err(_) => ID3Tag::new(),
            };

            // Create picture frame
            let picture = ID3Frame::with_content(
                "APIC",
                id3::frame::Content::Picture(id3::frame::Picture {
                    mime_type,
                    picture_type: id3::frame::PictureType::CoverFront,
                    description: String::new(),
                    data: image_data,
                }),
            );

            tag.add_frame(picture);

            // Save the tag
            tag.write_to_path(&path, ID3Version::Id3v24)
                .map_err(|e| Error::Unknown(format!("Failed to write ID3 tags: {}", e)))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::Unknown(e.to_string()))??;

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to MP3 file: {:?}", file_path);

        Ok(())
    }
}
