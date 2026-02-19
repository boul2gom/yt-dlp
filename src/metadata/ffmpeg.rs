//! FFmpeg-based metadata support for WebM/MKV and generic formats.
//!
//! This module provides functions to add metadata and thumbnails using FFmpeg
//! for formats that don't have dedicated library support.

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::Format;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{BaseMetadata, MetadataManager, PlaylistMetadata};

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
    pub(super) async fn add_metadata_to_webm(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let path: PathBuf = file_path.into();
        #[cfg(feature = "tracing")]
        {
            let video_resolution = video_format.and_then(|f| {
                match (f.video_resolution.width, f.video_resolution.height) {
                    (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                    _ => None,
                }
            });
            let video_codec = video_format.and_then(|f| f.codec_info.video_codec.as_deref());
            let audio_bitrate = audio_format.and_then(|f| f.rates_info.audio_rate);
            let audio_codec = audio_format.and_then(|f| f.codec_info.audio_codec.as_deref());

            tracing::debug!(
                file_path = ?path,
                video_id = %video.id,
                title = %video.title,
                has_video_format = video_format.is_some(),
                video_resolution = ?video_resolution,
                video_codec = ?video_codec,
                has_audio_format = audio_format.is_some(),
                audio_bitrate = ?audio_bitrate,
                audio_codec = ?audio_codec,
                "Adding metadata to WebM/MKV file"
            );
        }

        Self::log_metadata_debug(format!("Adding metadata to WebM/MKV file: {:?}", path));

        let file_format = "webm";
        let temp_output_path = Self::create_temp_output_path(&path, file_format)?;

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
            let video_metadata = Self::extract_video_format_metadata(format);
            #[cfg(feature = "tracing")]
            tracing::trace!(
                video_metadata_count = video_metadata.len(),
                "Extracted video format metadata"
            );
            all_metadata.extend(video_metadata);
        }

        // Add audio format metadata if available
        if let Some(format) = audio_format {
            let audio_metadata = Self::extract_audio_format_metadata(format);
            #[cfg(feature = "tracing")]
            tracing::trace!(
                audio_metadata_count = audio_metadata.len(),
                "Extracted audio format metadata"
            );
            all_metadata.extend(audio_metadata);
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(
            total_metadata_count = all_metadata.len(),
            "Total metadata entries collected for WebM/MKV"
        );

        // Build FFmpeg metadata arguments for WebM format
        // WebM is based on Matroska format and uses specific metadata tags
        let metadata_args: Vec<String> = all_metadata
            .iter()
            .flat_map(|(key, value)| {
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
                // Pass -metadata:g and key=value as separate arguments so that
                // special characters in the value (e.g. |) are not misinterpreted
                vec![
                    "-metadata:g".to_string(),
                    format!("{}={}", matroska_key, value),
                ]
            })
            .collect();

        // Build the FFmpeg command
        self.run_ffmpeg_metadata_command(
            input_str,
            output_str,
            &temp_output_path,
            &path,
            metadata_args,
        )
        .await
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
    pub(super) async fn add_thumbnail_to_webm(
        &self,
        file_path: impl Into<PathBuf>,
        thumbnail_path: impl Into<PathBuf>,
    ) -> Result<()> {
        let file_path: PathBuf = file_path.into();
        let thumbnail_path: PathBuf = thumbnail_path.into();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            file_exists = file_path.exists(),
            thumbnail_exists = thumbnail_path.exists(),
            "Adding thumbnail to WebM/MKV file"
        );

        let file_path_str = file_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&file_path, "Invalid file path"))?;

        let thumbnail_path_str = thumbnail_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&thumbnail_path, "Invalid thumbnail path"))?;

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

        let temp_output_path = Self::create_temp_output_path(&file_path, "mkv")?;
        let temp_output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&temp_output_path, "Invalid output path"))?;

        args.push("-y".to_string());
        args.push(temp_output_str.to_string());

        #[cfg(feature = "tracing")]
        tracing::trace!(
            file_path = ?file_path,
            thumbnail_path = ?thumbnail_path,
            ffmpeg_path = ?self.ffmpeg_path,
            "Executing FFmpeg to add thumbnail"
        );

        let executor = Executor::new(self.ffmpeg_path.clone(), args, Duration::from_secs(60));

        let _ = executor.execute().await?;

        // Replace original file with the new one
        tokio::fs::rename(&temp_output_path, &file_path).await?;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            file_path = ?file_path,
            "Thumbnail added successfully to WebM/MKV file"
        );

        Ok(())
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
    pub(super) async fn add_ffmpeg_metadata(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        file_format: &str,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
    ) -> Result<()> {
        let path: std::path::PathBuf = file_path.into();
        #[cfg(feature = "tracing")]
        {
            let video_resolution = video_format.and_then(|f| {
                match (f.video_resolution.width, f.video_resolution.height) {
                    (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
                    _ => None,
                }
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
                "Adding metadata using FFmpeg"
            );
        }

        let temp_output_path = Self::create_temp_output_path(&path, file_format)?;

        let input_str = path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert input path to string".to_string()))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert output path to string".to_string()))?;

        // Collect all metadata
        let mut all_metadata = Self::extract_basic_metadata(video);

        if let Some(format) = video_format {
            let video_metadata = Self::extract_video_format_metadata(format);
            #[cfg(feature = "tracing")]
            tracing::trace!(
                video_metadata_count = video_metadata.len(),
                "Extracted video format metadata"
            );
            all_metadata.extend(video_metadata);
        }

        if let Some(format) = audio_format {
            let audio_metadata = Self::extract_audio_format_metadata(format);
            #[cfg(feature = "tracing")]
            tracing::trace!(
                audio_metadata_count = audio_metadata.len(),
                "Extracted audio format metadata"
            );
            all_metadata.extend(audio_metadata);
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(
            total_metadata_count = all_metadata.len(),
            "Total metadata entries collected"
        );

        // Build FFmpeg metadata arguments
        let metadata_args: Vec<String> = all_metadata
            .iter()
            // Pass -metadata and key=value as separate arguments so that
            // special characters in the value (e.g. |) are not misinterpreted
            .flat_map(|(key, value)| vec!["-metadata".to_string(), format!("{}={}", key, value)])
            .collect();

        self.run_ffmpeg_metadata_command(
            input_str,
            output_str,
            &temp_output_path,
            &path,
            metadata_args,
        )
        .await
        .map_err(|e| Error::Unknown(format!("Failed to run ffmpeg metadata command: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            "Metadata added successfully using FFmpeg"
        );

        Ok(())
    }
    /// Helper to execute the FFmpeg metadata command.
    ///
    /// # Arguments
    ///
    /// * `input_str` - Input file path string
    /// * `output_str` - Output file path string
    /// * `temp_output_path` - Temporary output path
    /// * `final_output_path` - Final output path
    /// * `metadata_args` - FFmpeg metadata arguments
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg execution fails
    async fn run_ffmpeg_metadata_command(
        &self,
        input_str: &str,
        output_str: &str,
        temp_output_path: &Path,
        final_output_path: &Path,
        metadata_args: Vec<String>,
    ) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            input = input_str,
            output = output_str,
            temp_output = ?temp_output_path,
            final_output = ?final_output_path,
            metadata_arg_count = metadata_args.len(),
            "Running FFmpeg metadata command"
        );

        let mut ffmpeg_args = vec!["-i".to_string(), input_str.to_string()];

        ffmpeg_args.extend(metadata_args);

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

        let executor = Executor::new(
            self.ffmpeg_path.clone(),
            ffmpeg_args,
            Duration::from_secs(120),
        );

        let output = executor.execute().await?;

        #[cfg(feature = "tracing")]
        tracing::trace!(
            exit_code = output.code,
            stdout_len = output.stdout.len(),
            stderr_len = output.stderr.len(),
            "FFmpeg command executed"
        );

        if !output.code.eq(&0) {
            if temp_output_path.exists() {
                let _ = tokio::fs::remove_file(temp_output_path).await;
            }
            return Err(Error::CommandFailed {
                command: "ffmpeg".to_string(),
                exit_code: output.code,
                stderr: output.stderr,
            });
        }

        tokio::fs::rename(temp_output_path, final_output_path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            output_path = ?final_output_path,
            "FFmpeg metadata command completed successfully"
        );

        Ok(())
    }
}
