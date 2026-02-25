//! Download builder for fluent download API.
//!
//! This module provides a builder pattern for configuring and executing downloads.

use crate::client::Downloader;
use crate::client::streams::selection::VideoSelection;
use crate::download::DownloadPriority;
use crate::download::DownloadStatus;
use crate::download::partial::PartialRange;
use crate::error::Result;
use crate::model::Video;
use crate::model::format::FormatType;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, StoryboardQuality, ThumbnailQuality, VideoCodecPreference,
    VideoQuality,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Builder for configuring and executing video downloads.
///
/// Provides a fluent API for downloading videos with custom quality,
/// codec preferences, and progress tracking.
pub struct DownloadBuilder<'a> {
    downloader: &'a Downloader,
    video: &'a Video,
    output: PathBuf,
    video_quality: Option<VideoQuality>,
    audio_quality: Option<AudioQuality>,
    video_codec: Option<VideoCodecPreference>,
    audio_codec: Option<AudioCodecPreference>,
    storyboard_quality: Option<StoryboardQuality>,
    thumbnail_quality: Option<ThumbnailQuality>,
    priority: DownloadPriority,
    progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
    partial_range: Option<PartialRange>,
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
            "Creating new DownloadBuilder"
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
    pub fn video_quality(mut self, quality: VideoQuality) -> Self {
        tracing::debug!(quality = ?quality, "Setting video quality");

        self.video_quality = Some(quality);
        self
    }

    /// Sets the desired audio quality.
    pub fn audio_quality(mut self, quality: AudioQuality) -> Self {
        tracing::debug!(quality = ?quality, "Setting audio quality");

        self.audio_quality = Some(quality);
        self
    }

    /// Sets the preferred video codec.
    pub fn video_codec(mut self, codec: VideoCodecPreference) -> Self {
        tracing::debug!(codec = ?codec, "Setting video codec preference");

        self.video_codec = Some(codec);
        self
    }

    /// Sets the preferred audio codec.
    pub fn audio_codec(mut self, codec: AudioCodecPreference) -> Self {
        tracing::debug!(codec = ?codec, "Setting audio codec preference");

        self.audio_codec = Some(codec);
        self
    }

    /// Sets the desired storyboard quality.
    pub fn storyboard_quality(mut self, quality: StoryboardQuality) -> Self {
        tracing::debug!(quality = ?quality, "Setting storyboard quality");

        self.storyboard_quality = Some(quality);
        self
    }

    /// Sets the desired thumbnail quality.
    pub fn thumbnail_quality(mut self, quality: ThumbnailQuality) -> Self {
        tracing::debug!(quality = ?quality, "Setting thumbnail quality");

        self.thumbnail_quality = Some(quality);
        self
    }

    /// Sets the download priority.
    pub fn priority(mut self, priority: DownloadPriority) -> Self {
        tracing::debug!(priority = ?priority, "Setting download priority");

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
        tracing::debug!(range = ?range, "Setting partial download range");

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
    /// The modifed `DownloadBuilder` instance.
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
            "Executing download"
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
            "Selected video and audio formats"
        );

        // Generate temporary filenames for video and audio
        let video_ext = video_format.download_info.ext.as_str();
        let video_filename = format!(
            "temp_video_{}.{}",
            crate::utils::fs::random_filename(8),
            video_ext
        );

        let audio_ext = audio_format.download_info.ext.as_str();
        let audio_filename = format!(
            "temp_audio_{}.{}",
            crate::utils::fs::random_filename(8),
            audio_ext
        );

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

        // Create output paths
        let video_path = self.downloader.output_dir.join(&video_filename);
        let audio_path = self.downloader.output_dir.join(&audio_filename);

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
            "Waiting for downloads to complete"
        );

        let video_status = self.downloader.wait_for_download(video_download_id).await;
        let audio_status = self.downloader.wait_for_download(audio_download_id).await;

        // Check if downloads were successful
        match (video_status, audio_status) {
            (Some(DownloadStatus::Completed), Some(DownloadStatus::Completed)) => {
                tracing::debug!(
                    output = ?self.output,
                    "Both downloads completed, combining audio and video"
                );

                // Both downloads completed successfully, combine them
                if self.output.is_absolute() {
                    // Use the absolute path directly, bypassing output_dir
                    self.downloader
                        .combine_audio_and_video_to_path(&audio_path, &video_path, &self.output)
                        .await
                } else {
                    let output_str = self.output.to_str().ok_or_else(|| {
                        crate::error::Error::PathValidation {
                            path: self.output.clone(),
                            reason: "output path contains invalid UTF-8".into(),
                        }
                    })?;
                    self.downloader
                        .combine_audio_and_video(&audio_filename, &video_filename, output_str)
                        .await
                }
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
            _ => Err(crate::error::Error::download_failed(
                video_download_id,
                "Unexpected download status",
            )),
        }
    }

    /// Executes the download for the video stream only.
    ///
    /// # Errors
    ///
    /// Returns an error if the video stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video stream file.
    pub async fn execute_video_stream(self) -> Result<PathBuf> {
        let video_quality = self.video_quality.unwrap_or(VideoQuality::Best);
        let video_codec = self.video_codec.unwrap_or(VideoCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            video_quality = ?video_quality,
            video_codec = ?video_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "Executing video stream download"
        );

        let video_format = self
            .video
            .select_video_format(video_quality, video_codec)
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Video))?;

        let video_url = video_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &video_format.format_id))?;

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Video",
            video_url,
            Some(video_format.download_info.http_headers.clone()),
        )
        .await
    }

    /// Executes the download for the audio stream only.
    ///
    /// # Errors
    ///
    /// Returns an error if the audio stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded audio stream file.
    pub async fn execute_audio_stream(self) -> Result<PathBuf> {
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let audio_codec = self.audio_codec.unwrap_or(AudioCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            audio_quality = ?audio_quality,
            audio_codec = ?audio_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "Executing audio stream download"
        );

        let audio_format = self
            .video
            .select_audio_format(audio_quality, audio_codec)
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Audio))?;

        let audio_url = audio_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &audio_format.format_id))?;

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Audio",
            audio_url,
            Some(audio_format.download_info.http_headers.clone()),
        )
        .await
    }

    /// Executes the download for the storyboard.
    ///
    /// # Errors
    ///
    /// Returns an error if a fragment download fails.
    ///
    /// # Returns
    ///
    /// Returns a vector of paths for the downloaded fragments.
    pub async fn execute_storyboard(self) -> Result<Vec<PathBuf>> {
        let quality = self.storyboard_quality.unwrap_or(StoryboardQuality::Best);

        tracing::debug!(
            video_id = %self.video.id,
            output_dir = ?self.output,
            quality = ?quality,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            "Executing storyboard download"
        );

        let format = self
            .video
            .select_storyboard_format(quality)
            .ok_or_else(|| crate::error::Error::FormatNotAvailable {
                video_id: self.video.id.clone(),
                format_type: FormatType::Storyboard,
                available_formats: vec![],
            })?;

        let fragments = format
            .storyboard_info
            .fragments
            .as_deref()
            .unwrap_or_default();

        let prefix = format
            .video_id
            .as_deref()
            .unwrap_or(format.format_id.as_str());

        let output_dir_path = if self.output.is_absolute() {
            self.output.clone()
        } else {
            self.downloader.output_dir.join(&self.output)
        };

        let mut paths = Vec::with_capacity(fragments.len());
        let mut download_ids = Vec::with_capacity(fragments.len());

        // Progress tracking for fragments
        let fragment_count = fragments.len() as f64;
        let progress_callback = self.progress_callback.map(Arc::new);

        for (index, fragment) in fragments.iter().enumerate() {
            let filename = format!("{}_sb_{}_{:04}.mhtml", prefix, format.format_id, index);
            let output_path = output_dir_path.join(&filename);
            paths.push(output_path.clone());

            let callback = progress_callback.clone();

            let raw_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>> = callback.map(|cb| {
                Box::new(move |downloaded: u64, total: u64| {
                    let fragment_base = index as f64 / fragment_count;
                    let fragment_progress = if total > 0 {
                        (downloaded as f64 / total as f64) / fragment_count
                    } else {
                        0.0
                    };
                    cb(fragment_base + fragment_progress);
                }) as Box<dyn Fn(u64, u64) + Send + Sync>
            });

            let id = Self::enqueue_download(
                self.downloader,
                &fragment.url,
                output_path,
                self.priority,
                None, // storyboard doesn't use custom headers from format
                raw_callback,
            )
            .await;

            download_ids.push(id);
        }

        use crate::download::DownloadStatus;
        for id in download_ids {
            match self.downloader.wait_for_download(id).await {
                Some(DownloadStatus::Completed) => continue,
                Some(DownloadStatus::Failed { reason }) => {
                    return Err(crate::error::Error::download_failed(
                        id,
                        format!("Storyboard fragment download failed: {}", reason),
                    ));
                }
                Some(DownloadStatus::Canceled) => {
                    return Err(crate::error::Error::DownloadCancelled { download_id: id });
                }
                _ => {
                    return Err(crate::error::Error::download_failed(
                        id,
                        "Unexpected download status",
                    ));
                }
            }
        }

        Ok(paths)
    }

    /// Executes the download for the thumbnail.
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded thumbnail file.
    pub async fn execute_thumbnail(self) -> Result<PathBuf> {
        let quality = self.thumbnail_quality.unwrap_or(ThumbnailQuality::Best);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            quality = ?quality,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            "Executing thumbnail download"
        );

        let thumbnail = self.video.select_thumbnail(quality).ok_or_else(|| {
            crate::error::Error::NoThumbnail { video_id: self.video.id.clone() }
        })?;

        let http_headers =
            self.downloader
                .user_agent
                .clone()
                .map(|ua| crate::model::format::HttpHeaders {
                    user_agent: ua,
                    accept: "*/*".to_string(),
                    accept_language: "en-US,en".to_string(),
                    sec_fetch_mode: "navigate".to_string(),
                });

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Thumbnail",
            &thumbnail.url,
            http_headers,
        )
        .await
    }

    async fn enqueue_download(
        downloader: &crate::Downloader,
        url: &str,
        output_path: PathBuf,
        priority: crate::download::DownloadPriority,
        http_headers: Option<crate::model::format::HttpHeaders>,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>>,
    ) -> u64 {
        if let Some(cb) = progress_callback {
            downloader
                .download_manager
                .enqueue_with_progress_and_headers(
                    url,
                    output_path,
                    Some(priority),
                    cb,
                    http_headers,
                )
                .await
        } else {
            downloader
                .download_manager
                .enqueue_with_headers(url, output_path, Some(priority), http_headers)
                .await
        }
    }

    async fn execute_stream_internal(
        downloader: &crate::Downloader,
        output: &std::path::Path,
        priority: crate::download::DownloadPriority,
        progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
        format_type_name: &str,
        url: &str,
        http_headers: Option<crate::model::format::HttpHeaders>,
    ) -> Result<PathBuf> {
        let path = if output.is_absolute() {
            output.to_path_buf()
        } else {
            downloader.output_dir.join(output)
        };

        let raw_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>> =
            progress_callback.map(|cb| {
                Box::new(move |downloaded: u64, total: u64| {
                    if total > 0 {
                        cb(downloaded as f64 / total as f64);
                    }
                }) as Box<dyn Fn(u64, u64) + Send + Sync>
            });

        let download_id = Self::enqueue_download(
            downloader,
            url,
            path.clone(),
            priority,
            http_headers,
            raw_callback,
        )
        .await;

        match downloader.wait_for_download(download_id).await {
            Some(DownloadStatus::Completed) => Ok(path),
            Some(DownloadStatus::Failed { reason }) => Err(crate::error::Error::download_failed(
                download_id,
                format!("{} download failed: {}", format_type_name, reason),
            )),
            Some(DownloadStatus::Canceled) => {
                Err(crate::error::Error::DownloadCancelled { download_id })
            }
            _ => Err(crate::error::Error::download_failed(
                download_id,
                "Unexpected download status",
            )),
        }
    }

    fn format_not_available(video: &Video, format_type: FormatType) -> crate::error::Error {
        crate::error::Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        }
    }

    fn format_no_url(video_id: &str, format_id: &str) -> crate::error::Error {
        crate::error::Error::FormatNoUrl {
            video_id: video_id.to_string(),
            format_id: format_id.to_string(),
        }
    }
}
