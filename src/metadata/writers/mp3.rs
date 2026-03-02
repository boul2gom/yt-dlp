//! MP3 metadata support using ID3 tags.
//!
//! This module provides functions to add metadata and thumbnails to MP3 files
//! using the ID3 tag format.

use std::path::{Path, PathBuf};

use id3::frame::{Content as ID3Content, ExtendedText as ID3ExtendedText};
use id3::{Frame as ID3Frame, Tag as ID3Tag, TagLike, Version as ID3Version};

use crate::error::{Error, Result};
use crate::metadata::{BaseMetadata, MetadataManager, PlaylistMetadata};
use crate::model::Video;
use crate::model::format::Format;

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
    pub(crate) async fn add_metadata_to_mp3(
        file_path: impl Into<PathBuf>,
        video: &Video,
        audio_format: Option<&Format>,
        playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let file_path = file_path.into();

        {
            let audio_bitrate = audio_format.and_then(|f| f.rates_info.audio_rate);
            let audio_codec = audio_format.and_then(|f| f.codec_info.audio_codec.as_deref());
            let playlist_title = playlist.map(|p| &p.title);
            let playlist_index = playlist.map(|p| p.index);

            tracing::debug!(
                file_path = ?file_path,
                video_id = %video.id,
                title = %video.title,
                has_audio_format = audio_format.is_some(),
                audio_bitrate = ?audio_bitrate,
                audio_codec = ?audio_codec,
                has_playlist = playlist.is_some(),
                playlist_title = ?playlist_title,
                playlist_index = ?playlist_index,
                "🏷️ Adding metadata to MP3 file"
            );
        }

        // Prepare data for the blocking thread (to avoid cloning the whole Video struct)
        let metadata = Self::extract_basic_metadata(video).into_iter().collect::<Vec<_>>();
        let playlist_info = playlist.map(|pl| (pl.title.clone(), pl.index, pl.total, pl.id.clone()));
        let audio_info = audio_format.map(|f| (f.rates_info.audio_rate, f.codec_info.audio_codec.clone()));
        let file_path_clone = file_path.clone();

        tokio::task::spawn_blocking(move || {
            let mut tag = ID3Tag::read_from_path(&file_path_clone).unwrap_or_else(|_| ID3Tag::new());
            apply_id3_metadata(&mut tag, &metadata, &playlist_info);
            apply_id3_technical_metadata(&mut tag, &audio_info);

            tag.write_to_path(&file_path_clone, ID3Version::Id3v24)
                .map_err(|e| Error::metadata("write ID3 tags", &file_path_clone, e.to_string()))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::runtime("write MP3 metadata", e))??;

        tracing::debug!(
            file_path = ?file_path,
            video_id = %video.id,
            "✅ Metadata added successfully to MP3 file"
        );

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
    pub(crate) async fn add_thumbnail_to_mp3(file_path: impl Into<PathBuf>, thumbnail_path: &Path) -> Result<()> {
        let file_path = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            "🏷️ Adding thumbnail to MP3 file"
        );

        // Read thumbnail content
        let image_data = tokio::fs::read(thumbnail_path)
            .await
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        // Determine MIME type based on file extension
        let mime_type = crate::utils::fs::determine_mime_type(thumbnail_path);

        tracing::trace!(
            thumbnail_path = ?thumbnail_path,
            mime_type = %mime_type,
            image_size_bytes = image_data.len(),
            "⚙️ Thumbnail loaded"
        );

        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            // Load existing tag or create a new one
            let mut tag = ID3Tag::read_from_path(&file_path_clone).unwrap_or_else(|_| ID3Tag::new());

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
            tag.write_to_path(&file_path_clone, ID3Version::Id3v24)
                .map_err(|e| Error::metadata("write ID3 tags", &file_path_clone, e.to_string()))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::runtime("write MP3 thumbnail", e))??;

        tracing::debug!(
            file_path = ?file_path,
            "✅ Thumbnail added successfully to MP3 file"
        );

        Ok(())
    }
}

fn apply_id3_metadata(
    tag: &mut ID3Tag,
    metadata: &[(String, String)],
    playlist_info: &Option<(String, usize, Option<usize>, String)>,
) {
    for (key, value) in metadata {
        match key.as_str() {
            "title" => tag.set_title(value),
            "artist" => tag.set_artist(value),
            "album" => {
                if let Some((pl_title, ..)) = playlist_info {
                    tag.set_album(pl_title);
                } else {
                    tag.set_album(value);
                }
            }
            "album_artist" => tag.set_album_artist(value),
            "genre" => tag.set_genre(value),
            "year" => {
                if let Ok(year) = value.parse::<i32>() {
                    tag.set_year(year);
                }
            }
            _ => {}
        }
    }

    if let Some((_, index, total, id)) = playlist_info {
        tag.set_track(*index as u32);
        if let Some(total) = total {
            tag.set_total_tracks(*total as u32);
        }

        let frame = ID3Frame::with_content(
            "TXXX",
            ID3Content::ExtendedText(ID3ExtendedText {
                description: "Playlist ID".to_string(),
                value: id.to_string(),
            }),
        );
        tag.add_frame(frame);
    }
}

fn apply_id3_technical_metadata(
    tag: &mut ID3Tag,
    audio_info: &Option<(Option<ordered_float::OrderedFloat<f64>>, Option<String>)>,
) {
    let Some((audio_rate, audio_codec)) = audio_info else {
        return;
    };

    if let Some(rate) = audio_rate {
        let frame = ID3Frame::with_content(
            "TXXX",
            ID3Content::ExtendedText(ID3ExtendedText {
                description: "Audio Bitrate".to_string(),
                value: rate.to_string(),
            }),
        );
        tag.add_frame(frame);
    }

    if let Some(codec) = audio_codec {
        let frame = ID3Frame::with_content(
            "TXXX",
            ID3Content::ExtendedText(ID3ExtendedText {
                description: "Audio Codec".to_string(),
                value: codec.to_string(),
            }),
        );
        tag.add_frame(frame);
    }
}
