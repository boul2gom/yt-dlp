//! FFmpeg-based metadata support for WebM/MKV and generic formats.
//!
//! This module provides functions to add metadata and thumbnails using FFmpeg
//! for formats that don't have dedicated library support.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::metadata::{BaseMetadata, MetadataManager, PlaylistMetadata};
use crate::model::Video;
use crate::model::format::Format;

impl MetadataManager {
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
    pub(crate) async fn add_metadata_to_webm(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let path: PathBuf = file_path.into();
        tracing::debug!(file_path = ?path, "🏷️ Adding metadata to WebM/MKV file");

        let file_format = "webm";

        // Collect all metadata
        let all_metadata = self.prepare_and_collect_metadata(&path, video, file_format, video_format, audio_format);

        // Build FFmpeg metadata arguments for WebM format
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .flat_map(|(key, value)| {
                let matroska_key = match key.as_str() {
                    "title" => "title",
                    "artist" => "artist",
                    "album_artist" => "album_artist",
                    "album" => "album",
                    "genre" => "genre",
                    "date" => "date",
                    "year" => "DATE_RECORDED",
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
                vec!["-metadata:g".to_string(), format!("{}={}", matroska_key, value)]
            })
            .collect();

        self.run_metadata_task(&path, &video.id, file_format, metadata_args, Duration::from_secs(120))
            .await
    }

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
    pub(crate) async fn add_ffmpeg_metadata(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        file_format: &str,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let path: PathBuf = file_path.into();

        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            file_format = file_format,
            "🏷️ Adding metadata using FFmpeg fallback"
        );

        // Collect all metadata
        let all_metadata = self.prepare_and_collect_metadata(&path, video, file_format, video_format, audio_format);

        // Build FFmpeg metadata arguments
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .flat_map(|(key, value)| vec!["-metadata".to_string(), format!("{}={}", key, value)])
            .collect();

        self.run_metadata_task(&path, &video.id, file_format, metadata_args, Duration::from_secs(120))
            .await?;

        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            "✅ Metadata added successfully using FFmpeg"
        );

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
    pub(crate) async fn add_thumbnail_to_webm(
        &self,
        file_path: impl Into<PathBuf>,
        thumbnail_path: impl Into<PathBuf>,
    ) -> Result<()> {
        let file_path: PathBuf = file_path.into();
        let thumbnail_path: PathBuf = thumbnail_path.into();

        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            file_exists = file_path.exists(),
            thumbnail_exists = thumbnail_path.exists(),
            "🏷️ Adding thumbnail to WebM/MKV file"
        );

        let file_path_str = file_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&file_path, "Invalid file path"))?;

        let thumbnail_path_str = thumbnail_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&thumbnail_path, "Invalid thumbnail path"))?;

        let args = crate::executor::FfmpegArgs::new()
            .input(file_path_str)
            .input(thumbnail_path_str)
            .args(["-map", "0", "-map", "1"])
            .codec_copy()
            .args(["-disposition:v:1", "attached_pic"]);

        self.run_ffmpeg_task(&file_path, "mkv", args, Duration::from_secs(60))
            .await?;

        tracing::debug!(
            file_path = ?file_path,
            "✅ Thumbnail added successfully to WebM/MKV file"
        );

        Ok(())
    }

    /// Prepare and collect metadata for a video.
    ///
    /// This function collects all metadata from the video and formats it for FFmpeg.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the video file
    /// * `video` - Video metadata to apply
    /// * `file_format` - Format of the video file
    /// * `video_format` - Optional video format for technical metadata
    /// * `audio_format` - Optional audio format for technical metadata
    ///
    /// # Returns
    ///
    /// A vector of tuples containing metadata key-value pairs
    fn prepare_and_collect_metadata(
        &self,
        path: &Path,
        video: &Video,
        file_format: &str,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
    ) -> Vec<(String, String)> {
        let video_resolution = video_format.and_then(|f| match (f.video_resolution.width, f.video_resolution.height) {
            (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
            _ => None,
        });
        let video_codec = video_format.and_then(|f| f.codec_info.video_codec.as_deref());
        let audio_bitrate = audio_format.and_then(|f| f.rates_info.audio_rate);
        let audio_codec = audio_format.and_then(|f| f.codec_info.audio_codec.as_deref());

        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            title = %video.title,
            file_format = file_format,
            has_video_format = video_format.is_some(),
            video_resolution = ?video_resolution,
            video_codec = ?video_codec,
            has_audio_format = audio_format.is_some(),
            audio_bitrate = ?audio_bitrate,
            audio_codec = ?audio_codec,
            "⚙️ Preparing metadata for FFmpeg"
        );

        let mut all_metadata = Self::extract_basic_metadata(video);

        if let Some(format) = video_format {
            let video_metadata = Self::extract_video_format_metadata(format);
            all_metadata.extend(video_metadata);
        }

        if let Some(format) = audio_format {
            let audio_metadata = Self::extract_audio_format_metadata(format);
            all_metadata.extend(audio_metadata);
        }

        all_metadata
    }

    /// Helper to run FFmpeg for metadata tasks with unified command structure.
    async fn run_metadata_task(
        &self,
        path: &Path,
        video_id: &str,
        file_format: &str,
        metadata_args: Vec<String>,
        timeout: Duration,
    ) -> Result<()> {
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::path_validation(path, "Invalid file path"))?;

        let args = crate::executor::FfmpegArgs::new()
            .input(path_str)
            .args(metadata_args)
            .codec_copy()
            .args(["-map", "0"]);

        self.run_ffmpeg_task(path, file_format, args, timeout).await?;

        tracing::debug!(
            file_path = ?path,
            video_id = %video_id,
            "✅ FFmpeg metadata task completed"
        );

        Ok(())
    }

    /// Internal helper to run FFmpeg for metadata or thumbnails.
    async fn run_ffmpeg_task(
        &self,
        base_path: &Path,
        extension: &str,
        args: crate::executor::FfmpegArgs,
        timeout: Duration,
    ) -> Result<()> {
        crate::executor::run_ffmpeg_with_tempfile(&self.ffmpeg_path, base_path, extension, args, timeout).await
    }
}
