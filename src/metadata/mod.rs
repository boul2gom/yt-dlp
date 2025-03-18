//! Metadata management module for downloaded files.
//!
//! This module provides functionality to add metadata to downloaded files,
//! such as title, artist, album, etc.

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::Format;
use chrono::DateTime;
use id3::{Frame as ID3Frame, Tag as ID3Tag, TagLike, Version as ID3Version};
use mp4ameta;
use mp4ameta::Tag as MP4Tag;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

/// Metadata manager for handling file metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataManager {}

/// Common metadata operations shared across different file formats
pub trait BaseMetadata {
    /// Format a timestamp into a string according to a specified format
    fn format_timestamp(timestamp: i64, format_str: &str) -> Option<String> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Formatting timestamp: {}", timestamp);

        DateTime::from_timestamp(timestamp, 0).map(|dt| dt.format(format_str).to_string())
    }

    /// Add metadata to a vector if the value exists
    fn add_metadata_if_some<T: ToString>(
        metadata: &mut Vec<(String, String)>,
        key: &str,
        value: Option<T>,
    ) {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata if some: {}", key);

        if let Some(value) = value {
            metadata.push((key.to_string(), value.to_string()));
        }
    }

    /// Extract basic metadata from a video
    fn extract_basic_metadata(video: &Video) -> Vec<(String, String)> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Extracting basic metadata for video: {}", video.id);

        let mut metadata = vec![
            ("title".to_string(), video.title.clone()),
            ("artist".to_string(), video.channel.clone()),
            ("album_artist".to_string(), video.channel.clone()),
            ("album".to_string(), video.channel.clone()),
        ];

        // Add tags as genre
        if !video.tags.is_empty() {
            metadata.push(("genre".to_string(), video.tags.join(", ")));
        }

        // Add dates
        if video.upload_date > 0 {
            if let Some(date_str) = Self::format_timestamp(video.upload_date, "%Y-%m-%d") {
                metadata.push(("date".to_string(), date_str));

                if let Some(year_str) = Self::format_timestamp(video.upload_date, "%Y") {
                    metadata.push(("year".to_string(), year_str));
                }
            }
        }

        metadata
    }

    /// Extract video format metadata
    fn extract_video_format_metadata(format: &Format) -> Vec<(String, String)> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Extracting video format metadata: {}", format.format_id);

        let mut metadata = Vec::new();

        // Resolution
        if let (Some(width), Some(height)) = (
            format.video_resolution.width,
            format.video_resolution.height,
        ) {
            metadata.push(("resolution".to_string(), format!("{}x{}", width, height)));
        }

        // FPS
        Self::add_metadata_if_some(&mut metadata, "framerate", format.video_resolution.fps);

        // Video codec
        Self::add_metadata_if_some(
            &mut metadata,
            "video_codec",
            format.codec_info.video_codec.clone(),
        );

        // Video bitrate
        Self::add_metadata_if_some(&mut metadata, "video_bitrate", format.rates_info.video_rate);

        metadata
    }

    /// Extract audio format metadata
    fn extract_audio_format_metadata(format: &Format) -> Vec<(String, String)> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Extracting audio format metadata: {}", format.format_id);

        let mut metadata = Vec::new();

        // Audio bitrate
        Self::add_metadata_if_some(&mut metadata, "audio_bitrate", format.rates_info.audio_rate);

        // Audio codec
        Self::add_metadata_if_some(
            &mut metadata,
            "audio_codec",
            format.codec_info.audio_codec.clone(),
        );

        // Audio channels
        Self::add_metadata_if_some(
            &mut metadata,
            "audio_channels",
            format.codec_info.audio_channels,
        );

        // Sample rate
        Self::add_metadata_if_some(&mut metadata, "audio_sample_rate", format.codec_info.asr);

        metadata
    }
}

// Implementation of BaseMetadata for MetadataManager
impl BaseMetadata for MetadataManager {}

impl MetadataManager {
    /// Add metadata to a file based on its format.
    pub async fn add_metadata(file_path: impl AsRef<Path>, video: &Video) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to file: {:?}", file_path.as_ref());

        // Determine file format
        let file_format = Self::get_file_extension(file_path.as_ref())?;

        match file_format.as_str() {
            "mp3" => Self::add_metadata_to_mp3(file_path.as_ref(), video),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => {
                Self::add_metadata_to_m4a(file_path.as_ref(), video)
            }
            "webm" | "mkv" => Self::add_metadata_to_webm(file_path.as_ref(), video).await,
            _ => {
                Self::add_ffmpeg_metadata(file_path.as_ref(), video, &file_format, None, None).await
            }
        }
    }

    /// Add metadata to a file with format details for audio and video
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

        // Determine file format
        let file_format = Self::get_file_extension(file_path.as_ref())?;

        match file_format.as_str() {
            "mp3" => Self::add_metadata_to_mp3_with_format(file_path.as_ref(), video, audio_format),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => Self::add_metadata_to_m4a_with_format(
                file_path.as_ref(),
                video,
                audio_format,
                video_format,
            ),
            "webm" | "mkv" => {
                Self::add_metadata_to_webm_with_format(
                    file_path.as_ref(),
                    video,
                    video_format,
                    audio_format,
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
                )
                .await
            }
        }
    }

    /// Add metadata and thumbnail to a file based on its format.
    pub async fn add_metadata_with_thumbnail(
        file_path: impl AsRef<Path> + Debug + Copy,
        video: &Video,
        thumbnail_path: Option<impl AsRef<Path>>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Adding metadata with thumbnail to file: {:?}",
            file_path.as_ref()
        );

        // Add basic metadata first
        Self::add_metadata(file_path, video).await?;

        // Add thumbnail if provided, otherwise use video's thumbnail
        if let Some(thumbnail_path) = thumbnail_path {
            Self::add_thumbnail_to_file(file_path, thumbnail_path).await
        } else {
            // Try to get the best thumbnail from video
            if !video.thumbnails.is_empty() {
                let best_thumbnail = video
                    .thumbnails
                    .iter()
                    .max_by_key(|t| t.width.unwrap_or(0))
                    .ok_or(Error::MissingThumbnail)?;

                // Create a temporary file for the thumbnail
                let temp_dir = std::env::temp_dir();
                let thumbnail_file = temp_dir.join(format!("thumbnail_{}.jpg", video.id));

                // Download the thumbnail
                let fetcher = crate::fetcher::Fetcher::new(&best_thumbnail.url);
                fetcher.fetch_asset(thumbnail_file.clone()).await?;

                // Add the thumbnail to the file
                let result = Self::add_thumbnail_to_file(file_path, thumbnail_file.as_path()).await;

                // Clean up temporary file
                if let Err(_e) = tokio::fs::remove_file(&thumbnail_file).await {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Failed to remove temporary thumbnail file: {}", _e);
                }

                result
            } else {
                // No thumbnail available
                #[cfg(feature = "tracing")]
                tracing::debug!("No thumbnail available for video: {}", video.id);
                Ok(())
            }
        }
    }

    /// Add a thumbnail to a file based on its format.
    pub async fn add_thumbnail_to_file(
        file_path: impl AsRef<Path> + Debug + Copy,
        thumbnail_path: impl AsRef<Path>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to file: {:?}", file_path.as_ref());

        // Determine file format
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

    /// Add thumbnail to an MP3 file using ID3
    fn add_thumbnail_to_mp3<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to MP3 file: {:?}", file_path);

        // Try to load existing tag or create a new one
        let mut tag = match ID3Tag::read_from_path(file_path.as_ref()) {
            Ok(tag) => tag,
            Err(_) => ID3Tag::new(),
        };

        // Read thumbnail content
        let image_data = match std::fs::read(thumbnail_path) {
            Ok(data) => data,
            Err(e) => return Err(Error::IO(e)),
        };

        // Determine MIME type based on file extension
        let mime_type = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("png") => "image/png",
            _ => "image/jpeg", // Default to JPEG
        };

        // Create picture frame
        let picture = ID3Frame::with_content(
            "APIC",
            id3::frame::Content::Picture(id3::frame::Picture {
                mime_type: mime_type.to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: String::new(),
                data: image_data,
            }),
        );

        // Add the picture frame to the tag
        tag.add_frame(picture);

        // Save the tag
        if let Err(e) = tag.write_to_path(file_path.as_ref(), ID3Version::Id3v24) {
            return Err(Error::Unknown(format!("Failed to write ID3 tags: {}", e)));
        }

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to MP3 file: {:?}", file_path);

        Ok(())
    }

    /// Add thumbnail to an M4A file
    fn add_thumbnail_to_m4a<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to M4A file: {:?}", file_path);

        // Read the tag
        let mut tag = mp4ameta::Tag::read_from_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to read MP4 tags: {}", e)))?;

        // Read the image file content
        let image_data = fs::read(thumbnail_path).map_err(Error::IO)?;

        // Determine image format from file extension
        let fmt = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("png") => mp4ameta::ImgFmt::Png,
            Some("jpg") | Some("jpeg") => mp4ameta::ImgFmt::Jpeg,
            Some("bmp") => mp4ameta::ImgFmt::Bmp,
            _ => mp4ameta::ImgFmt::Jpeg, // Default to JPEG if unknown
        };

        // Create an Img object with the correct format
        let artwork = mp4ameta::Img::new(fmt, image_data);

        // Set the artwork (this will replace any existing artwork)
        tag.set_artwork(artwork);

        // Write the tag back to the file
        tag.write_to_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to write metadata to m4a file: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to M4A file: {:?}", file_path);

        Ok(())
    }

    /// Add thumbnail to a WebM/MKV file
    async fn add_thumbnail_to_webm<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to WebM/MKV file: {:?}", file_path);

        // For WebM/MKV, we'll use ffmpeg to add the thumbnail as an attachment
        let file_path_str = match file_path.as_ref().to_str() {
            Some(s) => s,
            None => return Err(Error::Path("Invalid file path".to_string())),
        };

        let thumbnail_path_str = match thumbnail_path.to_str() {
            Some(s) => s,
            None => return Err(Error::Path("Invalid thumbnail path".to_string())),
        };

        let mut args = vec![
            "-i".to_string(),
            file_path_str.to_string(),
            "-i".to_string(),
            thumbnail_path_str.to_string(),
            "-map".to_string(),
            "0".to_string(),
            "-map".to_string(),
            "1".to_string(),
            "-c".to_string(),
            "copy".to_string(),
            "-disposition:v:1".to_string(),
            "attached_pic".to_string(),
        ];

        // Create output file path
        let temp_output_path = Self::create_temp_output_path(file_path.as_ref(), "mkv")?;
        let temp_output_str = match temp_output_path.to_str() {
            Some(s) => s,
            None => return Err(Error::Path("Invalid output path".to_string())),
        };

        args.push("-y".to_string());
        args.push(temp_output_str.to_string());

        // Execute ffmpeg using Executor
        let executor = Executor {
            executable_path: PathBuf::from("ffmpeg"),
            timeout: Duration::from_secs(120),
            args,
        };

        let _ = executor.execute().await?;

        // Replace original file with the new one
        tokio::fs::rename(temp_output_path, file_path.as_ref()).await?;

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to WebM/MKV file: {:?}", file_path);

        Ok(())
    }

    /// Log metadata debug messages if tracing is enabled
    fn log_metadata_debug<S: AsRef<str>>(_message: S) {
        #[cfg(feature = "tracing")]
        tracing::debug!("{}", _message.as_ref());
    }

    /// Add metadata to an MP3 file using ID3
    fn add_metadata_to_mp3<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to MP3 file: {:?}", file_path);

        Self::add_metadata_to_mp3_with_format(file_path, video, None)
    }

    /// Add metadata to an MP3 file with format details
    fn add_metadata_to_mp3_with_format<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to MP3 file with format: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to MP3 file: {:?}", file_path));

        // Try to load existing tag or create a new one
        let mut tag = match ID3Tag::read_from_path(file_path.as_ref()) {
            Ok(tag) => tag,
            Err(_) => ID3Tag::new(),
        };

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
                    if let Ok(year) = value.parse::<i32>() {
                        tag.set_year(year)
                    }
                }
                _ => {
                    // Skip other metadata fields
                    Self::log_metadata_debug(format!("Skipping ID3 metadata: {} = {}", key, value));
                }
            }
        }

        // Add technical metadata if available
        if let Some(format) = audio_format {
            // Add a custom frame for audio quality information
            if let Some(audio_rate) = format.rates_info.audio_rate {
                let frame = ID3Frame::text("TXXX", format!("Audio Bitrate: {}", audio_rate));
                tag.add_frame(frame);
            }

            if let Some(audio_codec) = &format.codec_info.audio_codec {
                let frame = ID3Frame::text("TXXX", format!("Audio Codec: {}", audio_codec));
                tag.add_frame(frame);
            }
        }

        // Save changes
        let file_path_str = file_path
            .as_ref()
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert path to string".to_string()))?;

        tag.write_to_path(file_path_str, ID3Version::Id3v24)
            .map_err(|e| Error::Unknown(format!("Failed to write ID3 tags: {}", e)))?;

        Ok(())
    }

    /// Add metadata to an M4A file using MP4AMETA
    fn add_metadata_to_m4a<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to M4A file: {:?}", file_path);

        Self::add_metadata_to_m4a_with_format(file_path, video, None, None)
    }

    /// Add metadata to an M4A/MP4 file with format details
    fn add_metadata_to_m4a_with_format<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
        video_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Adding metadata to M4A/MP4 file with format: {:?}",
            file_path
        );

        Self::log_metadata_debug(format!("Adding metadata to M4A/MP4 file: {:?}", file_path));

        // Try to load existing tag
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
                    // Skip other metadata fields
                    Self::log_metadata_debug(format!("Skipping MP4 metadata: {} = {}", key, value));
                }
            }
        }

        // Add technical metadata
        // MP4 format has limited metadata support compared to ID3
        if let Some(_format) = audio_format {
            Self::log_metadata_debug(
                "Audio format info available but MP4 tag has limited support for technical metadata",
            );
        }

        if let Some(_format) = video_format {
            Self::log_metadata_debug(
                "Video format info available but MP4 tag has limited support for technical metadata",
            );
        }

        // Save the changes
        tag.write_to_path(file_path.as_ref())
            .map_err(|e| Error::Unknown(format!("Failed to write MP4 tags: {}", e)))?;

        Ok(())
    }

    /// Add metadata to a WebM file using FFmpeg
    async fn add_metadata_to_webm<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to WebM file: {:?}", file_path);

        Self::add_metadata_to_webm_with_format(file_path, video, None, None).await
    }

    /// Add metadata to a WebM file with format details
    async fn add_metadata_to_webm_with_format<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to WebM file with format: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to WebM file: {:?}", file_path));

        // WebM uses Matroska format, which is handled by FFmpeg with specific options
        // Create a temporary output file path
        let path = file_path.as_ref();
        let file_format = "webm";
        let temp_output_path = Self::create_temp_output_path(path, file_format)?;

        // Convert paths to strings
        let input_str = path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert input path to string".to_string()))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert output path to string".to_string()))?;

        // Collect all metadata
        let mut all_metadata = Self::extract_basic_metadata(video);

        // Add video format metadata if available
        if let Some(format) = video_format {
            all_metadata.extend(Self::extract_video_format_metadata(format));
        }

        // Add audio format metadata if available
        if let Some(format) = audio_format {
            all_metadata.extend(Self::extract_audio_format_metadata(format));
        }

        // Build FFmpeg metadata arguments for WebM format
        // WebM is based on Matroska format and uses specific metadata tags
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .map(|(key, value)| {
                // Map standard metadata keys to Matroska format keys
                let matroska_key = match key.as_str() {
                    "title" => "title",
                    "artist" => "artist",
                    "album_artist" => "album_artist",
                    "album" => "album",
                    "genre" => "genre",
                    "date" => "date",
                    "year" => "date", // Matroska uses date for year
                    "framerate" => "FRAMERATE",
                    "resolution" => "RESOLUTION",
                    "video_codec" => "ENCODER",
                    "audio_codec" => "ENCODER-AUDIO",
                    "video_bitrate" => "VIDEODATARATE",
                    "audio_bitrate" => "AUDIODATARATE",
                    "audio_channels" => "AUDIOCHANNELS",
                    "audio_sample_rate" => "AUDIOSAMPLERATE",
                    _ => key.as_str(),
                };
                format!("-metadata:g {}={}", matroska_key, value)
            })
            .collect();

        // Build the FFmpeg command
        let mut ffmpeg_args = vec!["-i".to_string(), input_str.to_string()];

        // Add metadata arguments
        for arg in metadata_args {
            ffmpeg_args.push(arg);
        }

        // Add output arguments
        ffmpeg_args.extend(vec![
            "-c".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0".to_string(),
            output_str.to_string(),
        ]);

        // Execute FFmpeg command
        Self::log_metadata_debug(format!(
            "Running FFmpeg command with args: {:?}",
            ffmpeg_args
        ));

        let executor = Executor {
            executable_path: PathBuf::from("ffmpeg"),
            timeout: Duration::from_secs(120),
            args: ffmpeg_args,
        };

        let output = executor.execute().await?;

        // Clean up temporary file if failure
        if !output.code.eq(&0) {
            // Clean up temporary file if it exists
            if temp_output_path.exists() {
                let _ = tokio::fs::remove_file(&temp_output_path).await;
            }

            return Err(Error::Command("FFmpeg command failed".to_string()));
        }

        // Replace original file with the file containing metadata
        tokio::fs::rename(&temp_output_path, path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        Ok(())
    }

    /// Add metadata to a video file using FFmpeg.
    async fn add_ffmpeg_metadata<P: AsRef<Path>>(
        file_path: P,
        video: &Video,
        file_format: &str,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata using FFmpeg: {:?}", file_path.as_ref());

        let path = file_path.as_ref();

        // Extract basic metadata
        let metadata = Self::extract_basic_metadata(video);

        // Create a temporary output file path
        let temp_output_path = Self::create_temp_output_path(path, file_format)?;

        // Convert paths to strings
        let input_str = path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert input path to string".to_string()))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert output path to string".to_string()))?;

        // Collect all metadata
        let mut all_metadata = metadata;

        // Add video format metadata if available
        if let Some(format) = video_format {
            all_metadata.extend(Self::extract_video_format_metadata(format));
        }

        // Add audio format metadata if available
        if let Some(format) = audio_format {
            all_metadata.extend(Self::extract_audio_format_metadata(format));
        }

        // Build FFmpeg metadata arguments
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .map(|(key, value)| format!("-metadata {}={}", key, value))
            .collect();

        // Build the FFmpeg command
        let mut ffmpeg_args = vec!["-i".to_string(), input_str.to_string()];

        // Add metadata arguments
        for arg in metadata_args {
            ffmpeg_args.push(arg);
        }

        // Add output arguments
        ffmpeg_args.extend(vec![
            "-c".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0".to_string(),
            output_str.to_string(),
        ]);

        // Execute FFmpeg command
        Self::log_metadata_debug(format!(
            "Running FFmpeg command with args: {:?}",
            ffmpeg_args
        ));

        let executor = Executor {
            executable_path: PathBuf::from("ffmpeg"),
            timeout: Duration::from_secs(120),
            args: ffmpeg_args,
        };

        let output = executor.execute().await?;

        // Clean up temporary file if failure
        if !output.code.eq(&0) {
            // Clean up temporary file if it exists
            if temp_output_path.exists() {
                let _ = tokio::fs::remove_file(&temp_output_path).await;
            }

            return Err(Error::Command("FFmpeg command failed".to_string()));
        }

        // Replace original file with the file containing metadata
        tokio::fs::rename(&temp_output_path, path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        Ok(())
    }

    /// Create a temporary output path for metadata processing
    fn create_temp_output_path(file_path: impl AsRef<Path>, file_format: &str) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Creating temporary output path for {:?}",
            file_path.as_ref()
        );

        // Extract the parent directory
        let path = file_path.as_ref();
        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));

        // Generate a unique temporary filename
        let uuid = Uuid::new_v4();

        if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            Ok(parent_dir.join(format!("{}_{}_temp.{}", file_stem, uuid, file_format)))
        } else {
            Ok(parent_dir.join(format!("output_{}_temp.{}", uuid, file_format)))
        }
    }

    /// Get the file extension from a path
    fn get_file_extension(file_path: impl AsRef<Path>) -> Result<String> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Getting file extension for {:?}", file_path.as_ref());

        // Get file extension
        let ext = file_path
            .as_ref()
            .extension()
            .ok_or_else(|| Error::Path("File has no extension".to_string()))?
            .to_str()
            .ok_or_else(|| Error::Path("Invalid characters in file extension".to_string()))?
            .to_lowercase();

        Ok(ext)
    }
}
