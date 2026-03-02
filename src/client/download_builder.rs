//! Download builder for fluent download API.
//!
//! This module provides a builder pattern for configuring and executing downloads.

use std::path::PathBuf;
use std::sync::Arc;

use crate::client::Downloader;
use crate::client::stream_downloads::clip_stream;
use crate::client::streams::selection::VideoSelection;
use crate::download::engine::partial::PartialRange;
use crate::download::{DownloadPriority, DownloadStatus};
use crate::error::Result;
use crate::model::Video;
use crate::model::format::FormatType;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, StoryboardQuality, ThumbnailQuality, VideoCodecPreference, VideoQuality,
};

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
    /// This method uses the download manager to handle the download with the configured
    /// priority and progress callback.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or the video cannot be fetched.
    ///
    /// # Returns
    ///
    /// Returns the path to the downloaded file.
    pub async fn execute(self) -> Result<PathBuf> {
        // Use configured quality/codec or defaults
        let video_quality = self.video_quality.unwrap_or(VideoQuality::Best);
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let video_codec = self.video_codec.unwrap_or(VideoCodecPreference::Any);
        let audio_codec = self.audio_codec.unwrap_or(AudioCodecPreference::Any);

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
            "📥 Executing download"
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

        // Generate temporary filenames for video and audio
        let video_ext = video_format.download_info.ext.as_str();
        let video_filename = format!("temp_video_{}.{}", crate::utils::fs::random_filename(8), video_ext);

        let audio_ext = audio_format.download_info.ext.as_str();
        let audio_filename = format!("temp_audio_{}.{}", crate::utils::fs::random_filename(8), audio_ext);

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
        if let Some(range) = self.partial_range.as_ref() {
            let time_range = if range.needs_chapter_metadata() {
                range
                    .to_time_range(&self.video.chapters)
                    .ok_or_else(|| crate::error::Error::invalid_partial_range("chapter index out of bounds"))?
            } else {
                range.clone()
            };

            if let Some((start_secs, end_secs)) = time_range.get_times() {
                let video_total_size = video_format
                    .file_info
                    .filesize
                    .or(video_format.file_info.filesize_approx)
                    .filter(|&n| n > 0)
                    .map(|n| n as u64);
                let audio_total_size = audio_format
                    .file_info
                    .filesize
                    .or(audio_format.file_info.filesize_approx)
                    .filter(|&n| n > 0)
                    .map(|n| n as u64);

                let video_clip_filename = format!(
                    "clip_video_{}.{}",
                    crate::utils::fs::random_filename(8),
                    video_format.download_info.ext.as_str()
                );
                let video_clip_path = self.downloader.output_dir.join(&video_clip_filename);

                let audio_clip_filename = format!(
                    "clip_audio_{}.{}",
                    crate::utils::fs::random_filename(8),
                    audio_format.download_info.ext.as_str()
                );
                let audio_clip_path = self.downloader.output_dir.join(&audio_clip_filename);

                let video_result = clip_stream(
                    self.downloader,
                    video_url,
                    &video_format.download_info.http_headers,
                    video_total_size,
                    start_secs,
                    end_secs,
                    &video_clip_path,
                )
                .await;

                match video_result {
                    Ok(()) => {
                        let audio_result = clip_stream(
                            self.downloader,
                            audio_url,
                            &audio_format.download_info.http_headers,
                            audio_total_size,
                            start_secs,
                            end_secs,
                            &audio_clip_path,
                        )
                        .await;

                        match audio_result {
                            Ok(()) => {
                                tracing::info!(
                                    start_secs,
                                    end_secs,
                                    "✅ media-seek partial download succeeded, combining streams"
                                );

                                let output_path = if self.output.is_absolute() {
                                    self.output.clone()
                                } else {
                                    self.downloader.output_dir.join(&self.output)
                                };

                                let combined_path = self
                                    .downloader
                                    .combine_audio_and_video_to_path(&audio_clip_path, &video_clip_path, &output_path)
                                    .await?;

                                // Precision trim: media-seek returns keyframe-aligned boundaries;
                                // FFmpeg -c copy sharpens to the exact requested timestamps.
                                let trimmed_name = format!(
                                    "trimmed_{}.{}",
                                    crate::utils::fs::random_filename(8),
                                    combined_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4")
                                );
                                let trimmed_path = self.downloader.output_dir.join(&trimmed_name);

                                self.downloader
                                    .extract_time_range(&combined_path, &trimmed_path, start_secs, end_secs)
                                    .await?;

                                tokio::fs::rename(&trimmed_path, &combined_path).await.map_err(|e| {
                                    crate::error::Error::io_with_path("renaming trimmed output", &combined_path, e)
                                })?;

                                return Ok(combined_path);
                            }
                            Err(media_seek::Error::UnsupportedFormat | media_seek::Error::ParseFailed { .. }) => {
                                tracing::warn!(
                                    "media-seek audio clip unavailable for this format, falling back to full download"
                                );
                                // Clean up temp files from partial clip attempt
                                let _ = tokio::fs::remove_file(&video_clip_path).await;
                                let _ = tokio::fs::remove_file(&audio_clip_path).await;
                            }
                            Err(e) => {
                                let _ = tokio::fs::remove_file(&video_clip_path).await;
                                let _ = tokio::fs::remove_file(&audio_clip_path).await;
                                return Err(e.into());
                            }
                        }
                    }
                    Err(media_seek::Error::UnsupportedFormat | media_seek::Error::ParseFailed { .. }) => {
                        tracing::warn!(
                            "media-seek video clip unavailable for this format, falling back to full download"
                        );
                        // Clean up temp files from partial clip attempt
                        let _ = tokio::fs::remove_file(&video_clip_path).await;
                        let _ = tokio::fs::remove_file(&audio_clip_path).await;
                    }
                    Err(e) => {
                        let _ = tokio::fs::remove_file(&video_clip_path).await;
                        let _ = tokio::fs::remove_file(&audio_clip_path).await;
                        return Err(e.into());
                    }
                }
            }
        }

        // Create output paths
        let video_path = self.downloader.output_dir.join(&video_filename);
        let audio_path = self.downloader.output_dir.join(&audio_filename);

        // Enqueue downloads with configured priority
        let (video_download_id, audio_download_id) = if let Some(callback) = self.progress_callback {
            // Wrap callback in Arc to share between downloads
            let callback = Arc::new(callback);

            // Clone Arc for video download (progress will be split 50/50 between video and audio)
            let video_callback = {
                let callback = Arc::clone(&callback);
                move |downloaded: u64, total: u64| {
                    if total > 0 {
                        let progress = (downloaded as f64 / total as f64) * 0.5;
                        callback(progress);
                    }
                }
            };

            // Clone Arc for audio download (second half of progress)
            let audio_callback = {
                let callback = Arc::clone(&callback);
                move |downloaded: u64, total: u64| {
                    if total > 0 {
                        let progress = 0.5 + (downloaded as f64 / total as f64) * 0.5;
                        callback(progress);
                    }
                }
            };

            let video_id = self
                .downloader
                .download_manager
                .enqueue_with_progress_and_headers(
                    video_url,
                    video_path.clone(),
                    Some(self.priority),
                    video_callback,
                    Some(video_format.download_info.http_headers.clone()),
                )
                .await;

            let audio_id = self
                .downloader
                .download_manager
                .enqueue_with_progress_and_headers(
                    audio_url,
                    audio_path.clone(),
                    Some(self.priority),
                    audio_callback,
                    Some(audio_format.download_info.http_headers.clone()),
                )
                .await;

            (video_id, audio_id)
        } else {
            let video_id = self
                .downloader
                .download_manager
                .enqueue_with_headers(
                    video_url,
                    video_path.clone(),
                    Some(self.priority),
                    Some(video_format.download_info.http_headers.clone()),
                )
                .await;

            let audio_id = self
                .downloader
                .download_manager
                .enqueue_with_headers(
                    audio_url,
                    audio_path.clone(),
                    Some(self.priority),
                    Some(audio_format.download_info.http_headers.clone()),
                )
                .await;

            (video_id, audio_id)
        };

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

                // Both downloads completed successfully, combine them
                let combined_path = if self.output.is_absolute() {
                    self.downloader
                        .combine_audio_and_video_to_path(&audio_path, &video_path, &self.output)
                        .await?
                } else {
                    let output_str = self
                        .output
                        .to_str()
                        .ok_or_else(|| crate::error::Error::PathValidation {
                            path: self.output.clone(),
                            reason: "output path contains invalid UTF-8".into(),
                        })?;
                    self.downloader
                        .combine_audio_and_video(&audio_filename, &video_filename, output_str)
                        .await?
                };

                // Apply exact trim when a partial range was requested
                if let Some(range) = self.partial_range {
                    let time_range = if range.needs_chapter_metadata() {
                        range
                            .to_time_range(&self.video.chapters)
                            .ok_or_else(|| crate::error::Error::invalid_partial_range("chapter index out of bounds"))?
                    } else {
                        range
                    };
                    let (start_secs, end_secs) = time_range.get_times().ok_or_else(|| {
                        crate::error::Error::invalid_partial_range("could not resolve time boundaries for trim")
                    })?;

                    let trimmed_name = format!(
                        "trimmed_{}.{}",
                        crate::utils::fs::random_filename(8),
                        combined_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4")
                    );
                    let trimmed_path = self.downloader.output_dir.join(&trimmed_name);

                    self.downloader
                        .extract_time_range(&combined_path, &trimmed_path, start_secs, end_secs)
                        .await?;

                    tokio::fs::rename(&trimmed_path, &combined_path)
                        .await
                        .map_err(|e| crate::error::Error::io_with_path("renaming trimmed output", &combined_path, e))?;
                }

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
}
