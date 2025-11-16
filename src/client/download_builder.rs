//! Download builder for fluent download API.
//!
//! This module provides a builder pattern for configuring and executing downloads.

use crate::client::Youtube;
use crate::download::DownloadPriority;
use crate::download::partial::PartialRange;
use crate::error::Result;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Builder for configuring and executing video downloads.
///
/// Provides a fluent API for downloading videos with custom quality,
/// codec preferences, and progress tracking.
pub struct DownloadBuilder<'a> {
    youtube: &'a Youtube,
    url: String,
    output: PathBuf,
    video_quality: Option<VideoQuality>,
    audio_quality: Option<AudioQuality>,
    video_codec: Option<VideoCodecPreference>,
    audio_codec: Option<AudioCodecPreference>,
    priority: DownloadPriority,
    progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
    partial_range: Option<PartialRange>,
}

impl<'a> DownloadBuilder<'a> {
    /// Creates a new download builder.
    ///
    /// # Arguments
    ///
    /// * `youtube` - Reference to the Youtube client
    /// * `url` - The video URL to download
    /// * `output` - Output path for the downloaded file
    pub fn new(youtube: &'a Youtube, url: impl Into<String>, output: impl Into<PathBuf>) -> Self {
        Self {
            youtube,
            url: url.into(),
            output: output.into(),
            video_quality: None,
            audio_quality: None,
            video_codec: None,
            audio_codec: None,
            priority: DownloadPriority::Normal,
            progress_callback: None,
            partial_range: None,
        }
    }

    /// Sets the desired video quality.
    pub fn video_quality(mut self, quality: VideoQuality) -> Self {
        self.video_quality = Some(quality);
        self
    }

    /// Sets the desired audio quality.
    pub fn audio_quality(mut self, quality: AudioQuality) -> Self {
        self.audio_quality = Some(quality);
        self
    }

    /// Sets the preferred video codec.
    pub fn video_codec(mut self, codec: VideoCodecPreference) -> Self {
        self.video_codec = Some(codec);
        self
    }

    /// Sets the preferred audio codec.
    pub fn audio_codec(mut self, codec: AudioCodecPreference) -> Self {
        self.audio_codec = Some(codec);
        self
    }

    /// Sets the download priority.
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
    pub fn partial(mut self, range: PartialRange) -> Self {
        self.partial_range = Some(range);
        self
    }

    /// Helper method to set a time range for partial download.
    ///
    /// # Arguments
    ///
    /// * `start` - Start time in seconds
    /// * `end` - End time in seconds
    pub fn time_range(self, start: f64, end: f64) -> Self {
        self.partial(PartialRange::time_range(start, end))
    }

    /// Helper method to download a single chapter.
    ///
    /// # Arguments
    ///
    /// * `index` - Chapter index (0-based)
    pub fn chapter(self, index: usize) -> Self {
        self.partial(PartialRange::single_chapter(index))
    }

    /// Helper method to download a range of chapters.
    ///
    /// # Arguments
    ///
    /// * `start` - First chapter index (0-based)
    /// * `end` - Last chapter index (0-based, inclusive)
    pub fn chapters(self, start: usize, end: usize) -> Self {
        self.partial(PartialRange::chapter_range(start, end))
    }

    /// Executes the download with the configured options.
    ///
    /// This method uses the download manager to handle the download with the configured
    /// priority and progress callback.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails or the video cannot be fetched.
    pub async fn execute(self) -> Result<PathBuf> {
        // Use configured quality/codec or defaults
        let video_quality = self.video_quality.unwrap_or(VideoQuality::Best);
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let video_codec = self.video_codec.unwrap_or(VideoCodecPreference::Any);
        let audio_codec = self.audio_codec.unwrap_or(AudioCodecPreference::Any);

        // Fetch video information
        let video = self.youtube.fetch_video_infos(self.url.clone()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| crate::error::Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "video".to_string(),
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| crate::error::Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "audio".to_string(),
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Generate temporary filenames for video and audio
        let video_ext = format!("{:?}", video_format.download_info.ext);
        let video_filename = format!(
            "temp_video_{}.{}",
            crate::utils::fs::random_filename(8),
            video_ext
        );

        let audio_ext = format!("{:?}", audio_format.download_info.ext);
        let audio_filename = format!(
            "temp_audio_{}.{}",
            crate::utils::fs::random_filename(8),
            audio_ext
        );

        // Get download URLs
        let video_url = video_format.download_info.url.as_ref().ok_or_else(|| {
            crate::error::Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: video_format.format_id.clone(),
            }
        })?;

        let audio_url = audio_format.download_info.url.as_ref().ok_or_else(|| {
            crate::error::Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: audio_format.format_id.clone(),
            }
        })?;

        // Create output paths
        let video_path = self.youtube.output_dir.join(&video_filename);
        let audio_path = self.youtube.output_dir.join(&audio_filename);

        // Enqueue downloads with configured priority
        let (video_download_id, audio_download_id) = if let Some(callback) = self.progress_callback
        {
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
                .youtube
                .download_manager
                .enqueue_with_progress(
                    video_url,
                    video_path.clone(),
                    Some(self.priority),
                    video_callback,
                )
                .await;

            let audio_id = self
                .youtube
                .download_manager
                .enqueue_with_progress(
                    audio_url,
                    audio_path.clone(),
                    Some(self.priority),
                    audio_callback,
                )
                .await;

            (video_id, audio_id)
        } else {
            let video_id = self
                .youtube
                .download_manager
                .enqueue(video_url, video_path.clone(), Some(self.priority))
                .await;

            let audio_id = self
                .youtube
                .download_manager
                .enqueue(audio_url, audio_path.clone(), Some(self.priority))
                .await;

            (video_id, audio_id)
        };

        // Wait for both downloads to complete
        let video_status = self.youtube.wait_for_download(video_download_id).await;
        let audio_status = self.youtube.wait_for_download(audio_download_id).await;

        // Check if downloads were successful
        use crate::download::DownloadStatus;
        match (video_status, audio_status) {
            (Some(DownloadStatus::Completed), Some(DownloadStatus::Completed)) => {
                // Both downloads completed successfully, combine them
                self.youtube
                    .combine_audio_and_video(
                        &audio_filename,
                        &video_filename,
                        self.output.to_str().unwrap(),
                    )
                    .await
            }
            (Some(DownloadStatus::Failed { reason }), _) => {
                Err(crate::error::Error::download_failed(
                    video_download_id,
                    format!("Video download failed: {}", reason),
                ))
            }
            (_, Some(DownloadStatus::Failed { reason })) => {
                Err(crate::error::Error::download_failed(
                    audio_download_id,
                    format!("Audio download failed: {}", reason),
                ))
            }
            (Some(DownloadStatus::Canceled), _) => Err(crate::error::Error::DownloadCancelled {
                download_id: video_download_id,
            }),
            (_, Some(DownloadStatus::Canceled)) => Err(crate::error::Error::DownloadCancelled {
                download_id: audio_download_id,
            }),
            _ => Err(crate::error::Error::Unknown(
                "Unexpected download status".to_string(),
            )),
        }
    }
}
