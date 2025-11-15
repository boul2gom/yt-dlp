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

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::Format;
use chrono::DateTime;
use id3::{Frame as ID3Frame, Tag as ID3Tag, TagLike, Version as ID3Version};
use mp4ameta::Tag as MP4Tag;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

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
    fn default_ffmpeg_path() -> PathBuf {
        std::env::var("FFMPEG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("ffmpeg"))
    }
}

impl Default for MetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Common metadata operations shared across different file formats.
///
/// This trait provides methods to extract and format metadata from Video and Format objects.
pub trait BaseMetadata {
    /// Format a timestamp into a string according to a specified format.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - Unix timestamp to format
    /// * `format_str` - Format string (e.g., "%Y-%m-%d" for date, "%Y" for year)
    ///
    /// # Returns
    ///
    /// Formatted string if the timestamp is valid, None otherwise
    fn format_timestamp(timestamp: i64, format_str: &str) -> Option<String> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Formatting timestamp: {}", timestamp);

        DateTime::from_timestamp(timestamp, 0).map(|dt| dt.format(format_str).to_string())
    }

    /// Add metadata to a vector if the value exists.
    ///
    /// # Arguments
    ///
    /// * `metadata` - Vector to add the metadata to
    /// * `key` - Metadata key
    /// * `value` - Optional value to add
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

    /// Extract basic metadata from a video.
    ///
    /// Basic metadata includes: title, artist (channel), album, genre (from tags), date/year
    ///
    /// # Arguments
    ///
    /// * `video` - The video to extract metadata from
    ///
    /// # Returns
    ///
    /// Vector of (key, value) metadata pairs
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
        if video.upload_date > 0
            && let Some(date_str) = Self::format_timestamp(video.upload_date, "%Y-%m-%d")
        {
            metadata.push(("date".to_string(), date_str));

            if let Some(year_str) = Self::format_timestamp(video.upload_date, "%Y") {
                metadata.push(("year".to_string(), year_str));
            }
        }

        metadata
    }

    /// Extract video format metadata.
    ///
    /// Video format metadata includes: resolution, FPS, video codec, video bitrate
    ///
    /// # Arguments
    ///
    /// * `format` - The format to extract metadata from
    ///
    /// # Returns
    ///
    /// Vector of (key, value) metadata pairs
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

    /// Extract audio format metadata.
    ///
    /// Audio format metadata includes: audio bitrate, audio codec, audio channels, sample rate
    ///
    /// # Arguments
    ///
    /// * `format` - The format to extract metadata from
    ///
    /// # Returns
    ///
    /// Vector of (key, value) metadata pairs
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

impl BaseMetadata for MetadataManager {}

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
            "mp3" => Self::add_metadata_to_mp3(file_path.as_ref(), video, None),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => {
                Self::add_metadata_to_m4a(file_path.as_ref(), video, None, None)
            }
            "webm" | "mkv" => {
                Self::add_metadata_to_webm(file_path.as_ref(), video, None, None).await
            }
            _ => {
                Self::add_ffmpeg_metadata(file_path.as_ref(), video, &file_format, None, None).await
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
            "mp3" => Self::add_metadata_to_mp3(file_path.as_ref(), video, audio_format),
            "m4a" | "m4b" | "m4p" | "m4v" | "mp4" => {
                Self::add_metadata_to_m4a(file_path.as_ref(), video, audio_format, video_format)
            }
            "webm" | "mkv" => {
                Self::add_metadata_to_webm(file_path.as_ref(), video, video_format, audio_format)
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
    fn get_file_extension(file_path: impl AsRef<Path>) -> Result<String> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Getting file extension for {:?}", file_path.as_ref());

        let path = file_path.as_ref();
        let ext = path
            .extension()
            .ok_or_else(|| Error::path_validation(path, "File has no extension"))?
            .to_str()
            .ok_or_else(|| Error::path_validation(path, "Invalid characters in file extension"))?
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
    fn create_temp_output_path(file_path: impl AsRef<Path>, file_format: &str) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Creating temporary output path for {:?}",
            file_path.as_ref()
        );

        let path = file_path.as_ref();
        let parent_dir = path.parent().unwrap_or_else(|| Path::new(""));
        let uuid = Uuid::new_v4();

        if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            Ok(parent_dir.join(format!("{}_{}_temp.{}", file_stem, uuid, file_format)))
        } else {
            Ok(parent_dir.join(format!("output_{}_temp.{}", uuid, file_format)))
        }
    }

    /// Log metadata debug messages if tracing is enabled.
    fn log_metadata_debug<S: AsRef<str>>(_message: S) {
        #[cfg(feature = "tracing")]
        tracing::debug!("{}", _message.as_ref());
    }

    // ========================================================================
    // MP3 METADATA SUPPORT (ID3)
    // ========================================================================

    /// Add metadata to an MP3 file using ID3 tags.
    ///
    /// MP3 metadata includes: Title, artist, album, genre (from tags), release year
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the MP3 file
    /// * `video` - Video metadata to apply
    /// * `audio_format` - Optional audio format for technical metadata
    ///
    /// # Errors
    ///
    /// Returns an error if ID3 tags cannot be read or written
    fn add_metadata_to_mp3<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to MP3 file: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to MP3 file: {:?}", file_path));

        // Load existing tag or create a new one
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
                    Self::log_metadata_debug(format!("Skipping ID3 metadata: {} = {}", key, value));
                }
            }
        }

        // Add technical metadata if available (as custom frames)
        if let Some(format) = audio_format {
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
        tag.write_to_path(file_path.as_ref(), ID3Version::Id3v24)
            .map_err(|e| Error::Unknown(format!("Failed to write ID3 tags: {}", e)))?;

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
    fn add_thumbnail_to_mp3<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to MP3 file: {:?}", file_path);

        // Load existing tag or create a new one
        let mut tag = match ID3Tag::read_from_path(file_path.as_ref()) {
            Ok(tag) => tag,
            Err(_) => ID3Tag::new(),
        };

        // Read thumbnail content
        let image_data = std::fs::read(thumbnail_path)
            .map_err(|e| Error::io_with_path("read thumbnail", thumbnail_path, e))?;

        // Determine MIME type based on file extension
        let mime_type = match thumbnail_path.extension().and_then(|ext| ext.to_str()) {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("png") => "image/png",
            _ => "image/jpeg",
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

        tag.add_frame(picture);

        // Save the tag
        tag.write_to_path(file_path.as_ref(), ID3Version::Id3v24)
            .map_err(|e| Error::Unknown(format!("Failed to write ID3 tags: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::debug!("Added thumbnail to MP3 file: {:?}", file_path);

        Ok(())
    }

    // ========================================================================
    // M4A/MP4 METADATA SUPPORT (mp4ameta)
    // ========================================================================

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
    fn add_metadata_to_m4a<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        audio_format: Option<&Format>,
        video_format: Option<&Format>,
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
    fn add_thumbnail_to_m4a<P: AsRef<Path> + Debug + Copy>(
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

    // ========================================================================
    // WebM/MKV METADATA SUPPORT (FFmpeg)
    // ========================================================================

    /// Add metadata to a WebM/MKV file using FFmpeg.
    ///
    /// WebM/MKV metadata includes: All basic metadata (via Matroska format),
    /// plus technical information (resolution, FPS, codecs, bitrates, etc.)
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the WebM/MKV file
    /// * `video` - Video metadata to apply
    /// * `video_format` - Optional video format for technical metadata
    /// * `audio_format` - Optional audio format for technical metadata
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg command fails
    async fn add_metadata_to_webm<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding metadata to WebM/MKV file: {:?}", file_path);

        Self::log_metadata_debug(format!("Adding metadata to WebM/MKV file: {:?}", file_path));

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
                    "year" => "date",
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

        for arg in metadata_args {
            ffmpeg_args.push(arg);
        }

        ffmpeg_args.extend(vec![
            "-c".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0".to_string(),
            output_str.to_string(),
        ]);

        Self::log_metadata_debug(format!(
            "Running FFmpeg command with args: {:?}",
            ffmpeg_args
        ));

        let executor = Executor {
            executable_path: Self::default_ffmpeg_path(),
            timeout: Duration::from_secs(120),
            args: ffmpeg_args,
        };

        let output = executor.execute().await?;

        if !output.code.eq(&0) {
            if temp_output_path.exists() {
                let _ = tokio::fs::remove_file(&temp_output_path).await;
            }
            return Err(Error::CommandFailed {
                command: "ffmpeg".to_string(),
                exit_code: output.code,
                stderr: output.stderr,
            });
        }

        // Replace original file with the file containing metadata
        tokio::fs::rename(&temp_output_path, path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        Ok(())
    }

    /// Add thumbnail to a WebM/MKV file using FFmpeg.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the WebM/MKV file
    /// * `thumbnail_path` - Path to the thumbnail image
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg command fails
    async fn add_thumbnail_to_webm<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        thumbnail_path: &Path,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Adding thumbnail to WebM/MKV file: {:?}", file_path);

        let file_path_str = file_path
            .as_ref()
            .to_str()
            .ok_or_else(|| Error::path_validation(file_path.as_ref(), "Invalid file path"))?;

        let thumbnail_path_str = thumbnail_path
            .to_str()
            .ok_or_else(|| Error::path_validation(thumbnail_path, "Invalid thumbnail path"))?;

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

        let temp_output_path = Self::create_temp_output_path(file_path.as_ref(), "mkv")?;
        let temp_output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&temp_output_path, "Invalid output path"))?;

        args.push("-y".to_string());
        args.push(temp_output_str.to_string());

        let executor = Executor {
            executable_path: Self::default_ffmpeg_path(),
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

    // ========================================================================
    // GENERIC FFMPEG METADATA SUPPORT
    // ========================================================================

    /// Add metadata to a video file using FFmpeg (for formats not directly supported).
    ///
    /// This is a fallback method for formats that don't have dedicated support.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the video file
    /// * `video` - Video metadata to apply
    /// * `file_format` - File extension
    /// * `video_format` - Optional video format for technical metadata
    /// * `audio_format` - Optional audio format for technical metadata
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg command fails
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
        let temp_output_path = Self::create_temp_output_path(path, file_format)?;

        let input_str = path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert input path to string".to_string()))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert output path to string".to_string()))?;

        // Collect all metadata
        let mut all_metadata = Self::extract_basic_metadata(video);

        if let Some(format) = video_format {
            all_metadata.extend(Self::extract_video_format_metadata(format));
        }

        if let Some(format) = audio_format {
            all_metadata.extend(Self::extract_audio_format_metadata(format));
        }

        // Build FFmpeg metadata arguments
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .map(|(key, value)| format!("-metadata {}={}", key, value))
            .collect();

        let mut ffmpeg_args = vec!["-i".to_string(), input_str.to_string()];

        for arg in metadata_args {
            ffmpeg_args.push(arg);
        }

        ffmpeg_args.extend(vec![
            "-c".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0".to_string(),
            output_str.to_string(),
        ]);

        Self::log_metadata_debug(format!(
            "Running FFmpeg command with args: {:?}",
            ffmpeg_args
        ));

        let executor = Executor {
            executable_path: Self::default_ffmpeg_path(),
            timeout: Duration::from_secs(120),
            args: ffmpeg_args,
        };

        let output = executor.execute().await?;

        if !output.code.eq(&0) {
            if temp_output_path.exists() {
                let _ = tokio::fs::remove_file(&temp_output_path).await;
            }
            return Err(Error::CommandFailed {
                command: "ffmpeg".to_string(),
                exit_code: output.code,
                stderr: output.stderr,
            });
        }

        tokio::fs::rename(&temp_output_path, path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        Ok(())
    }
}
