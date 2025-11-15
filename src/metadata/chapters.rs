//! Chapter metadata support using FFmpeg.
//!
//! This module provides functions to create and embed chapter markers
//! in video files using FFmpeg metadata format.

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::chapter::Chapter;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

use super::MetadataManager;

impl MetadataManager {
    /// Create an FFmpeg metadata file with chapters.
    ///
    /// # Arguments
    ///
    /// * `chapters` - The chapters to write
    /// * `output_path` - Path where to write the metadata file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or written
    ///
    /// # Returns
    ///
    /// Path to the created metadata file
    pub(super) fn create_chapters_metadata_file(
        chapters: &[Chapter],
        output_path: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Creating chapters metadata file with {} chapters",
            chapters.len()
        );

        let metadata_path = output_path.as_ref();
        let mut file = fs::File::create(metadata_path)
            .map_err(|e| Error::io_with_path("create chapters metadata file", metadata_path, e))?;

        // Write FFmpeg metadata format header
        writeln!(file, ";FFMETADATA1").map_err(|e| Error::io("write metadata header", e))?;

        // Write each chapter
        for chapter in chapters {
            // Convert seconds to timebase (FFmpeg uses microseconds for chapters)
            let start_us = (chapter.start_time * 1_000_000.0) as i64;
            let end_us = (chapter.end_time * 1_000_000.0) as i64;

            writeln!(file, "[CHAPTER]").map_err(|e| Error::io("write chapter marker", e))?;
            writeln!(file, "TIMEBASE=1/1000000").map_err(|e| Error::io("write timebase", e))?;
            writeln!(file, "START={}", start_us).map_err(|e| Error::io("write start time", e))?;
            writeln!(file, "END={}", end_us).map_err(|e| Error::io("write end time", e))?;

            if let Some(title) = &chapter.title {
                // Escape special characters in title
                let escaped_title = title
                    .replace('\\', "\\\\")
                    .replace('=', "\\=")
                    .replace(';', "\\;")
                    .replace('#', "\\#")
                    .replace('\n', "\\n");
                writeln!(file, "title={}", escaped_title)
                    .map_err(|e| Error::io("write chapter title", e))?;
            } else {
                writeln!(
                    file,
                    "title=Chapter {}",
                    chapters.iter().position(|c| c == chapter).unwrap_or(0) + 1
                )
                .map_err(|e| Error::io("write default chapter title", e))?;
            }
        }

        #[cfg(feature = "tracing")]
        tracing::debug!("Created chapters metadata file at {:?}", metadata_path);

        Ok(metadata_path.to_path_buf())
    }

    /// Add chapters metadata to a video file using FFmpeg.
    ///
    /// This method embeds chapter markers into MP4/MKV/WebM files.
    /// Chapters allow media players to navigate to specific sections of the video.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the video file
    /// * `chapters` - The chapters to embed
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg fails or if the file cannot be processed
    ///
    /// # Returns
    ///
    /// Ok(()) if chapters were successfully embedded
    pub async fn add_chapters_metadata<P: AsRef<Path>>(
        file_path: P,
        chapters: &[Chapter],
    ) -> Result<()> {
        if chapters.is_empty() {
            #[cfg(feature = "tracing")]
            tracing::debug!("No chapters to add, skipping");
            return Ok(());
        }

        let path = file_path.as_ref();

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Adding {} chapters to video file: {:?}",
            chapters.len(),
            path
        );

        // Determine file extension
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

        // Create temporary metadata file
        let temp_metadata_path =
            std::env::temp_dir().join(format!("chapters_{}.txt", Uuid::new_v4()));
        let metadata_file = Self::create_chapters_metadata_file(chapters, &temp_metadata_path)?;

        // Create temporary output file
        let temp_output_path = Self::create_temp_output_path(path, extension)?;

        let input_str = path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert input path to string".to_string()))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::Unknown("Failed to convert output path to string".to_string()))?;
        let metadata_str = metadata_file.to_str().ok_or_else(|| {
            Error::Unknown("Failed to convert metadata path to string".to_string())
        })?;

        // Build FFmpeg command
        let ffmpeg_args = vec![
            "-i".to_string(),
            input_str.to_string(),
            "-i".to_string(),
            metadata_str.to_string(),
            "-map_metadata".to_string(),
            "1".to_string(), // Map metadata from second input (chapters file)
            "-map_chapters".to_string(),
            "1".to_string(), // Map chapters from second input
            "-c".to_string(),
            "copy".to_string(), // Copy streams without re-encoding
            output_str.to_string(),
        ];

        #[cfg(feature = "tracing")]
        tracing::debug!("Running FFmpeg with args: {:?}", ffmpeg_args);

        let executor = Executor {
            executable_path: Self::default_ffmpeg_path(),
            timeout: Duration::from_secs(120),
            args: ffmpeg_args,
        };

        let output = executor.execute().await;

        // Clean up temporary metadata file
        let _ = tokio::fs::remove_file(&metadata_file).await;

        let output = output?;

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

        // Replace original file with the one containing chapters
        tokio::fs::rename(&temp_output_path, path)
            .await
            .map_err(|e| Error::Unknown(format!("Failed to replace original file: {}", e)))?;

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully added {} chapters to video file",
            chapters.len()
        );

        Ok(())
    }

    /// Add both regular metadata and chapters to a video file.
    ///
    /// This is a convenience method that combines `add_metadata_with_format` and
    /// `add_chapters_metadata` in a single operation.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the video file
    /// * `video` - The video metadata
    /// * `video_format` - Optional video format for technical metadata
    /// * `audio_format` - Optional audio format for technical metadata
    ///
    /// # Errors
    ///
    /// Returns an error if metadata or chapters cannot be added
    pub async fn add_metadata_with_chapters<P: AsRef<Path> + std::fmt::Debug>(
        file_path: P,
        video: &Video,
        video_format: Option<&crate::model::format::Format>,
        audio_format: Option<&crate::model::format::Format>,
    ) -> Result<()> {
        let path = file_path.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!("Adding metadata with chapters for file: {:?}", path);

        // First add regular metadata
        Self::add_metadata_with_format(&file_path, video, video_format, audio_format).await?;

        // Then add chapters if available
        if !video.chapters.is_empty() {
            Self::add_chapters_metadata(path, &video.chapters).await?;
        }

        Ok(())
    }
}
