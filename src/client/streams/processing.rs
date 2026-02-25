use std::path::PathBuf;

use crate::Downloader;
use crate::executor::Executor;
#[cfg(cache)]
use crate::metadata::MetadataManager;
use crate::model::format::Format;

impl Downloader {
    /// Embeds subtitle files into a video file using ffmpeg.
    ///
    /// # Arguments
    ///
    /// * `video_path` - Path to the input video file
    /// * `subtitle_paths` - Paths to subtitle files to embed
    /// * `output` - Output filename (relative to output_dir)
    ///
    /// # Returns
    ///
    /// Path to the output file with embedded subtitles
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg fails or paths are invalid
    pub async fn embed_subtitles_in_video(
        &self,
        video_path: impl Into<PathBuf>,
        subtitle_paths: &[PathBuf],
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        self.embed_subtitles_with_languages(video_path, subtitle_paths, &[], output)
            .await
    }

    /// Embeds a single subtitle file into a video file using ffmpeg.
    ///
    /// This is a convenience wrapper around `embed_subtitles_in_video`.
    ///
    /// # Arguments
    ///
    /// * `video_path` - Path to the input video file
    /// * `subtitle_path` - Path to the subtitle file to embed
    /// * `output` - Output filename (relative to output_dir)
    ///
    /// # Returns
    ///
    /// Path to the output file with embedded subtitle
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg fails or paths are invalid
    pub async fn embed_subtitles(
        &self,
        video_path: impl Into<PathBuf>,
        subtitle_path: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        self.embed_subtitles_in_video(video_path, &[subtitle_path.into()], output)
            .await
    }

    /// Embeds subtitle files into a video file with language metadata using ffmpeg.
    ///
    /// # Arguments
    ///
    /// * `video_path` - Path to the input video file
    /// * `subtitle_paths` - Paths to subtitle files to embed
    /// * `language_codes` - Language codes for each subtitle (optional)
    /// * `output` - Output filename (relative to output_dir)
    ///
    /// # Returns
    ///
    /// Path to the output file with embedded subtitles
    ///
    /// # Errors
    ///
    /// Returns an error if ffmpeg fails or paths are invalid
    pub async fn embed_subtitles_with_languages(
        &self,
        video_path: impl Into<PathBuf>,
        subtitle_paths: &[PathBuf],
        language_codes: &[&str],
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        let video_path: PathBuf = video_path.into();
        let output: PathBuf = output.into();
        let output_path = self.output_dir.join(&output);

        tracing::debug!(
            video_path = ?video_path,
            subtitle_count = subtitle_paths.len(),
            language_count = language_codes.len(),
            output_path = ?output_path,
            "💬 Embedding subtitles into video"
        );

        // Build ffmpeg command
        let mut builder = crate::executor::FfmpegArgs::new().input(video_path.to_string_lossy());

        for subtitle_path in subtitle_paths {
            builder = builder.input(subtitle_path.to_string_lossy());
        }

        builder = builder.args(["-map", "0:v", "-map", "0:a"]);

        // Map subtitle streams
        for i in 0..subtitle_paths.len() {
            builder = builder.args(["-map".to_string(), format!("{}:s", i + 1)]);
        }

        // Add language metadata for each subtitle stream
        for (i, &language_code) in language_codes.iter().enumerate() {
            if i < subtitle_paths.len() {
                builder = builder.args([format!("-metadata:s:s:{}", i), format!("language={}", language_code)]);

                tracing::debug!(
                    language = language_code,
                    stream_index = i,
                    subtitle_path = ?subtitle_paths.get(i),
                    "💬 Setting language metadata for subtitle stream"
                );
            }
        }

        let args = builder.codec_copy().output(output_path.to_string_lossy()).build();

        tracing::debug!(
            args = ?args,
            arg_count = args.len(),
            output_path = ?output_path,
            "⚙️ Running ffmpeg to embed subtitles"
        );

        let executor = Executor::new(self.libraries.ffmpeg.clone(), args, self.timeout);

        executor.execute().await?;

        tracing::info!(
            output_path = ?output_path,
            "✅ Successfully embedded subtitles into video"
        );

        Ok(output_path)
    }

    /// Adds format metadata based on the format type (audio-only, video-only, or both)
    /// This function is extracted to avoid code duplication
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to add metadata to
    /// * `format` - Format information
    ///
    /// # Returns
    ///
    /// `Ok(())` on success
    ///
    /// # Errors
    ///
    /// Returns an error if metadata addition fails
    pub(crate) async fn add_metadata_if_needed(
        &self,
        path: impl Into<PathBuf>,
        format: &Format,
    ) -> crate::error::Result<()> {
        let path: PathBuf = path.into();
        let format_type = format.format_type();
        let is_standalone_format = format_type.is_audio_and_video() || format_type.is_audio();

        tracing::debug!(
            path = ?path,
            format_id = %format.format_id,
            format_type = ?format_type,
            is_standalone = is_standalone_format,
            "🏷️ Checking if metadata should be added"
        );

        if is_standalone_format {
            if let Some(video_id) = format.video_id.as_ref() {
                tracing::debug!(
                    video_id = video_id,
                    format_id = %format.format_id,
                    "🏷️ Adding metadata to standalone format file"
                );

                // Try to get video metadata from cache
                #[cfg(cache)]
                if let Some(cache) = &self.cache
                    && let Ok(cached_video) = cache.videos.get_by_id(video_id).await
                    && let Ok(video) = cached_video.video()
                {
                    // Add metadata with format information
                    let metadata_manager = MetadataManager::new();
                    if let Err(_e) = metadata_manager
                        .add_metadata_with_format(path.clone(), &video, None, Some(format))
                        .await
                    {
                        tracing::warn!(
                            error = %_e,
                            path = ?path,
                            video_id = video_id,
                            "🏷️ Failed to add metadata"
                        );
                    } else {
                        tracing::debug!(
                            path = ?path,
                            video_id = video_id,
                            "✅ Successfully added metadata"
                        );

                        self.emit_event(crate::events::DownloadEvent::MetadataApplied {
                            path: path.clone(),
                            metadata_type: crate::events::MetadataType::Ffmpeg,
                        })
                        .await;
                    }
                }

                #[cfg(not(cache))]
                {
                    tracing::debug!(
                        video_id = video_id,
                        "⚙️ Cache feature disabled, cannot retrieve video metadata"
                    );
                }
            }
        } else {
            tracing::debug!(
                format_id = %format.format_id,
                format_type = ?format_type,
                "🏷️ Skipping metadata for non-standalone format (will be added after combining)"
            );
        }

        Ok(())
    }
}
