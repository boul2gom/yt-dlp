use crate::Downloader;

use crate::executor::Executor;
use crate::metadata::MetadataManager;
use crate::model::format::Format;

use crate::utils;
use std::path::{Path, PathBuf};

impl Downloader {
    /// Adds format metadata based on the format type (audio-only, video-only, or both)
    /// This function is extracted to avoid code duplication
    pub(crate) async fn add_metadata_if_needed(
        &self,
        path: impl AsRef<Path>,
        format: &Format,
    ) -> crate::error::Result<()> {
        let format_type = format.format_type();
        let is_standalone_format = format_type.is_audio_and_video() || format_type.is_audio();

        if is_standalone_format {
            if let Some(video_id) = format.video_id.as_ref() {
                #[cfg(feature = "tracing")]
                tracing::debug!("Adding metadata to standalone format file");

                // Try to get video metadata from cache
                #[cfg(feature = "cache")]
                if let Some(cache) = &self.cache
                    && let Ok(cached_video) = cache.get_by_id(video_id).await
                    && let Ok(video) = cached_video.video()
                {
                    // Add metadata with format information
                    let metadata_manager = MetadataManager::new();
                    if let Err(_e) = metadata_manager
                        .add_metadata_with_format(path.as_ref(), &video, None, Some(format))
                        .await
                    {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to add metadata: {}", _e);
                    }
                }

                #[cfg(not(feature = "cache"))]
                {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Cache feature disabled, cannot retrieve video metadata");
                    let _ = video_id; // Suppress unused warning
                }
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Skipping metadata for non-standalone format: will be added after combining"
            );
        }

        Ok(())
    }

    /// Embeds subtitle files into a video file using ffmpeg.
    pub async fn embed_subtitles_in_video(
        &self,
        video_path: impl AsRef<Path>,
        subtitle_paths: &[PathBuf],
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        self.embed_subtitles_with_languages(video_path, subtitle_paths, &[], output)
            .await
    }

    /// Embeds a single subtitle file into a video file using ffmpeg.
    ///
    /// This is a convenience wrapper around `embed_subtitles_in_video`.
    pub async fn embed_subtitles(
        &self,
        video_path: impl AsRef<Path>,
        subtitle_path: impl AsRef<Path>,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        self.embed_subtitles_in_video(video_path, &[subtitle_path.as_ref().to_path_buf()], output)
            .await
    }

    /// Embeds subtitle files into a video file with language metadata using ffmpeg.
    pub async fn embed_subtitles_with_languages(
        &self,
        video_path: impl AsRef<Path>,
        subtitle_paths: &[PathBuf],
        language_codes: &[&str],
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        let video_path = video_path.as_ref();
        let output_path = self.output_dir.join(output.as_ref());

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Embedding {} subtitles into video {:?}",
            subtitle_paths.len(),
            video_path
        );

        // Build ffmpeg command
        let mut args = vec!["-i".to_string(), video_path.to_string_lossy().to_string()];

        // Add each subtitle file as input
        for subtitle_path in subtitle_paths {
            args.push("-i".to_string());
            args.push(subtitle_path.to_string_lossy().to_string());
        }

        // Map video and audio streams
        args.push("-map".to_string());
        args.push("0:v".to_string());
        args.push("-map".to_string());
        args.push("0:a".to_string());

        // Map subtitle streams
        for i in 0..subtitle_paths.len() {
            args.push("-map".to_string());
            args.push(format!("{}:s", i + 1));
        }

        // Add language metadata for each subtitle stream
        for (i, &language_code) in language_codes.iter().enumerate() {
            if i < subtitle_paths.len() {
                // Set language metadata for subtitle stream
                args.push(format!("-metadata:s:s:{}", i));
                args.push(format!("language={}", language_code));

                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Setting language {} for subtitle stream {}",
                    language_code,
                    i
                );
            }
        }

        // Copy codecs
        args.push("-c".to_string());
        args.push("copy".to_string());

        // Output file
        args.push(output_path.to_string_lossy().to_string());

        #[cfg(feature = "tracing")]
        tracing::debug!("Running ffmpeg with args: {:?}", args);

        let executor = Executor::new(
            self.libraries.ffmpeg.clone(),
            utils::to_owned(args),
            self.timeout,
        );

        executor.execute().await?;

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully embedded subtitles into video at {:?}",
            output_path
        );

        Ok(output_path)
    }
}
