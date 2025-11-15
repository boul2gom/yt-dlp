//! FFmpeg-based metadata support for WebM/MKV and generic formats.
//!
//! This module provides functions to add metadata and thumbnails using FFmpeg
//! for formats that don't have dedicated library support.

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::Format;
use std::fmt::Debug;
use std::path::Path;
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
    pub(super) async fn add_metadata_to_webm<P: AsRef<Path> + Debug + Copy>(
        file_path: P,
        video: &Video,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
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
    pub(super) async fn add_thumbnail_to_webm<P: AsRef<Path> + Debug + Copy>(
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
    pub(super) async fn add_ffmpeg_metadata<P: AsRef<Path>>(
        file_path: P,
        video: &Video,
        file_format: &str,
        video_format: Option<&Format>,
        audio_format: Option<&Format>,
        _playlist: Option<&PlaylistMetadata>,
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
