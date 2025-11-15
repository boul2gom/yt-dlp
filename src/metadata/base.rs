//! Base metadata trait and common operations.
//!
//! This module provides the BaseMetadata trait with methods to extract and format
//! metadata from Video and Format objects.

use crate::model::{Video, format::Format};
use chrono::DateTime;

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
