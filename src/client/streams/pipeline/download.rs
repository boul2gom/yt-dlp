use std::path::{Path, PathBuf};

use crate::client::streams::selection::VideoSelection;
use crate::download::Fetcher;
use crate::error::Error;
use crate::model::Video;
use crate::model::format::{Format, FormatType};
#[cfg(cache)]
use crate::model::selector::FormatPreferences;
use crate::model::selector::ThumbnailQuality;
#[cfg(cache)]
use crate::utils;
use crate::{DownloadStatus, Downloader};

impl Downloader {
    /// Fetch the video, download it (video with audio) and returns its path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    /// let path = downloader.download_video(&video, "downloaded_video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video(&self, video: &Video, output: impl AsRef<str>) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_to_path(video, &output_path).await
    }

    /// Fetch the video, download it (video with audio) to a specific path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The absolute or relative path to save the video to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_video_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        tracing::info!(title = video.title, "📥 Downloading video");

        let path = output.into();

        // Check if the video is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            // Try to find the video in the cache by its ID
            if let Ok(Some((_, cached_path))) = cache.downloads.get_by_hash(&video.id).await {
                tracing::debug!(video_id = video.id, "🔍 Cache hit for downloaded video");

                // Hard link if possible, fall back to copy for cross-filesystem
                if tokio::fs::hard_link(&cached_path, &path).await.is_err() {
                    tokio::fs::copy(&cached_path, &path).await?;
                }
                return Ok(path);
            }
        }

        let best_video = video.best_video_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Video,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        let best_audio = video.best_audio_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Audio,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        // Download and combine video and audio, embedding metadata in a single ffmpeg pass
        self.download_and_combine_with_meta(video, best_video, best_audio, &path)
            .await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(video_id = video.id, "🔍 Caching downloaded video");

            let output_str = utils::try_name(path.as_path()).unwrap_or_default();

            if let Err(_e) = cache
                .downloads
                .put_file(&path, output_str, Some(video.id.clone()), None)
                .await
            {
                tracing::warn!(error = %_e, "Failed to cache downloaded video");
            }
        }

        Ok(path)
    }

    /// Download the video only, and returns its path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_video_stream(&self, video: &Video, output: impl AsRef<str>) -> crate::error::Result<PathBuf> {
        tracing::debug!(title = video.title, "📥 Downloading video stream");

        let best_video = video.best_video_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Video,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        self.download_format(best_video, output).await
    }

    /// Download the video stream to a specific path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The absolute or relative path to save the video to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_video_stream_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        tracing::debug!(title = video.title, "📥 Downloading video stream to path");

        let best_video = video.best_video_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Video,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        self.download_format_to_path(best_video, output).await
    }

    /// Downloads the thumbnail of a video.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `quality` - The requested thumbnail quality.
    /// * `output` - The path to save the thumbnail to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded thumbnail.
    pub async fn download_thumbnail(
        &self,
        video: &Video,
        quality: ThumbnailQuality,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        let output: PathBuf = output.into();
        tracing::debug!(
            video_id = %video.id,
            quality = ?quality,
            "🖼️ Downloading thumbnail for {}", video.title
        );

        let thumbnail = video
            .select_thumbnail(quality)
            .ok_or_else(|| crate::error::Error::NoThumbnail {
                video_id: video.id.clone(),
            })?;

        let http_headers = self.user_agent.clone().map(|ua| crate::model::format::HttpHeaders {
            user_agent: ua,
            accept: "*/*".to_string(),
            accept_language: "en-US,en".to_string(),
            sec_fetch_mode: "navigate".to_string(),
        });

        let id = self
            .download_manager
            .enqueue_with_headers(
                &thumbnail.url,
                output.clone(),
                Some(crate::download::DownloadPriority::Normal),
                http_headers,
            )
            .await;

        match self.wait_for_download(id).await {
            Some(DownloadStatus::Completed) => Ok(output),
            Some(DownloadStatus::Failed { reason }) => Err(crate::error::Error::download_failed(
                id,
                format!("Thumbnail download failed: {}", reason),
            )),
            Some(DownloadStatus::Canceled) => Err(crate::error::Error::DownloadCancelled { download_id: id }),
            _ => Err(crate::error::Error::download_failed(id, "Unexpected download status")),
        }
    }

    /// Fetch the audio stream, download it and returns its path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_audio_stream(&self, video: &Video, output: impl AsRef<str>) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_audio_stream_to_path(video, &output_path).await
    }

    /// Fetch the audio stream, download it to a specific path.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output` - The absolute or relative path to save the audio to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_audio_stream_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        tracing::debug!(title = video.title, "📥 Downloading audio stream");

        let best_audio = video.best_audio_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Audio,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        self.download_format_to_path(best_audio, output).await
    }

    /// Downloads a format.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_format(&self, format: &Format, output: impl AsRef<str>) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_format_to_path(format, &output_path).await
    }

    /// Downloads a format to a specific path.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download.
    /// * `output` - The absolute or relative path to save the format to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_format_to_path(
        &self,
        format: &Format,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        tracing::debug!(format_id = format.format_id, "📥 Downloading format");

        let output_path = output.into();

        // Use the internal function to download the format without preferences
        cfg_if::cfg_if! {
            if #[cfg(cache)] {
                self.download_format_internal(format, &output_path, FormatPreferences::default()).await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Internal function that handles downloading a format with or without preferences
    pub(crate) async fn download_format_internal(
        &self,
        format: &Format,
        path: &PathBuf,
        #[cfg(cache)] preferences: FormatPreferences,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(cache)]
        let has_preferences = preferences.has_any();

        // Check if the format is in the cache
        #[cfg(cache)]
        if let Some(cached_path) = self
            .lookup_cached_format(format, path, &preferences, has_preferences)
            .await?
        {
            return Ok(cached_path);
        }

        // Check if URL is available
        let url = format.download_info.url.clone().ok_or_else(|| Error::FormatNoUrl {
            video_id: format.video_id.clone().unwrap_or_else(|| "unknown".to_string()),
            format_id: format.format_id.clone(),
        })?;

        // Create an optimized fetcher with parallel downloading, driven by the configured SpeedProfile
        let fetcher = Fetcher::new(&url, self.proxy.as_ref(), None)?
            .with_parallel_segments(self.download_manager.parallel_segments())
            .with_segment_size(self.download_manager.segment_size())
            .with_retry_attempts(self.download_manager.retry_attempts());

        fetcher.fetch_asset(path.clone()).await?;

        // Don't add metadata for video or audio streams that will be combined later
        // Only add metadata for standalone formats that contain both
        // audio and video, or for audio-only formats intended for direct use
        self.add_metadata_if_needed(path, format).await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(cache)]
        self.cache_format_output(format, path, has_preferences, &preferences)
            .await;

        Ok(path.clone())
    }

    /// Checks the two-level download cache for a format and copies it to `path` on hit.
    ///
    /// First tries an exact format-ID lookup; then falls back to a preference-based
    /// lookup when `has_preferences` is `true`. On a hit the cached file is hard-linked
    /// (or copied for cross-filesystem paths) to `path`.
    ///
    /// # Arguments
    ///
    /// * `format` - The format whose cache entry is sought.
    /// * `path` - Destination path where the cached file should be placed.
    /// * `preferences` - Format preferences used for the secondary lookup.
    /// * `has_preferences` - Whether `preferences` contains any non-default values.
    ///
    /// # Errors
    ///
    /// Returns an error if the file copy operation fails.
    ///
    /// # Returns
    ///
    /// `Some(path)` when a cache hit is found and the file has been placed at `path`,
    /// `None` otherwise.
    #[cfg(cache)]
    async fn lookup_cached_format(
        &self,
        format: &Format,
        path: &Path,
        preferences: &FormatPreferences,
        has_preferences: bool,
    ) -> crate::error::Result<Option<PathBuf>> {
        let Some(cache) = &self.cache else { return Ok(None) };
        let Some(video_id) = format.video_id.as_ref() else {
            return Ok(None);
        };

        if let Ok(Some((_, cached_path))) = cache
            .downloads
            .get_by_video_and_format(video_id, &format.format_id)
            .await
        {
            tracing::debug!(format_id = format.format_id, "🔍 Using cached format");
            if tokio::fs::hard_link(&cached_path, path).await.is_err() {
                tokio::fs::copy(&cached_path, path).await?;
            }
            return Ok(Some(path.to_path_buf()));
        }

        if has_preferences
            && let Ok(Some((_, cached_path))) = cache
                .downloads
                .get_by_video_and_preferences(video_id, preferences)
                .await
        {
            tracing::debug!("🔍 Using cached format by preferences");
            if tokio::fs::hard_link(&cached_path, path).await.is_err() {
                tokio::fs::copy(&cached_path, path).await?;
            }
            return Ok(Some(path.to_path_buf()));
        }

        Ok(None)
    }

    /// Writes a downloaded format file to the cache, using preferences when available.
    ///
    /// Logs a warning on cache failure without propagating the error so that a cache
    /// write failure never aborts a successful download.
    ///
    /// # Arguments
    ///
    /// * `format` - The downloaded format to store.
    /// * `path` - Path of the local file that was just downloaded.
    /// * `has_preferences` - Whether `preferences` contains any non-default values.
    /// * `preferences` - Format preferences used when `has_preferences` is `true`.
    #[cfg(cache)]
    async fn cache_format_output(
        &self,
        format: &Format,
        path: &Path,
        has_preferences: bool,
        preferences: &FormatPreferences,
    ) {
        let Some(cache) = &self.cache else { return };
        let output_str = utils::try_name(path).unwrap_or_default();

        tracing::debug!(format_id = format.format_id, "🔍 Caching format");

        if has_preferences {
            if let Some(video_id) = format.video_id.as_ref()
                && let Err(_e) = cache
                    .downloads
                    .put_file_with_preferences(path, output_str, Some(video_id.clone()), Some(format), preferences)
                    .await
            {
                tracing::warn!(error = %_e, "Failed to cache format with preferences");
            }
        } else if let Err(_e) = cache
            .downloads
            .put_file(path, output_str, format.video_id.clone(), Some(format))
            .await
        {
            tracing::warn!(error = %_e, "Failed to cache format");
        }
    }
}
