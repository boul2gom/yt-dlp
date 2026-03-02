//! Chapter metadata support using FFmpeg.
//!
//! This module provides functions to create and embed chapter markers
//! in video files using FFmpeg metadata format.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use uuid::Uuid;

use super::{BaseMetadata, MetadataManager};
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::model::Video;
use crate::model::chapter::Chapter;
use crate::utils::fs::remove_temp_file;

impl MetadataManager {
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
    pub async fn add_metadata_with_chapters(
        &self,
        file_path: impl Into<PathBuf>,
        video: &Video,
        video_format: Option<&crate::model::format::Format>,
        audio_format: Option<&crate::model::format::Format>,
    ) -> Result<()> {
        let path: PathBuf = file_path.into();

        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            has_chapters = !video.chapters.is_empty(),
            chapter_count = video.chapters.len(),
            has_video_format = video_format.is_some(),
            has_audio_format = audio_format.is_some(),
            "🏷️ Adding metadata with chapters"
        );

        // First add regular metadata
        self.add_metadata_with_format(&path, video, video_format, audio_format)
            .await?;

        // Then add chapters if available
        if !video.chapters.is_empty() {
            self.add_chapters_metadata(&path, &video.chapters).await?;
        }

        tracing::debug!(
            file_path = ?path,
            video_id = %video.id,
            "✅ Metadata with chapters added successfully"
        );

        Ok(())
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
    pub async fn add_chapters_metadata(&self, file_path: impl Into<PathBuf>, chapters: &[Chapter]) -> Result<()> {
        let path: PathBuf = file_path.into();

        if chapters.is_empty() {
            tracing::debug!(
                file_path = ?path,
                "🏷️ No chapters to add, skipping"
            );
            return Ok(());
        }

        tracing::debug!(
            file_path = ?path,
            chapter_count = chapters.len(),
            "🏷️ Adding chapters to video file"
        );

        // Determine file extension
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

        // Create temporary metadata file
        let temp_metadata_path = std::env::temp_dir().join(format!("chapters_{}.txt", Uuid::new_v4()));

        let chapters_clone = chapters.to_vec();
        let metadata_path_clone = temp_metadata_path.clone();

        let metadata_file = tokio::task::spawn_blocking(move || {
            Self::create_chapters_metadata_file(&chapters_clone, metadata_path_clone)
        })
        .await
        .map_err(|e| Error::runtime("create chapters metadata file", e))??;

        // Create temporary output file
        let temp_output_path = Self::create_temp_output_path(&path, extension)?;

        let input_str = path
            .to_str()
            .ok_or_else(|| Error::path_validation(&path, "Invalid input path"))?;
        let output_str = temp_output_path
            .to_str()
            .ok_or_else(|| Error::path_validation(&temp_output_path, "Invalid output path"))?;
        let metadata_str = metadata_file
            .to_str()
            .ok_or_else(|| Error::path_validation(&metadata_file, "Invalid metadata path"))?;

        // Build FFmpeg command — preserve global metadata from input 0, add chapters from input 1
        let ffmpeg_args = crate::executor::FfmpegArgs::new()
            .input(input_str)
            .input(metadata_str)
            .args(["-map_metadata", "0", "-map_chapters", "1"])
            .codec_copy()
            .output(output_str)
            .build();

        tracing::debug!(
            file_path = ?path,
            metadata_file = ?metadata_file,
            arg_count = ffmpeg_args.len(),
            "✂️ Running FFmpeg to embed chapters"
        );

        let executor = Executor::new(self.ffmpeg_path.clone(), ffmpeg_args, Duration::from_secs(120));

        let output = executor.execute().await;

        // Clean up temporary metadata file regardless of outcome
        remove_temp_file(&metadata_file).await;

        let output = match output {
            Ok(output) => output,
            Err(e) => {
                // Clean up temp output on execution failure
                if temp_output_path.exists() {
                    remove_temp_file(&temp_output_path).await;
                }
                return Err(e);
            }
        };

        if !output.code.eq(&0) {
            if temp_output_path.exists() {
                remove_temp_file(&temp_output_path).await;
            }
            return Err(Error::CommandFailed {
                command: "ffmpeg".to_string(),
                exit_code: output.code,
                stderr: output.stderr,
            });
        }

        // Replace original file with the one containing chapters
        tokio::fs::rename(&temp_output_path, &path)
            .await
            .map_err(|e| Error::io_with_path("replace original file with chapters", &path, e))?;

        tracing::debug!(
            file_path = ?path,
            chapter_count = chapters.len(),
            "✅ Chapters added successfully"
        );

        Ok(())
    }

    /// Creates a temporary FFMETADATA1 file containing both global metadata tags and chapters.
    ///
    /// The resulting file can be passed directly to `ffmpeg -i metadata.txt -map_metadata N
    /// -map_chapters N` in the combine command, enabling a single-pass mux + embed.
    ///
    /// # Arguments
    ///
    /// * `video` - The video whose metadata and chapters to embed
    ///
    /// # Errors
    ///
    /// Returns an error if the temp file cannot be created or written
    ///
    /// # Returns
    ///
    /// Path to the created temporary FFMETADATA1 file
    pub(crate) fn create_combined_metadata_file(video: &Video) -> Result<PathBuf> {
        let temp_path = std::env::temp_dir().join(format!("metadata_{}.txt", Uuid::new_v4()));

        tracing::debug!(
            video_id = %video.id,
            chapter_count = video.chapters.len(),
            temp_path = ?temp_path,
            "⚙️ Creating combined FFMETADATA1 file"
        );

        let mut file = fs::File::create(&temp_path)
            .map_err(|e| Error::io_with_path("create combined metadata file", &temp_path, e))?;

        writeln!(file, ";FFMETADATA1").map_err(|e| Error::io("write metadata header", e))?;

        // Write global metadata tags
        let metadata = Self::extract_basic_metadata(video);
        for (key, value) in &metadata {
            let escaped = value
                .replace('\\', "\\\\")
                .replace('=', "\\=")
                .replace(';', "\\;")
                .replace('#', "\\#")
                .replace('\n', "\\n");
            writeln!(file, "{}={}", key, escaped).map_err(|e| Error::io("write metadata entry", e))?;
        }

        // Write chapters (if any)
        for (idx, chapter) in video.chapters.iter().enumerate() {
            let start_us = (chapter.start_time * 1_000_000.0) as i64;
            let end_us = (chapter.end_time * 1_000_000.0) as i64;

            writeln!(file, "[CHAPTER]").map_err(|e| Error::io("write chapter marker", e))?;
            writeln!(file, "TIMEBASE=1/1000000").map_err(|e| Error::io("write timebase", e))?;
            writeln!(file, "START={}", start_us).map_err(|e| Error::io("write chapter start", e))?;
            writeln!(file, "END={}", end_us).map_err(|e| Error::io("write chapter end", e))?;

            if let Some(title) = &chapter.title {
                let escaped = title
                    .replace('\\', "\\\\")
                    .replace('=', "\\=")
                    .replace(';', "\\;")
                    .replace('#', "\\#")
                    .replace('\n', "\\n");
                writeln!(file, "title={}", escaped).map_err(|e| Error::io("write chapter title", e))?;
            } else {
                writeln!(file, "title=Chapter {}", idx + 1).map_err(|e| Error::io("write default chapter title", e))?;
            }
        }

        tracing::debug!(
            temp_path = ?temp_path,
            chapter_count = video.chapters.len(),
            "✅ Combined FFMETADATA1 file created"
        );

        Ok(temp_path)
    }

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
        output_path: impl Into<PathBuf>,
    ) -> Result<PathBuf> {
        let output_path: PathBuf = output_path.into();

        {
            let total_duration = chapters.last().map(|c| c.end_time).unwrap_or(0.0);
            tracing::debug!(
                output_path = ?output_path,
                chapter_count = chapters.len(),
                total_duration_secs = total_duration,
                "⚙️ Creating chapters metadata file"
            );
        }

        let mut file = fs::File::create(&output_path)
            .map_err(|e| Error::io_with_path("create chapters metadata file", &output_path, e))?;

        // Write FFmpeg metadata format header
        writeln!(file, ";FFMETADATA1").map_err(|e| Error::io("write metadata header", e))?;

        // Write each chapter
        for (idx, chapter) in chapters.iter().enumerate() {
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
                writeln!(file, "title={}", escaped_title).map_err(|e| Error::io("write chapter title", e))?;
            } else {
                writeln!(file, "title=Chapter {}", idx + 1).map_err(|e| Error::io("write default chapter title", e))?;
            }
        }

        tracing::debug!(
            output_path = ?output_path,
            chapter_count = chapters.len(),
            "✅ Chapters metadata file created"
        );

        Ok(output_path)
    }
}
