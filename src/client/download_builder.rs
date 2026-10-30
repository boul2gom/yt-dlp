//! Download builder for fluent download API.
//!
//! This module provides a builder pattern for configuring and executing downloads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::client::Downloader;
use crate::client::stream_downloads::clip_stream;
use crate::client::streams::selection::VideoSelection;
use crate::download::engine::partial::PartialRange;
use crate::download::{DownloadPriority, DownloadStatus};
use crate::error::Result;
use crate::model::Video;
use crate::model::format::{Format, FormatType, HttpHeaders};
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, StoryboardQuality, ThumbnailQuality, VideoCodecPreference, VideoQuality,
};

/// IDs returned after enqueueing both video and audio downloads.
struct EnqueuedDownloads {
    video_id: u64,
    audio_id: u64,
}

/// Selected stream URLs and format metadata for a partial-clip attempt.
struct PartialClipStreams<'a> {
    video_url: &'a str,
    audio_url: &'a str,
    video_format: &'a Format,
    audio_format: &'a Format,
}

/// Paths, headers, and identifiers needed to enqueue both download streams.
struct EnqueueStreams<'a> {
    video_url: &'a str,
    audio_url: &'a str,
    video_path: PathBuf,
    audio_path: PathBuf,
    video_headers: HttpHeaders,
    audio_headers: HttpHeaders,
    video_available_at: Option<i64>,
    audio_available_at: Option<i64>,
}

/// Mode for video or audio download.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DownloadMode {
    /// Automatically determine mode based on output file extension.
    /// Recognized audio extensions (.mp3, .m4a, .aac, .opus, .ogg, .flac, .wav, .alac, .wma)
    /// activate audio-only download; all other extensions use video+audio download.
    #[default]
    Auto,
    /// Download both video and audio streams and combine them.
    VideoAudio,
    /// Download only the audio stream (and transcode if the target format requires it).
    AudioOnly,
}

/// Tests whether a file extension corresponds to an audio-only format.
pub fn is_audio_extension(ext: &str) -> bool {
    matches!(
        ext.trim_start_matches('.').to_ascii_lowercase().as_str(),
        "mp3" | "m4a" | "aac" | "opus" | "ogg" | "flac" | "wav" | "alac" | "wma"
    )
}

/// Builder for configuring and executing video downloads.
///
/// Provides a fluent API for downloading videos with custom quality,
/// codec preferences, and progress tracking.
pub struct DownloadBuilder<'a> {
    pub(super) downloader: &'a Downloader,
    pub(super) video: &'a Video,
    pub(super) output: PathBuf,
    pub(super) video_quality: Option<VideoQuality>,
    pub(super) audio_quality: Option<AudioQuality>,
    pub(super) video_codec: Option<VideoCodecPreference>,
    pub(super) audio_codec: Option<AudioCodecPreference>,
    pub(super) storyboard_quality: Option<StoryboardQuality>,
    pub(super) thumbnail_quality: Option<ThumbnailQuality>,
    pub(super) priority: DownloadPriority,
    pub(super) progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
    pub(super) partial_range: Option<PartialRange>,
    pub(super) mode: DownloadMode,
    pub(super) embed_thumbnail: bool,
}

impl<'a> DownloadBuilder<'a> {
    /// Creates a new download builder.
    ///
    /// # Arguments
    ///
    /// * `downloader` - Reference to the Downloader client
    /// * `video` - The video to download
    /// * `output` - Output path for the downloaded file
    pub fn new(downloader: &'a Downloader, video: &'a Video, output: impl Into<PathBuf>) -> Self {
        let output = output.into();

        tracing::debug!(
            video_id = %video.id,
            output = ?output,
            "📥 Creating new DownloadBuilder"
        );

        Self {
            downloader,
            video,
            output,
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            storyboard_quality: None,
            thumbnail_quality: None,
            priority: DownloadPriority::Normal,
            progress_callback: None,
            partial_range: None,
            mode: DownloadMode::Auto,
            embed_thumbnail: false,
        }
    }

    /// Sets the desired video quality.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired video quality level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn video_quality(mut self, quality: VideoQuality) -> Self {
        self.video_quality = Some(quality);
        self
    }

    /// Sets the desired audio quality.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired audio quality level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn audio_quality(mut self, quality: AudioQuality) -> Self {
        self.audio_quality = Some(quality);
        self
    }

    /// Sets the preferred video codec.
    ///
    /// # Arguments
    ///
    /// * `codec` - The preferred video codec
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn video_codec(mut self, codec: VideoCodecPreference) -> Self {
        self.video_codec = Some(codec);
        self
    }

    /// Sets the preferred audio codec.
    ///
    /// # Arguments
    ///
    /// * `codec` - The preferred audio codec
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn audio_codec(mut self, codec: AudioCodecPreference) -> Self {
        self.audio_codec = Some(codec);
        self
    }

    /// Sets the desired storyboard quality.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired storyboard quality level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn storyboard_quality(mut self, quality: StoryboardQuality) -> Self {
        self.storyboard_quality = Some(quality);
        self
    }

    /// Sets the desired thumbnail quality.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired thumbnail quality level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn thumbnail_quality(mut self, quality: ThumbnailQuality) -> Self {
        self.thumbnail_quality = Some(quality);
        self
    }

    /// Enables or disables embedding the video thumbnail into the downloaded file.
    ///
    /// If enabled, the thumbnail is downloaded into a temporary file and embedded into
    /// the output file's metadata (e.g. ID3 APIC for MP3, cover art for M4A/MP4,
    /// Vorbis comment for FLAC/OGG, or attachment for WebM/MKV).
    ///
    /// # Arguments
    ///
    /// * `embed` - Whether to embed the thumbnail
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn embed_thumbnail(mut self, embed: bool) -> Self {
        self.embed_thumbnail = embed;
        self
    }

    /// Sets the desired thumbnail quality and enables embedding it into the downloaded file.
    ///
    /// # Arguments
    ///
    /// * `quality` - The desired thumbnail quality level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn embed_thumbnail_quality(mut self, quality: ThumbnailQuality) -> Self {
        self.embed_thumbnail = true;
        self.thumbnail_quality = Some(quality);
        self
    }

    /// Returns whether thumbnail embedding is enabled.
    pub fn is_embed_thumbnail_enabled(&self) -> bool {
        self.embed_thumbnail
    }

    /// Returns the configured thumbnail quality preference, if any.
    pub fn resolved_thumbnail_quality(&self) -> Option<ThumbnailQuality> {
        self.thumbnail_quality
    }

    /// Sets the download priority.
    ///
    /// # Arguments
    ///
    /// * `priority` - The download priority level
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn priority(mut self, priority: DownloadPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets a progress callback function.
    ///
    /// The callback receives a value between 0.0 and 1.0 representing download progress.
    pub fn with_progress<F>(mut self, callback: F) -> Self
    where
        F: Fn(f64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Forces audio-only download mode.
    ///
    /// In audio-only mode, only the audio stream is downloaded (no video stream will be fetched).
    /// If the target file format requires transcoding (e.g. `.mp3`), FFmpeg is automatically
    /// invoked to encode into the target container.
    pub fn audio_only(mut self) -> Self {
        self.mode = DownloadMode::AudioOnly;
        self
    }

    /// Sets the download mode explicitly.
    pub fn mode(mut self, mode: DownloadMode) -> Self {
        self.mode = mode;
        self
    }

    /// Resolves the effective download mode.
    ///
    /// If set to [`DownloadMode::Auto`], checks the file extension of the output path:
    /// recognized audio extensions (.mp3, .m4a, .aac, .opus, .ogg, .flac, .wav, .alac, .wma)
    /// resolve to [`DownloadMode::AudioOnly`], while all other extensions resolve to
    /// [`DownloadMode::VideoAudio`].
    pub fn resolved_mode(&self) -> DownloadMode {
        match self.mode {
            DownloadMode::Auto => {
                let ext = self.output.extension().and_then(|e| e.to_str()).unwrap_or("");
                if is_audio_extension(ext) {
                    DownloadMode::AudioOnly
                } else {
                    DownloadMode::VideoAudio
                }
            }
            mode => mode,
        }
    }

    /// Sets a partial range for downloading only a portion of the video.
    ///
    /// # Arguments
    ///
    /// * `range` - The partial range to download (time range or chapter range)
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn partial(mut self, range: PartialRange) -> Self {
        self.partial_range = Some(range);
        self
    }

    /// Helper method to set a time range for partial download.
    ///
    /// # Arguments
    ///
    /// * `start` - Start time in seconds (must be non-negative)
    /// * `end` - End time in seconds (must be greater than `start`)
    ///
    /// # Errors
    ///
    /// Returns an error if the time range is invalid.
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn time_range(self, start: f64, end: f64) -> Result<Self> {
        Ok(self.partial(PartialRange::time_range(start, end)?))
    }

    /// Helper method to download a single chapter.
    ///
    /// # Arguments
    ///
    /// * `index` - Chapter index (0-based)
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn chapter(self, index: usize) -> Self {
        self.partial(PartialRange::single_chapter(index))
    }

    /// Helper method to download a range of chapters.
    ///
    /// # Arguments
    ///
    /// * `start` - First chapter index (0-based)
    /// * `end` - Last chapter index (0-based, inclusive, must be >= `start`)
    ///
    /// # Errors
    ///
    /// Returns an error if `start > end`.
    ///
    /// # Returns
    ///
    /// The modified `DownloadBuilder` instance.
    pub fn chapters(self, start: usize, end: usize) -> Result<Self> {
        Ok(self.partial(PartialRange::chapter_range(start, end)?))
    }

    /// Executes the download with the configured options.
    ///
    /// Depending on [`DownloadBuilder::resolved_mode`], this either downloads both video and
    /// audio streams and combines them, or downloads only the audio stream (with automatic
    /// transcoding if required, such as to MP3).
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails, format is unavailable, or FFmpeg fails.
    ///
    /// # Returns
    ///
    /// Returns the path to the downloaded file.
    pub async fn execute(self) -> Result<PathBuf> {
        match self.resolved_mode() {
            DownloadMode::AudioOnly => self.execute_audio_only().await,
            DownloadMode::VideoAudio | DownloadMode::Auto => self.execute_video_and_audio().await,
        }
    }

    /// Executes audio-only download and optional transcoding.
    async fn execute_audio_only(mut self) -> Result<PathBuf> {
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let audio_codec = self.audio_codec.clone().unwrap_or(AudioCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            audio_quality = ?audio_quality,
            audio_codec = ?audio_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "📥 Executing audio-only download"
        );

        // Select audio format based on quality and codec preferences
        let audio_format = self
            .video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Audio))?;

        let audio_url = audio_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &audio_format.format_id))?;

        // Respect available_at restriction if present (e.g. YouTube pre-roll ad restrictions)
        let max_available_at = [audio_format.available_at, self.video.available_at]
            .into_iter()
            .flatten()
            .max();

        if let Some(available_at) = max_available_at {
            let now = crate::utils::current_timestamp();
            if available_at > now {
                let wait_secs = (available_at - now) as u64;
                tracing::info!(
                    wait_secs = wait_secs,
                    video_id = %self.video.id,
                    available_at = available_at,
                    "⏳ Waiting before audio download as required by the site"
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
            }
        }

        let audio_ext = audio_format.download_info.ext.as_str();
        let audio_guard = crate::utils::fs::create_temp_file("temp_audio", audio_ext);
        let audio_path = audio_guard.path().to_path_buf();

        let final_output = if self.output.is_absolute() {
            self.output.clone()
        } else {
            self.downloader.output_dir.join(&self.output)
        };
        crate::utils::create_parent_dir(&final_output).await?;

        // Track progress if callback is registered
        let cb_arc: Option<Arc<dyn Fn(f64) + Send + Sync>> = self.progress_callback.take().map(Arc::from);
        let progress_cb = cb_arc.as_ref().map(|cb| {
            let cb = Arc::clone(cb);
            move |downloaded: u64, total: u64| {
                if total > 0 {
                    let progress = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                    cb(progress);
                }
            }
        });

        let audio_id = self
            .downloader
            .download_manager
            .enqueue_with_all_options(
                audio_url,
                &audio_path,
                Some(self.priority),
                progress_cb,
                Some(audio_format.download_info.http_headers.clone()),
                audio_format.available_at,
            )
            .await;

        let status = self.downloader.wait_for_download(audio_id).await;
        match status {
            Some(DownloadStatus::Completed) => {}
            Some(DownloadStatus::Failed { reason }) => {
                return Err(crate::error::Error::download_failed(
                    audio_id,
                    format!("Audio download failed: {}", reason),
                ));
            }
            Some(DownloadStatus::Canceled) => {
                return Err(crate::error::Error::DownloadCancelled { download_id: audio_id });
            }
            _ => {
                return Err(crate::error::Error::download_failed(
                    audio_id,
                    "Unexpected download status",
                ));
            }
        }

        // Determine destination transcoding requirements
        let target_ext = final_output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let source_ext = audio_ext.to_lowercase();

        let audio_str = crate::utils::try_path_str(&audio_path)?;
        let output_str = crate::utils::try_path_str(&final_output)?;

        let source_is_aac = audio_format
            .codec_info
            .audio_codec
            .as_deref()
            .map(|c| c.contains("aac") || c.contains("mp4a"))
            .unwrap_or(source_ext == "m4a");

        let source_is_opus = audio_format
            .codec_info
            .audio_codec
            .as_deref()
            .map(|c| c.contains("opus"))
            .unwrap_or(source_ext == "webm" || source_ext == "opus");

        match target_ext.as_str() {
            "mp3" => {
                // Transcode to MP3 using libmp3lame without video mapping
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "libmp3lame", "-b:a", "192k"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "m4a" | "aac" if source_is_aac => {
                // Stream copy into M4A
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "copy"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "m4a" | "aac" => {
                // Transcode into AAC
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "aac", "-b:a", "192k"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "opus" | "ogg" if source_is_opus => {
                // Stream copy into Opus container
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "copy"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "opus" | "ogg" => {
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "libopus", "-b:a", "160k"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "flac" => {
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "flac"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            "wav" => {
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "pcm_s16le"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                executor.execute().await?;
            }
            _ if target_ext == source_ext => {
                // Direct file copy / rename
                if tokio::fs::rename(&audio_path, &final_output).await.is_err() {
                    tokio::fs::copy(&audio_path, &final_output).await?;
                }
            }
            _ => {
                // Default fallback: copy stream if compatible, or let ffmpeg transcode
                let args = crate::executor::FfmpegArgs::new()
                    .input(audio_str)
                    .args(["-vn", "-c:a", "copy"])
                    .overwrite()
                    .output(output_str)
                    .build();
                let executor = crate::executor::Executor::new(
                    self.downloader.libraries.ffmpeg.clone(),
                    args,
                    self.downloader.timeout,
                );
                if executor.execute().await.is_err() {
                    let fallback_args = crate::executor::FfmpegArgs::new()
                        .input(audio_str)
                        .args(["-vn"])
                        .overwrite()
                        .output(output_str)
                        .build();
                    let fallback_executor = crate::executor::Executor::new(
                        self.downloader.libraries.ffmpeg.clone(),
                        fallback_args,
                        self.downloader.timeout,
                    );
                    fallback_executor.execute().await?;
                }
            }
        }

        // Apply exact trim when a partial range was requested
        if let Some(ref range) = self.partial_range {
            let time_range = if range.needs_chapter_metadata() {
                range
                    .to_time_range(&self.video.chapters)
                    .ok_or_else(|| crate::error::Error::invalid_partial_range("chapter index out of bounds"))?
            } else {
                range.clone()
            };
            let (start_secs, end_secs) = time_range.get_times().ok_or_else(|| {
                crate::error::Error::invalid_partial_range("could not resolve time boundaries for trim")
            })?;

            let trimmed_guard = crate::utils::fs::create_temp_file(
                "trimmed",
                final_output.extension().and_then(|e| e.to_str()).unwrap_or("mp3"),
            );
            let trimmed_path = trimmed_guard.path().to_path_buf();

            self.downloader
                .extract_time_range(&final_output, &trimmed_path, start_secs, end_secs)
                .await?;

            tokio::fs::rename(&trimmed_path, &final_output)
                .await
                .map_err(|e| crate::error::Error::io_with_path("renaming trimmed output", &final_output, e))?;
        }

        // Add metadata to the audio file
        let metadata_manager = crate::metadata::MetadataManager::with_ffmpeg_path(&self.downloader.libraries.ffmpeg);
        let _ = metadata_manager.add_metadata(&final_output, self.video).await;

        self.maybe_embed_thumbnail(&final_output).await;

        if let Some(ref cb) = cb_arc {
            cb(1.0);
        }

        Ok(final_output)
    }

    /// Executes both video and audio downloads and combines them.
    async fn execute_video_and_audio(mut self) -> Result<PathBuf> {
        // Use configured quality/codec or defaults
        let video_quality = self.video_quality.unwrap_or(VideoQuality::Best);
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let video_codec = self.video_codec.clone().unwrap_or(VideoCodecPreference::Any);
        let audio_codec = self.audio_codec.clone().unwrap_or(AudioCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            video_quality = ?video_quality,
            audio_quality = ?audio_quality,
            video_codec = ?video_codec,
            audio_codec = ?audio_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "📥 Executing video and audio download"
        );

        // Select video format based on quality and codec preferences
        let video_format = self
            .video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Video))?;

        // Select audio format based on quality and codec preferences
        let audio_format = self
            .video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Audio))?;

        tracing::debug!(
            video_format_id = %video_format.format_id,
            audio_format_id = %audio_format.format_id,
            video_ext = ?video_format.download_info.ext,
            audio_ext = ?audio_format.download_info.ext,
            "📥 Selected video and audio formats"
        );

        // Generate temporary files using RAII guards in system temp dir
        let video_ext = video_format.download_info.ext.as_str();
        let video_guard = crate::utils::fs::create_temp_file("temp_video", video_ext);
        let video_path = video_guard.path().to_path_buf();

        let audio_ext = audio_format.download_info.ext.as_str();
        let audio_guard = crate::utils::fs::create_temp_file("temp_audio", audio_ext);
        let audio_path = audio_guard.path().to_path_buf();

        // Get download URLs
        let video_url = video_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &video_format.format_id))?;

        let audio_url = audio_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &audio_format.format_id))?;

        // Attempt media-seek partial download before falling back to a full fetch.
        let streams = PartialClipStreams {
            video_url,
            audio_url,
            video_format,
            audio_format,
        };
        if let Some(range) = self.partial_range.as_ref()
            && let Some(path) = try_partial_clip(self.downloader, self.video, &streams, range, &self.output).await?
        {
            return Ok(path);
        }

        // Respect available_at restriction if present (e.g. YouTube pre-roll ad restrictions)
        let max_available_at = [
            video_format.available_at,
            audio_format.available_at,
            self.video.available_at,
        ]
        .into_iter()
        .flatten()
        .max();

        if let Some(available_at) = max_available_at {
            let now = crate::utils::current_timestamp();
            if available_at > now {
                let wait_secs = (available_at - now) as u64;
                tracing::info!(
                    wait_secs = wait_secs,
                    video_id = %self.video.id,
                    available_at = available_at,
                    "⏳ Waiting before download as required by the site (available_at restriction)"
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
            }
        }

        // Create atomic dual progress tracker when callback is set
        let tracker = self
            .progress_callback
            .take()
            .map(|cb| Arc::new(DualProgressTracker::new(cb)));

        // Enqueue downloads with configured priority
        let enqueue_streams = EnqueueStreams {
            video_url: streams.video_url,
            audio_url: streams.audio_url,
            video_path: video_path.clone(),
            audio_path: audio_path.clone(),
            video_headers: streams.video_format.download_info.http_headers.clone(),
            audio_headers: streams.audio_format.download_info.http_headers.clone(),
            video_available_at: video_format.available_at,
            audio_available_at: audio_format.available_at,
        };
        let enqueued = enqueue_both_downloads(self.downloader, enqueue_streams, self.priority, tracker.clone()).await;
        let video_download_id = enqueued.video_id;
        let audio_download_id = enqueued.audio_id;

        // Wait for both downloads to complete
        tracing::debug!(
            video_download_id = video_download_id,
            audio_download_id = audio_download_id,
            "📥 Waiting for downloads to complete"
        );

        let (video_status, audio_status) = tokio::join!(
            self.downloader.wait_for_download(video_download_id),
            self.downloader.wait_for_download(audio_download_id),
        );

        // Check if downloads were successful
        match (video_status, audio_status) {
            (Some(DownloadStatus::Completed), Some(DownloadStatus::Completed)) => {
                tracing::debug!(
                    output = ?self.output,
                    "✅ Both downloads completed, combining audio and video"
                );

                if let Some(ref tr) = tracker {
                    tr.finish();
                }

                // Both downloads completed successfully, combine them
                let output_path = if self.output.is_absolute() {
                    self.output.clone()
                } else {
                    self.downloader.output_dir.join(&self.output)
                };
                crate::utils::create_parent_dir(&output_path).await?;

                let combined_path = self
                    .downloader
                    .combine_audio_and_video_to_path(&audio_path, &video_path, &output_path)
                    .await?;

                // Apply exact trim when a partial range was requested
                if let Some(ref range) = self.partial_range {
                    let time_range = if range.needs_chapter_metadata() {
                        range
                            .to_time_range(&self.video.chapters)
                            .ok_or_else(|| crate::error::Error::invalid_partial_range("chapter index out of bounds"))?
                    } else {
                        range.clone()
                    };
                    let (start_secs, end_secs) = time_range.get_times().ok_or_else(|| {
                        crate::error::Error::invalid_partial_range("could not resolve time boundaries for trim")
                    })?;

                    let trimmed_guard = crate::utils::fs::create_temp_file(
                        "trimmed",
                        combined_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4"),
                    );
                    let trimmed_path = trimmed_guard.path().to_path_buf();

                    self.downloader
                        .extract_time_range(&combined_path, &trimmed_path, start_secs, end_secs)
                        .await?;

                    tokio::fs::rename(&trimmed_path, &combined_path)
                        .await
                        .map_err(|e| crate::error::Error::io_with_path("renaming trimmed output", &combined_path, e))?;
                }

                self.maybe_embed_thumbnail(&combined_path).await;

                Ok(combined_path)
            }
            (Some(DownloadStatus::Failed { reason }), _) => Err(crate::error::Error::download_failed(
                video_download_id,
                format!("Video download failed: {}", reason),
            )),
            (_, Some(DownloadStatus::Failed { reason })) => Err(crate::error::Error::download_failed(
                audio_download_id,
                format!("Audio download failed: {}", reason),
            )),
            (Some(DownloadStatus::Canceled), _) => Err(crate::error::Error::DownloadCancelled {
                download_id: video_download_id,
            }),
            (_, Some(DownloadStatus::Canceled)) => Err(crate::error::Error::DownloadCancelled {
                download_id: audio_download_id,
            }),
            _ => Err(crate::error::Error::download_failed(
                video_download_id,
                "Unexpected download status",
            )),
        }
    }

    /// Embeds the thumbnail into the output file if `embed_thumbnail` is enabled.
    ///
    /// Uses best effort: if thumbnail downloading or embedding fails, emits a warning
    /// log without failing the main download.
    async fn maybe_embed_thumbnail(&self, output_path: &Path) {
        if !self.embed_thumbnail {
            return;
        }

        let quality = self.thumbnail_quality.unwrap_or(ThumbnailQuality::Best);
        let Some(thumbnail) = self.video.select_thumbnail(quality) else {
            tracing::warn!(
                video_id = %self.video.id,
                "No thumbnail found to embed into output file"
            );
            return;
        };

        let thumb_ext = thumbnail
            .url
            .split('?')
            .next()
            .and_then(|u| u.rsplit('.').next())
            .unwrap_or("jpg");

        let thumb_guard = crate::utils::fs::create_temp_file("thumb_embed", thumb_ext);
        let thumb_path = thumb_guard.path().to_path_buf();

        match self
            .downloader
            .download_thumbnail(self.video, quality, &thumb_path)
            .await
        {
            Ok(_) => {
                let metadata_manager =
                    crate::metadata::MetadataManager::with_ffmpeg_path(&self.downloader.libraries.ffmpeg);
                if let Err(e) = metadata_manager.add_thumbnail_to_file(output_path, &thumb_path).await {
                    tracing::warn!(
                        output = ?output_path,
                        error = %e,
                        "Failed to embed thumbnail into output file"
                    );
                } else {
                    tracing::debug!(
                        output = ?output_path,
                        "✅ Successfully embedded thumbnail into output file"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    output = ?output_path,
                    error = %e,
                    "Failed to download thumbnail for embedding"
                );
            }
        }
    }
}

/// Attempts a media-seek partial clip for both streams and combines the result.
///
/// Performs a binary-search-based byte-range clip for the given time window on both
/// the video and audio streams, then combines them with FFmpeg and applies a
/// precision trim. Returns `Ok(Some(path))` on success, `Ok(None)` when the format
/// does not support partial clipping (falls through to full download), or `Err` on
/// a real I/O or network error.
async fn try_partial_clip(
    downloader: &Downloader,
    video: &Video,
    streams: &PartialClipStreams<'_>,
    range: &PartialRange,
    output: &Path,
) -> Result<Option<PathBuf>> {
    let time_range = if range.needs_chapter_metadata() {
        range
            .to_time_range(&video.chapters)
            .ok_or_else(|| crate::error::Error::invalid_partial_range("chapter index out of bounds"))?
    } else {
        range.clone()
    };

    let Some((start_secs, end_secs)) = time_range.get_times() else {
        return Ok(None);
    };

    let video_total_size = streams
        .video_format
        .file_info
        .filesize
        .or(streams.video_format.file_info.filesize_approx)
        .filter(|&n| n > 0)
        .map(|n| n as u64);
    let audio_total_size = streams
        .audio_format
        .file_info
        .filesize
        .or(streams.audio_format.file_info.filesize_approx)
        .filter(|&n| n > 0)
        .map(|n| n as u64);

    let video_clip_guard =
        crate::utils::fs::create_temp_file("clip_video", streams.video_format.download_info.ext.as_str());
    let video_clip_path = video_clip_guard.path().to_path_buf();

    let audio_clip_guard =
        crate::utils::fs::create_temp_file("clip_audio", streams.audio_format.download_info.ext.as_str());
    let audio_clip_path = audio_clip_guard.path().to_path_buf();

    let video_result = clip_stream(
        downloader,
        streams.video_url,
        &streams.video_format.download_info.http_headers,
        video_total_size,
        start_secs,
        end_secs,
        &video_clip_path,
    )
    .await;

    match video_result {
        Ok(()) => {}
        Err(media_seek::Error::UnsupportedFormat | media_seek::Error::ParseFailed { .. }) => {
            tracing::warn!("media-seek video clip unavailable for this format, falling back to full download");
            return Ok(None);
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    let audio_result = clip_stream(
        downloader,
        streams.audio_url,
        &streams.audio_format.download_info.http_headers,
        audio_total_size,
        start_secs,
        end_secs,
        &audio_clip_path,
    )
    .await;

    match audio_result {
        Ok(()) => {}
        Err(media_seek::Error::UnsupportedFormat | media_seek::Error::ParseFailed { .. }) => {
            tracing::warn!("media-seek audio clip unavailable for this format, falling back to full download");
            return Ok(None);
        }
        Err(e) => {
            return Err(e.into());
        }
    }

    tracing::info!(
        start_secs,
        end_secs,
        "✅ media-seek partial download succeeded, combining streams"
    );

    let output_path = if output.is_absolute() {
        output.to_path_buf()
    } else {
        downloader.output_dir.join(output)
    };
    crate::utils::create_parent_dir(&output_path).await?;

    let combined_path = downloader
        .combine_audio_and_video_to_path(&audio_clip_path, &video_clip_path, &output_path)
        .await?;

    // Precision trim: media-seek returns keyframe-aligned boundaries;
    // FFmpeg -c copy sharpens to the exact requested timestamps.
    let trimmed_guard = crate::utils::fs::create_temp_file(
        "trimmed",
        combined_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4"),
    );
    let trimmed_path = trimmed_guard.path().to_path_buf();

    downloader
        .extract_time_range(&combined_path, &trimmed_path, start_secs, end_secs)
        .await?;

    tokio::fs::rename(&trimmed_path, &combined_path)
        .await
        .map_err(|e| crate::error::Error::io_with_path("renaming trimmed output", &combined_path, e))?;

    Ok(Some(combined_path))
}

use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe tracker that aggregates concurrent video and audio download streams
/// into a single monotonic progress value from 0.0 to 1.0 based on real transferred bytes.
#[doc(hidden)]
pub struct DualProgressTracker {
    video_downloaded: AtomicU64,
    video_total: AtomicU64,
    audio_downloaded: AtomicU64,
    audio_total: AtomicU64,
    last_emitted: std::sync::Mutex<f64>,
    callback: Box<dyn Fn(f64) + Send + Sync>,
}

impl DualProgressTracker {
    pub fn new(callback: Box<dyn Fn(f64) + Send + Sync>) -> Self {
        Self {
            video_downloaded: AtomicU64::new(0),
            video_total: AtomicU64::new(0),
            audio_downloaded: AtomicU64::new(0),
            audio_total: AtomicU64::new(0),
            last_emitted: std::sync::Mutex::new(0.0),
            callback,
        }
    }

    pub fn on_video_progress(&self, downloaded: u64, total: u64) {
        self.video_downloaded.store(downloaded, Ordering::Release);
        if total > 0 {
            self.video_total.store(total, Ordering::Release);
        }
        self.emit();
    }

    pub fn on_audio_progress(&self, downloaded: u64, total: u64) {
        self.audio_downloaded.store(downloaded, Ordering::Release);
        if total > 0 {
            self.audio_total.store(total, Ordering::Release);
        }
        self.emit();
    }

    fn emit(&self) {
        let v_dl = self.video_downloaded.load(Ordering::Acquire);
        let a_dl = self.audio_downloaded.load(Ordering::Acquire);
        let v_tot = self.video_total.load(Ordering::Acquire);
        let a_tot = self.audio_total.load(Ordering::Acquire);

        let total = v_tot + a_tot;
        if total > 0 {
            let progress = (v_dl + a_dl) as f64 / total as f64;
            let clamped = progress.clamp(0.0, 1.0);
            if let Ok(mut last) = self.last_emitted.lock()
                && clamped >= *last
            {
                *last = clamped;
                (self.callback)(clamped);
            }
        }
    }

    pub fn finish(&self) {
        if let Ok(mut last) = self.last_emitted.lock() {
            *last = 1.0;
            (self.callback)(1.0);
        }
    }
}

/// Enqueues video and audio downloads and returns their assigned IDs.
///
/// When a `progress_tracker` is provided, both streams feed into the tracker to produce
/// a single monotonic progress report from 0.0 to 1.0 based on real transferred bytes.
async fn enqueue_both_downloads(
    downloader: &Downloader,
    streams: EnqueueStreams<'_>,
    priority: DownloadPriority,
    progress_tracker: Option<Arc<DualProgressTracker>>,
) -> EnqueuedDownloads {
    if let Some(tracker) = progress_tracker {
        let video_tracker = Arc::clone(&tracker);
        let video_callback = move |downloaded: u64, total: u64| {
            video_tracker.on_video_progress(downloaded, total);
        };

        let audio_tracker = Arc::clone(&tracker);
        let audio_callback = move |downloaded: u64, total: u64| {
            audio_tracker.on_audio_progress(downloaded, total);
        };

        let video_id = downloader
            .download_manager
            .enqueue_with_all_options(
                streams.video_url,
                streams.video_path,
                Some(priority),
                Some(video_callback),
                Some(streams.video_headers),
                streams.video_available_at,
            )
            .await;

        let audio_id = downloader
            .download_manager
            .enqueue_with_all_options(
                streams.audio_url,
                streams.audio_path,
                Some(priority),
                Some(audio_callback),
                Some(streams.audio_headers),
                streams.audio_available_at,
            )
            .await;

        EnqueuedDownloads { video_id, audio_id }
    } else {
        let video_id = downloader
            .download_manager
            .enqueue_with_all_options::<fn(u64, u64)>(
                streams.video_url,
                streams.video_path,
                Some(priority),
                None,
                Some(streams.video_headers),
                streams.video_available_at,
            )
            .await;

        let audio_id = downloader
            .download_manager
            .enqueue_with_all_options::<fn(u64, u64)>(
                streams.audio_url,
                streams.audio_path,
                Some(priority),
                None,
                Some(streams.audio_headers),
                streams.audio_available_at,
            )
            .await;

        EnqueuedDownloads { video_id, audio_id }
    }
}
