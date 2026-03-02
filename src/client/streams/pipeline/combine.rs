use std::path::{Path, PathBuf};

#[cfg(cache)]
use crate::cache::DownloadCache;
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::metadata::MetadataManager;
use crate::model::Video;
use crate::model::format::Format;
use crate::{Downloader, utils};

/// Returns the appropriate FFmpeg audio codec argument for muxing based on container compatibility.
///
/// Uses stream copy (`"copy"`) when the audio format is natively compatible with the output
/// container (e.g., AAC/M4A into MP4, Opus/WebM into WebM, any codec into MKV).
/// Falls back to `"aac"` re-encoding otherwise.
///
/// The optional `audio_codec` hint (e.g. `"mp4a.40.2"`, `"opus"`) takes precedence over
/// the file extension heuristic, providing robustness against extension deserialization issues.
fn audio_codec_for_mux(audio_path: &Path, output_path: &Path, audio_codec: Option<&str>) -> &'static str {
    let audio_ext = audio_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let output_ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Prefer codec metadata over extension heuristic (robust to unknown/mangled extensions)
    let is_aac = audio_codec
        .map(|c| c.contains("aac") || c.contains("mp4a"))
        .unwrap_or_else(|| matches!(audio_ext.as_str(), "m4a" | "aac"));
    let is_opus = audio_codec
        .map(|c| c.contains("opus"))
        .unwrap_or_else(|| matches!(audio_ext.as_str(), "webm" | "opus" | "ogg"));

    match output_ext.as_str() {
        "mp4" | "m4a" | "mov" if is_aac => "copy",
        "webm" if is_opus => "copy",
        // Matroska supports any codec natively
        "mkv" | "mka" => "copy",
        _ => "aac",
    }
}

impl Downloader {
    /// Downloads a video and splits it into one file per chapter using FFmpeg stream copy.
    ///
    /// Downloads the full video first, then extracts each chapter with
    /// `ffmpeg -ss {start} -t {duration} -c copy -avoid_negative_ts 1`.
    ///
    /// # Arguments
    ///
    /// * `video` - Video metadata including the chapter list.
    /// * `output_dir` - Directory to write chapter files into.
    ///
    /// # Errors
    ///
    /// Returns an error if the video has no chapters, the download fails, or any
    /// chapter extraction fails.
    ///
    /// # Returns
    ///
    /// A list of paths to the created chapter files, in chapter order.
    pub async fn split_by_chapters(
        &self,
        video: &Video,
        output_dir: impl AsRef<std::path::Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        if video.chapters.is_empty() {
            return Err(Error::invalid_partial_range("video has no chapters to split"));
        }

        let output_dir = output_dir.as_ref();
        tokio::fs::create_dir_all(output_dir)
            .await
            .map_err(|e| Error::io_with_path("creating chapter output directory", output_dir, e))?;

        let temp_name = format!("temp_full_{}.mp4", utils::fs::random_filename(8));
        let temp_path = self.download_video(video, &temp_name).await?;

        let chapter_count = video.chapters.len();
        let source_path = temp_path.clone();

        let operation = crate::events::PostProcessOperation::SplitChapters {
            source_path: source_path.clone(),
            chapter_count,
        };
        let start_time = std::time::Instant::now();

        self.emit_event(crate::events::DownloadEvent::PostProcessStarted {
            input_path: source_path.clone(),
            operation: operation.clone(),
        })
        .await;

        let ext = temp_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");

        let mut chapter_paths = Vec::with_capacity(chapter_count);

        for (i, chapter) in video.chapters.iter().enumerate() {
            let safe_title = utils::validation::sanitize_filename(chapter.title.as_deref().unwrap_or("chapter"));
            let chapter_filename = format!("{:02}_{}.{}", i + 1, safe_title, ext);
            let chapter_path = output_dir.join(&chapter_filename);

            tracing::debug!(
                chapter_index = i,
                title = chapter.title.as_deref().unwrap_or("unknown"),
                start_time = chapter.start_time,
                end_time = chapter.end_time,
                "✂️ Extracting chapter"
            );

            self.extract_time_range(&temp_path, &chapter_path, chapter.start_time, chapter.end_time)
                .await?;

            chapter_paths.push(chapter_path);
        }

        utils::remove_temp_file(&temp_path).await;

        let duration = start_time.elapsed();
        self.emit_event(crate::events::DownloadEvent::PostProcessCompleted {
            input_path: source_path,
            output_path: output_dir.to_path_buf(),
            operation,
            duration,
        })
        .await;

        tracing::info!(
            video_id = %video.id,
            chapter_count = chapter_count,
            output_dir = ?output_dir,
            "✅ Video split into chapter files"
        );

        Ok(chapter_paths)
    }

    /// Downloads two separate format streams (video + audio) and combines them with ffmpeg.
    ///
    /// This is useful when you want to manually select specific video and audio formats
    /// and have them merged into a single output file.
    ///
    /// # Arguments
    ///
    /// * `video_format` - The video format to download.
    /// * `audio_format` - The audio format to download.
    /// * `output_path` - The path to save the combined file to.
    ///
    /// # Returns
    ///
    /// The path to the combined output file.
    pub async fn download_and_combine_formats(
        &self,
        video_format: &Format,
        audio_format: &Format,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        // Generate temporary filenames
        let video_ext = video_format.download_info.ext.as_str();
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);
        let audio_ext = audio_format.download_info.ext.as_str();
        let audio_filename = format!("temp_audio_{}.{}", utils::fs::random_filename(8), audio_ext);

        // Download video and audio in parallel
        let (video_result, audio_result) = tokio::join!(
            self.download_format(video_format, &video_filename),
            self.download_format(audio_format, &audio_filename)
        );

        // Check results — clean up temp files on partial failure
        let video_temp_path = match video_result {
            Ok(path) => path,
            Err(e) => {
                if let Ok(audio_path) = audio_result {
                    utils::remove_temp_file(&audio_path).await;
                }
                return Err(e);
            }
        };
        let audio_temp_path = match audio_result {
            Ok(path) => path,
            Err(e) => {
                utils::remove_temp_file(&video_temp_path).await;
                return Err(e);
            }
        };

        // Combine audio and video
        let output_filename = output_path.file_name().and_then(|f| f.to_str()).unwrap_or("output.mp4");

        let combined_path = self
            .combine_audio_and_video(&audio_filename, &video_filename, output_filename)
            .await?;

        // If the user specified a different directory than output_dir, move the file
        if combined_path != output_path {
            utils::create_parent_dir(output_path).await?;
            if tokio::fs::rename(&combined_path, output_path).await.is_err() {
                // rename fails across filesystems, fall back to copy+delete
                tokio::fs::copy(&combined_path, output_path).await?;
                tokio::fs::remove_file(&combined_path).await?;
            }
        }

        // Clean up temporary files
        utils::remove_temp_file(&video_temp_path).await;
        utils::remove_temp_file(&audio_temp_path).await;

        Ok(output_path.to_path_buf())
    }

    /// Downloads two format streams in parallel and combines them with ffmpeg in a single pass,
    /// embedding video metadata and chapters at the same time.
    ///
    /// Unlike [`download_and_combine_formats`](Self::download_and_combine_formats), this method:
    /// - Selects a container-compatible audio codec to avoid re-encoding when possible
    /// - Embeds metadata (title, artist, chapters, etc.) in the same ffmpeg invocation
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata used to build the embedded FFMETADATA1 file.
    /// * `video_format` - The video format to download.
    /// * `audio_format` - The audio format to download.
    /// * `output_path` - The full path for the combined output file.
    ///
    /// # Returns
    ///
    /// The path to the combined output file.
    pub(crate) async fn download_and_combine_with_meta(
        &self,
        video: &Video,
        video_format: &Format,
        audio_format: &Format,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        let video_ext = video_format.download_info.ext.as_str();
        let audio_ext = audio_format.download_info.ext.as_str();
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);
        let audio_filename = format!("temp_audio_{}.{}", utils::fs::random_filename(8), audio_ext);

        // Download video and audio in parallel
        let (video_result, audio_result) = tokio::join!(
            self.download_format(video_format, &video_filename),
            self.download_format(audio_format, &audio_filename)
        );

        let video_temp_path = video_result?;
        let audio_temp_path = audio_result?;

        // Build FFMETADATA1 file with global metadata and chapters for a single-pass embed.
        // Errors are non-fatal: we fall back to combining without metadata.
        let video_clone = video.clone();
        let metadata_file =
            tokio::task::spawn_blocking(move || MetadataManager::create_combined_metadata_file(&video_clone))
                .await
                .ok()
                .and_then(|r| {
                    if let Err(ref e) = r {
                        tracing::warn!(error = %e, "Failed to build metadata file for combine");
                    }
                    r.ok()
                });

        let operation = crate::events::PostProcessOperation::CombineStreams {
            audio_path: audio_temp_path.clone(),
            video_path: video_temp_path.clone(),
        };
        let start_time = std::time::Instant::now();

        self.emit_event(crate::events::DownloadEvent::PostProcessStarted {
            input_path: audio_temp_path.clone(),
            operation: operation.clone(),
        })
        .await;

        utils::create_parent_dir(output_path).await?;

        let combine_result = self
            .execute_ffmpeg_combine(
                &audio_temp_path,
                &video_temp_path,
                output_path,
                metadata_file.as_deref(),
                audio_format.codec_info.audio_codec.as_deref(),
            )
            .await;

        // Always clean up temp files regardless of outcome
        utils::remove_temp_file(&video_temp_path).await;
        utils::remove_temp_file(&audio_temp_path).await;
        if let Some(ref meta) = metadata_file {
            utils::remove_temp_file(meta).await;
        }

        match combine_result {
            Err(e) => {
                self.emit_event(crate::events::DownloadEvent::PostProcessFailed {
                    input_path: audio_temp_path,
                    operation,
                    error: e.to_string(),
                })
                .await;
                Err(e)
            }
            Ok(()) => {
                let duration = start_time.elapsed();
                self.emit_event(crate::events::DownloadEvent::PostProcessCompleted {
                    input_path: audio_temp_path,
                    output_path: output_path.to_path_buf(),
                    operation,
                    duration,
                })
                .await;
                Ok(output_path.to_path_buf())
            }
        }
    }

    /// Combines the audio and video files into a single file.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `audio_file` - The name of the audio file to combine.
    /// * `video_file` - The name of the video file to combine.
    /// * `output_file` - The name of the output file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the audio and video files could not be combined.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::VideoSelection;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    /// let video = downloader.fetch_video_infos(url).await?;
    ///
    /// let audio_format = video.best_audio_format().unwrap();
    /// let audio_path = downloader
    ///     .download_format(&audio_format, "audio-stream.mp3")
    ///     .await?;
    ///
    /// let video_format = video.worst_video_format().unwrap();
    /// let format_path = downloader
    ///     .download_format(&video_format, "video-stream.mp4")
    ///     .await?;
    ///
    /// let output_path = downloader
    ///     .combine_audio_and_video("audio-stream.mp3", "video-stream.mp4", "my-output.mp4")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn combine_audio_and_video(
        &self,
        audio_file: impl AsRef<str>,
        video_file: impl AsRef<str>,
        output_file: impl AsRef<str>,
    ) -> Result<PathBuf> {
        let audio_path = self.output_dir.join(audio_file.as_ref());
        let video_path = self.output_dir.join(video_file.as_ref());
        let output_path = self.output_dir.join(output_file.as_ref());
        self.combine_audio_and_video_to_path(&audio_path, &video_path, &output_path)
            .await
    }

    /// Combines audio and video files into a single file at a specific path.
    ///
    /// Unlike [`combine_audio_and_video`](Self::combine_audio_and_video), this method uses
    /// the exact paths specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `audio_file` - The full path to the audio file.
    /// * `video_file` - The full path to the video file.
    /// * `output_file` - The full path for the combined output file.
    pub async fn combine_audio_and_video_to_path(
        &self,
        audio_file: impl Into<PathBuf>,
        video_file: impl Into<PathBuf>,
        output_file: impl Into<PathBuf>,
    ) -> Result<PathBuf> {
        let audio_path = audio_file.into();
        let video_path = video_file.into();
        let output_path = output_file.into();

        tracing::info!(
            audio_path = ?audio_path,
            video_path = ?video_path,
            output_path = ?output_path,
            "🎬 Combining audio and video"
        );

        let operation = crate::events::PostProcessOperation::CombineStreams {
            audio_path: audio_path.clone(),
            video_path: video_path.clone(),
        };
        let start_time = std::time::Instant::now();

        self.emit_event(crate::events::DownloadEvent::PostProcessStarted {
            input_path: audio_path.clone(),
            operation: operation.clone(),
        })
        .await;

        // Perform the combination with FFmpeg
        if let Err(e) = self
            .execute_ffmpeg_combine(&audio_path, &video_path, &output_path, None, None)
            .await
        {
            self.emit_event(crate::events::DownloadEvent::PostProcessFailed {
                input_path: audio_path,
                operation,
                error: e.to_string(),
            })
            .await;
            return Err(e);
        }

        // Add metadata to the combined file, propagating potential errors
        if let Err(e) = self
            .add_metadata_to_combined_file(&audio_path, &video_path, &output_path)
            .await
        {
            self.emit_event(crate::events::DownloadEvent::PostProcessFailed {
                input_path: audio_path,
                operation,
                error: e.to_string(),
            })
            .await;
            return Err(e);
        }

        let duration = start_time.elapsed();
        self.emit_event(crate::events::DownloadEvent::PostProcessCompleted {
            input_path: audio_path,
            output_path: output_path.clone(),
            operation,
            duration,
        })
        .await;

        Ok(output_path)
    }

    /// Executes the FFmpeg command to combine audio and video files.
    ///
    /// Selects the audio codec automatically: uses stream copy when the audio format is
    /// natively compatible with the output container (e.g., AAC into MP4, Opus into WebM),
    /// otherwise re-encodes to AAC. Optionally embeds a pre-built FFMETADATA1 file
    /// (metadata + chapters) in the same pass when `metadata_file` is provided.
    ///
    /// The optional `audio_codec_hint` (e.g. `"mp4a.40.2"`) takes precedence over the
    /// file-extension heuristic in [`audio_codec_for_mux`], providing robustness when the
    /// audio temp file has an unexpected extension.
    pub(crate) async fn execute_ffmpeg_combine(
        &self,
        audio_path: &Path,
        video_path: &Path,
        output_path: &Path,
        metadata_file: Option<&Path>,
        audio_codec_hint: Option<&str>,
    ) -> Result<()> {
        let audio = audio_path.to_str().ok_or_else(|| Error::PathValidation {
            path: audio_path.to_path_buf(),
            reason: "Non-UTF8 audio path".to_string(),
        })?;
        let video = video_path.to_str().ok_or_else(|| Error::PathValidation {
            path: video_path.to_path_buf(),
            reason: "Non-UTF8 video path".to_string(),
        })?;
        let output = output_path.to_str().ok_or_else(|| Error::PathValidation {
            path: output_path.to_path_buf(),
            reason: "Non-UTF8 output path".to_string(),
        })?;

        let audio_codec = audio_codec_for_mux(audio_path, output_path, audio_codec_hint);

        tracing::debug!(
            audio_path = ?audio_path,
            video_path = ?video_path,
            output_path = ?output_path,
            audio_codec = audio_codec,
            has_metadata = metadata_file.is_some(),
            ffmpeg_path = ?self.libraries.ffmpeg,
            timeout = ?self.timeout,
            "🎬 Executing FFmpeg combine operation"
        );

        let mut builder = crate::executor::FfmpegArgs::new().input(audio).input(video);

        if let Some(meta) = metadata_file {
            let meta_str = meta.to_str().ok_or_else(|| Error::PathValidation {
                path: meta.to_path_buf(),
                reason: "Non-UTF8 metadata path".to_string(),
            })?;
            builder = builder.input(meta_str);
        }

        builder = builder.args(["-map", "0:a", "-map", "1:v"]);

        if metadata_file.is_some() {
            builder = builder.args(["-map_metadata", "2", "-map_chapters", "2"]);
        }

        let args = builder
            .args(["-c:v", "copy", "-c:a", audio_codec])
            .output(output)
            .build();

        tracing::debug!(
            args = ?args,
            "🎬 FFmpeg combine command arguments"
        );

        let executor = Executor::new(self.libraries.ffmpeg.clone(), args, self.timeout);

        executor.execute().await?;

        tracing::info!(
            output_path = ?output_path,
            "✅ Audio and video combined successfully"
        );

        Ok(())
    }

    /// Adds metadata to the combined file by extracting the video ID and
    /// retrieving information from the original audio and video formats
    async fn add_metadata_to_combined_file(
        &self,
        audio_path: impl Into<PathBuf>,
        video_path: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> Result<()> {
        let audio_path: PathBuf = audio_path.into();
        let video_path: PathBuf = video_path.into();
        let output_path: PathBuf = output_path.into();

        let video_id = Self::extract_video_id_from_file_paths(video_path.as_path(), audio_path.as_path());

        if let Some(video_id) = video_id
            && let Some(video) = self.get_video_by_id(&video_id).await
        {
            tracing::debug!("🏷️ Adding metadata to combined file");

            cfg_if::cfg_if! {
                if #[cfg(cache)] {
                    let video_format = self.find_cached_format(video_path.clone()).await;
                    let audio_format = self.find_cached_format(audio_path.clone()).await;

                    // Add metadata (including chapters) to the combined file with full format information
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(_e) = metadata_manager.add_metadata_with_chapters(
                        &output_path,
                        &video,
                        video_format.as_ref(),
                        audio_format.as_ref(),
                    )
                    .await
                    {
                        tracing::warn!(error = %_e, "Failed to add metadata to combined file");
                    } else {
                        tracing::debug!("✅ Metadata (including chapters) added to combined file");
                    }
                } else {
                    // Without cache, we don't have format details, add basic metadata only
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(e) = metadata_manager.add_metadata(
                        output_path.as_path(),
                        &video,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "Failed to add basic metadata to combined file");
                    } else {
                        tracing::debug!("✅ Basic metadata added to combined file");
                    }
                }
            }
        }

        Ok(())
    }

    /// Extracts the video ID from audio and video file paths
    fn extract_video_id_from_file_paths(
        video_path: impl Into<PathBuf>,
        audio_path: impl Into<PathBuf>,
    ) -> Option<String> {
        let video_path: PathBuf = video_path.into();
        let audio_path: PathBuf = audio_path.into();

        tracing::trace!(
            video_path = ?video_path,
            audio_path = ?audio_path,
            "🔍 Extracting video ID from file paths"
        );

        let video_filename = video_path.as_path().file_name()?.to_str()?;

        if let Some(id) = utils::fs::extract_video_id(video_filename) {
            tracing::trace!(
                video_id = id,
                source = "video_path",
                "🔍 Video ID extracted from video filename"
            );
            return Some(id);
        }

        let audio_filename = audio_path.as_path().file_name()?.to_str()?;
        let id = utils::fs::extract_video_id(audio_filename);

        if let Some(ref id_str) = id {
            tracing::trace!(
                video_id = id_str,
                source = "audio_path",
                "🔍 Video ID extracted from audio filename"
            );
        } else {
            tracing::trace!("🔍 No video ID found in file paths");
        }

        id
    }

    /// Finds the format of a file in the cache if it exists
    #[cfg(cache)]
    async fn find_cached_format(&self, file_path: impl Into<PathBuf>) -> Option<Format> {
        let file_path: PathBuf = file_path.into();

        tracing::trace!(
            file_path = ?file_path,
            has_cache = self.cache.is_some(),
            "🔍 Looking up format in download cache"
        );

        let cache = self.cache.as_ref()?;
        let file_hash = match DownloadCache::calculate_file_hash(file_path.as_path()).await {
            Ok(hash) => {
                tracing::trace!(
                    file_path = ?file_path,
                    file_hash = hash,
                    "🔍 Calculated file hash for cache lookup"
                );
                hash
            }
            Err(_e) => {
                tracing::trace!(
                    file_path = ?file_path,
                    error = %_e,
                    "🔍 Failed to calculate file hash"
                );
                return None;
            }
        };

        if let Ok(Some((cached_file, _))) = cache.downloads.get_by_hash(&file_hash).await
            && let Some(format_json) = cached_file.format_json.as_deref()
            && let Ok(format) = serde_json::from_str::<Format>(format_json)
        {
            tracing::trace!(
                file_path = ?file_path,
                format_id = format.format_id,
                "🔍 Format found in cache"
            );
            return Some(format);
        }

        tracing::trace!(
            file_path = ?file_path,
            "🔍 Format not found in cache"
        );

        None
    }
}
