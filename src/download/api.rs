use std::path::PathBuf;

use crate::error::Result;
use crate::model::Video;
use crate::{DownloadPriority, DownloadStatus, Downloader};

impl Downloader {
    /// Download a video using the download manager with priority.
    ///
    /// This method adds the video download to the download queue with the specified priority.
    /// The download will be processed according to its priority and the current load.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    /// * `priority` - The download priority (optional).
    ///
    /// # Returns
    ///
    /// The download ID that can be used to track the download status.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video information could not be retrieved.
    pub async fn download_video_with_priority(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        priority: Option<DownloadPriority>,
    ) -> Result<u64> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_with_priority_to_path(video, &output_path, priority)
            .await
    }

    /// Download a video using the download manager with priority to a specific path.
    ///
    /// Unlike [`download_video_with_priority`](Self::download_video_with_priority), this method
    /// writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The full path where the file will be saved.
    /// * `priority` - The download priority (optional).
    ///
    /// # Returns
    ///
    /// The download ID that can be used to track the download status.
    pub async fn download_video_with_priority_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        priority: Option<DownloadPriority>,
    ) -> Result<u64> {
        let output_path: PathBuf = output.into();

        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            output_path = ?output_path,
            priority = ?priority,
            "📥 Downloading video with priority"
        );

        // Get the best format with video and audio
        let format = video.best_audio_video_format()?;

        tracing::debug!(
            video_id = video.id,
            format_id = format.format_id,
            format_type = ?format.format_type(),
            "🧩 Selected format for download"
        );

        // Get the URL
        let url = format.url()?;

        // Add to download queue
        let download_id = self.download_manager.enqueue(url, output_path, priority).await;

        tracing::debug!(
            video_id = video.id,
            download_id = download_id,
            "📥 Video added to download queue"
        );

        Ok(download_id)
    }

    /// Download a video using the download manager with progress tracking.
    ///
    /// This method adds the video download to the download queue and provides progress updates.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    /// * `progress_callback` - A function that will be called with progress updates.
    ///
    /// # Returns
    ///
    /// The download ID that can be used to track the download status.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video information could not be retrieved.
    pub async fn download_video_with_progress<F>(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        progress_callback: F,
    ) -> Result<u64>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_with_progress_to_path(video, &output_path, progress_callback)
            .await
    }

    /// Download a video using the download manager with progress tracking to a specific path.
    ///
    /// Unlike [`download_video_with_progress`](Self::download_video_with_progress), this method
    /// writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The full path where the file will be saved.
    /// * `progress_callback` - A function that will be called with progress updates.
    ///
    /// # Returns
    ///
    /// The download ID that can be used to track the download status.
    pub async fn download_video_with_progress_to_path<F>(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        progress_callback: F,
    ) -> Result<u64>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let output_path: PathBuf = output.into();

        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            output_path = ?output_path,
            "📥 Downloading video with progress tracking"
        );

        // Get the best format with video and audio
        let format = video.best_audio_video_format()?;

        tracing::debug!(
            video_id = video.id,
            format_id = format.format_id,
            format_type = ?format.format_type(),
            "🧩 Selected format for download with progress"
        );

        // Get the URL
        let url = format.url()?;

        // Add to download queue with progress callback
        let download_id = self
            .download_manager
            .enqueue_with_progress(url, output_path, Some(DownloadPriority::Normal), progress_callback)
            .await;

        tracing::debug!(
            video_id = video.id,
            download_id = download_id,
            "📥 Video added to download queue with progress tracking"
        );

        Ok(download_id)
    }

    /// Get the status of a download.
    ///
    /// # Arguments
    ///
    /// * `download_id` - The ID of the download to check.
    ///
    /// # Returns
    ///
    /// The download status, or None if the download ID is not found.
    pub async fn get_download_status(&self, download_id: u64) -> Option<DownloadStatus> {
        self.download_manager.get_status(download_id).await
    }

    /// Cancel a download.
    ///
    /// # Arguments
    ///
    /// * `download_id` - The ID of the download to cancel.
    ///
    /// # Returns
    ///
    /// true if the download was canceled, false if it was not found or already completed.
    pub async fn cancel_download(&self, download_id: u64) -> bool {
        self.download_manager.cancel(download_id).await
    }

    /// Wait for a download to complete.
    ///
    /// # Arguments
    ///
    /// * `download_id` - The ID of the download to wait for.
    ///
    /// # Returns
    ///
    /// The final download status, or None if the download ID is not found.
    pub async fn wait_for_download(&self, download_id: u64) -> Option<DownloadStatus> {
        self.download_manager.wait_for_completion(download_id).await
    }
}
