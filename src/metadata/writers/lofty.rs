//! Lofty-based metadata support for FLAC, OGG/Opus/Vorbis, WAV, AAC, and AIFF.
//!
//! This module provides functions to add metadata and thumbnails to audio formats
//! supported by the `lofty` crate. It complements the existing MP3 (id3) and
//! M4A/MP4 (mp4ameta) modules.

use std::path::{Path, PathBuf};

use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{MimeType, PictureType};
use lofty::prelude::Accessor;
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};
use tokio::task;

use crate::error::{Error, Result};
use crate::metadata::{BaseMetadata, MetadataManager, PlaylistMetadata};
use crate::model::Video;
use crate::model::format::Format;

/// Determine the preferred tag type for a given file extension.
fn preferred_tag_type(extension: &str) -> TagType {
    match extension {
        "flac" => TagType::VorbisComments,
        "ogg" | "oga" | "opus" => TagType::VorbisComments,
        "wav" => TagType::RiffInfo,
        "aac" => TagType::Id3v2,
        "aiff" | "aif" => TagType::Id3v2,
        _ => TagType::Id3v2,
    }
}

/// Map a metadata key to the corresponding lofty `ItemKey`.
fn map_item_key(key: &str) -> Option<ItemKey> {
    match key {
        "title" => Some(ItemKey::TrackTitle),
        "artist" => Some(ItemKey::TrackArtist),
        "album_artist" => Some(ItemKey::AlbumArtist),
        "album" => Some(ItemKey::AlbumTitle),
        "genre" => Some(ItemKey::Genre),
        "date" | "year" => Some(ItemKey::RecordingDate),
        "description" => Some(ItemKey::Description),
        "audio_codec" => Some(ItemKey::EncoderSoftware),
        _ => None,
    }
}

impl MetadataManager {
    /// Add metadata to a file supported by lofty (FLAC, OGG, WAV, AAC, AIFF).
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the audio file
    /// * `video` - Video metadata to apply
    /// * `audio_format` - Optional audio format for technical metadata
    /// * `playlist` - Optional playlist metadata
    /// * `extension` - The file extension string (e.g. "flac", "ogg")
    ///
    /// # Errors
    ///
    /// Returns an error if lofty cannot read or write the file tags.
    pub(crate) async fn add_metadata_with_lofty(
        file_path: impl Into<PathBuf>,
        video: &Video,
        audio_format: Option<&Format>,
        playlist: Option<&PlaylistMetadata>,
        extension: &str,
    ) -> Result<()> {
        let file_path = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            video_id = %video.id,
            extension = extension,
            "🏷️ Adding metadata via lofty"
        );

        let metadata = Self::extract_basic_metadata(video);
        let audio_metadata = audio_format
            .map(Self::extract_audio_format_metadata)
            .unwrap_or_default();
        let playlist_info = playlist.map(|pl| (pl.title.clone(), pl.index, pl.total));
        let tag_type = preferred_tag_type(extension);
        let file_path_clone = file_path.clone();

        task::spawn_blocking(move || {
            write_lofty_tags(
                &file_path_clone,
                tag_type,
                &metadata,
                &audio_metadata,
                playlist_info.as_ref(),
            )
        })
        .await
        .map_err(|e| Error::runtime("write lofty metadata", e))??;

        tracing::debug!(
            file_path = ?file_path,
            video_id = %video.id,
            "✅ Metadata added via lofty"
        );

        Ok(())
    }

    /// Add a thumbnail to a file supported by lofty (FLAC, OGG, WAV, AAC, AIFF).
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the audio file
    /// * `thumbnail_path` - Path to the thumbnail image
    /// * `extension` - The file extension string (e.g. "flac", "ogg")
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail cannot be read or the tags cannot be written.
    pub(crate) async fn add_thumbnail_with_lofty(
        file_path: impl Into<PathBuf>,
        thumbnail_path: &Path,
        extension: &str,
    ) -> Result<()> {
        let file_path = file_path.into();

        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            extension = extension,
            "🏷️ Adding thumbnail via lofty"
        );

        let image_data = tokio::fs::read(thumbnail_path)
            .await
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        let mime = match thumbnail_path.extension().and_then(|e| e.to_str()) {
            Some("png") => MimeType::Png,
            Some("bmp") => MimeType::Bmp,
            Some("gif") => MimeType::Gif,
            Some("tiff") | Some("tif") => MimeType::Tiff,
            _ => MimeType::Jpeg,
        };

        let tag_type = preferred_tag_type(extension);
        let file_path_clone = file_path.clone();

        task::spawn_blocking(move || {
            let mut tagged = Probe::open(&file_path_clone)
                .map_err(|e| Error::metadata("open file for lofty", &file_path_clone, e.to_string()))?
                .read()
                .map_err(|e| Error::metadata("read tags via lofty", &file_path_clone, e.to_string()))?;

            let tag = get_or_create_tag(&mut tagged, tag_type);

            let picture = lofty::picture::Picture::unchecked(image_data)
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build();
            tag.push_picture(picture);

            tagged
                .save_to_path(&file_path_clone, WriteOptions::default())
                .map_err(|e| Error::metadata("save lofty tags", &file_path_clone, e.to_string()))?;

            Ok::<_, Error>(())
        })
        .await
        .map_err(|e| Error::runtime("write lofty thumbnail", e))??;

        tracing::debug!(
            file_path = ?file_path,
            "✅ Thumbnail added via lofty"
        );

        Ok(())
    }
}

/// Get an existing tag of the given type, or insert a new one and return it.
fn get_or_create_tag(tagged: &mut lofty::file::TaggedFile, tag_type: TagType) -> &mut Tag {
    if tagged.tag(tag_type).is_none() {
        tagged.insert_tag(Tag::new(tag_type));
    }
    tagged.tag_mut(tag_type).unwrap()
}

/// Write metadata tags to a file using lofty (runs on a blocking thread).
fn write_lofty_tags(
    file_path: &Path,
    tag_type: TagType,
    metadata: &[(String, String)],
    audio_metadata: &[(String, String)],
    playlist_info: Option<&(String, usize, Option<usize>)>,
) -> Result<()> {
    let mut tagged = Probe::open(file_path)
        .map_err(|e| Error::metadata("open file for lofty", file_path, e.to_string()))?
        .read()
        .map_err(|e| Error::metadata("read tags via lofty", file_path, e.to_string()))?;

    let tag = get_or_create_tag(&mut tagged, tag_type);

    for (key, value) in metadata {
        apply_tag_field(tag, key, value);
    }

    for (key, value) in audio_metadata {
        apply_tag_field(tag, key, value);
    }

    if let Some((pl_title, index, total)) = playlist_info {
        tag.insert_text(ItemKey::AlbumTitle, pl_title.clone());
        tag.set_track(*index as u32);
        if let Some(total) = total {
            tag.set_track_total(*total as u32);
        }
    }

    tagged
        .save_to_path(file_path, WriteOptions::default())
        .map_err(|e| Error::metadata("save lofty tags", file_path, e.to_string()))?;

    Ok(())
}

/// Parses a date string into a lofty `Timestamp`.
///
/// Accepts formats: "YYYY", "YYYY-MM-DD", "YYYY-MM-DDTHH:MM:SS", "YYYYMMDD".
fn parse_timestamp(value: &str) -> Option<Timestamp> {
    let trimmed = value.trim();
    // "YYYY" — plain year
    if let Ok(year) = trimmed.parse::<u16>() {
        return Some(Timestamp {
            year,
            month: None,
            day: None,
            hour: None,
            minute: None,
            second: None,
        });
    }

    // "YYYYMMDD" — compact date
    if trimmed.len() == 8 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        let year = trimmed[0..4].parse::<u16>().ok()?;
        let month = trimmed[4..6].parse::<u8>().ok()?;
        let day = trimmed[6..8].parse::<u8>().ok()?;
        return Some(Timestamp {
            year,
            month: Some(month),
            day: Some(day),
            hour: None,
            minute: None,
            second: None,
        });
    }

    // "YYYY-MM-DD" or "YYYY-MM-DDTHH:MM:SS"
    let parts: Vec<&str> = trimmed.splitn(2, 'T').collect();
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    if date_parts.len() == 3 {
        let year = date_parts[0].parse::<u16>().ok()?;
        let month = date_parts[1].parse::<u8>().ok()?;
        let day = date_parts[2].parse::<u8>().ok()?;
        let (hour, minute, second) = if parts.len() == 2 {
            let time_parts: Vec<&str> = parts[1].split(':').collect();
            if time_parts.len() >= 3 {
                (
                    time_parts[0].parse::<u8>().ok(),
                    time_parts[1].parse::<u8>().ok(),
                    time_parts[2].parse::<u8>().ok(),
                )
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };
        return Some(Timestamp {
            year,
            month: Some(month),
            day: Some(day),
            hour,
            minute,
            second,
        });
    }

    None
}

/// Apply a single metadata key-value pair to a tag using known ItemKeys.
fn apply_tag_field(tag: &mut Tag, key: &str, value: &str) {
    // Handle date/year specially: parse into Timestamp
    if key == "year" || key == "date" {
        if let Some(ts) = parse_timestamp(value) {
            tag.set_date(ts);
        }
        return;
    }

    if let Some(item_key) = map_item_key(key) {
        // Use insert_unchecked to bypass tag type mapping checks,
        // since some keys may not have a direct mapping for all tag types.
        let item = TagItem::new(item_key, ItemValue::Text(value.to_string()));
        tag.insert_unchecked(item);
    }
}
