use crate::Downloader;
use crate::client::streams::selection::VideoSelection;
use crate::download::Fetcher;
use crate::error::Error;
use crate::executor::Executor;
use crate::model::Video;
use crate::model::caption::Extension as CaptionExtension;
use crate::model::format::{Format, FormatType};
use crate::model::playlist::{Playlist, PlaylistDownloadProgress};
#[cfg(feature = "cache-backend")]
use crate::model::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};
use crate::utils;

use std::path::{Path, PathBuf};
use std::sync::Arc;

impl Downloader {
    /// Helper to check if a video is in the cache by URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The video URL to check
    ///
    /// # Returns
    ///
    /// `Some(Video)` if found in cache and not expired, `None` otherwise
    async fn check_video_cache(&self, url: &str) -> Option<Video> {
        #[cfg(feature = "cache-backend")]
        {
            tracing::debug!(url = url, "Checking video cache");

            let cache = self.cache.as_ref()?;
            let result = cache.get(url).await.ok().flatten();

            tracing::debug!(
                url = url,
                cache_hit = result.is_some(),
                "Video cache check completed"
            );

            result
        }
        #[cfg(not(feature = "cache-backend"))]
        {
            tracing::debug!(url = url, "Cache feature disabled");
            let _ = url;

            None
        }
    }

    /// Helper to determine the correct extractor for a given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The video URL
    ///
    /// # Returns
    ///
    /// Reference to the appropriate video extractor (YouTube or Generic)
    fn get_extractor(&self, url: &str) -> &dyn crate::extractor::VideoExtractor {
        let is_youtube = crate::extractor::Youtube::supports_url(url);

        tracing::debug!(
            url = url,
            is_youtube = is_youtube,
            "Selecting video extractor"
        );

        if is_youtube {
            &self.youtube_extractor
        } else {
            &self.generic_extractor
        }
    }

    /// Emits a `DownloadEvent` through all registered sinks in order:
    /// hooks (with 30 s timeout), webhooks (non-blocking channel send), then the broadcast bus.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to emit.
    pub(crate) async fn emit_event(&self, event: crate::events::DownloadEvent) {
        #[cfg(feature = "hooks")]
        if let Some(registry) = &self.hook_registry {
            registry.execute(&event).await;
        }

        #[cfg(feature = "webhooks")]
        if let Some(delivery) = &self.webhook_delivery {
            delivery.process_event(&event).await;
        }

        self.event_bus.emit(event);
    }

    /// Fetch the video information from the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    ///
    /// # Returns
    ///
    /// A `Video` struct containing metadata about the video.
    ///
    /// # Errors
    ///
    /// Returns an error if the yt-dlp command fails or the output cannot be parsed.
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
    /// println!("Video title: {}", video.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_video_infos(&self, url: impl AsRef<str>) -> crate::error::Result<Video> {
        let url_str = url.as_ref();

        tracing::debug!(url = url_str, "Fetching video information");

        if let Some(video) = self.check_video_cache(url_str).await {
            tracing::debug!(
                url = url_str,
                video_id = %video.id,
                video_title = %video.title,
                "Cache hit, returning cached video"
            );
            return Ok(video);
        }

        tracing::debug!(url = url_str, "Cache miss, fetching from extractor");

        let start = std::time::Instant::now();
        let result = self.get_extractor(url_str).fetch_video(url_str).await;
        let duration = start.elapsed();

        let video = match result {
            Ok(v) => {
                tracing::debug!(
                    url = url_str,
                    video_id = %v.id,
                    video_title = %v.title,
                    format_count = v.formats.len(),
                    duration = ?duration,
                    "Video information fetched successfully"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetched {
                    url: url_str.to_string(),
                    video: v.clone(),
                    duration,
                })
                .await;

                v
            }
            Err(e) => {
                tracing::debug!(
                    url = url_str,
                    error = %e,
                    duration = ?duration,
                    "Video information fetch failed"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetchFailed {
                    url: url_str.to_string(),
                    error: e.to_string(),
                    duration,
                })
                .await;

                return Err(e);
            }
        };

        #[cfg(feature = "cache-backend")]
        if let Some(cache) = &self.cache {
            tracing::debug!(video_id = %video.id, "Storing video in cache");

            let _ = cache.put(url_str.to_string(), video.clone()).await;
        }

        Ok(video)
    }

    /// Fetch the video information from the given URL, bypassing the cache.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    ///
    /// # Returns
    ///
    /// A `Video` struct containing metadata about the video.
    ///
    /// # Errors
    ///
    /// Returns an error if the yt-dlp command fails or the output cannot be parsed.
    pub async fn fetch_video_infos_fresh(
        &self,
        url: impl AsRef<str>,
    ) -> crate::error::Result<Video> {
        let url_str = url.as_ref();

        tracing::debug!(
            url = url_str,
            "Fetching fresh video information (bypassing cache)"
        );

        let start = std::time::Instant::now();
        let result = self.get_extractor(url_str).fetch_video(url_str).await;
        let duration = start.elapsed();

        let video = match result {
            Ok(v) => {
                tracing::debug!(
                    url = url_str,
                    video_id = %v.id,
                    video_title = %v.title,
                    format_count = v.formats.len(),
                    duration = ?duration,
                    "Fresh video information fetched successfully"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetched {
                    url: url_str.to_string(),
                    video: v.clone(),
                    duration,
                })
                .await;

                v
            }
            Err(e) => {
                tracing::debug!(
                    url = url_str,
                    error = %e,
                    duration = ?duration,
                    "Fresh video information fetch failed"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetchFailed {
                    url: url_str.to_string(),
                    error: e.to_string(),
                    duration,
                })
                .await;

                return Err(e);
            }
        };

        #[cfg(feature = "cache-backend")]
        if let Some(cache) = &self.cache {
            tracing::debug!(video_id = %video.id, "Updating cache with fresh video data");

            let _ = cache.put(url_str.to_string(), video.clone()).await;
        }

        Ok(video)
    }

    /// Helper to get video by ID from cache (if available)
    ///
    /// # Arguments
    ///
    /// * `id` - The video ID.
    ///
    /// # Returns
    ///
    /// `Some(Video)` if found in cache, `None` otherwise.
    pub async fn get_video_by_id(&self, id: &str) -> Option<Video> {
        #[cfg(feature = "cache-backend")]
        {
            tracing::debug!(video_id = id, "Getting video from cache by ID");

            let cache = self.cache.as_ref()?;
            let cached_video = cache.get_by_id(id).await.ok()?;
            let video = cached_video.video().ok();

            tracing::debug!(
                video_id = id,
                found = video.is_some(),
                "Video cache lookup by ID completed"
            );

            video
        }
        #[cfg(not(feature = "cache-backend"))]
        {
            tracing::debug!(video_id = id, "Cache feature disabled");
            let _ = id;

            None
        }
    }

    /// Helper to execute an action with automatic URL expiry retry.
    ///
    /// This method will execute the given action. If it fails with `Error::UrlExpired`,
    /// it will refresh the video metadata and retry the action once.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video.
    /// * `action` - A closure that takes a `Video` and returns a Future.
    pub async fn execute_with_retry<T, F, Fut>(
        &self,
        url: String,
        action: F,
    ) -> crate::error::Result<T>
    where
        F: Fn(Video) -> Fut + Send + Sync + Clone,
        Fut: Future<Output = crate::error::Result<T>> + Send,
    {
        // First attempt with potentially cached metadata
        let video = self.fetch_video_infos(url.clone()).await?;

        match action(video.clone()).await {
            Ok(result) => Ok(result),
            Err(Error::UrlExpired) => {
                tracing::warn!("URL expired, refreshing metadata and retrying...");

                // Refresh metadata bypassing cache
                let video = self.fetch_video_infos_fresh(&url).await?;
                // Retry action with fresh metadata
                action(video).await
            }
            Err(e) => Err(e),
        }
    }

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
    pub async fn download_video(
        &self,
        video: &Video,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
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
        tracing::debug!("Downloading video {}", video.title);

        let path = output.into();

        // Check if the video is in the cache
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the video in the cache by its ID
            if let Some((_, cached_path)) = download_cache.get_by_hash(&video.id).await {
                tracing::debug!("Caching downloaded video with ID: {}", video.id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let best_video = video
            .best_video_format()
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        let best_audio = video
            .best_audio_format()
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Audio,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Download and combine video and audio
        self.download_and_combine_formats(best_video, best_audio, &path)
            .await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache {
            tracing::debug!("Caching downloaded video with ID: {}", video.id);

            let output_str = utils::try_name(path.as_path()).unwrap_or_default();

            if let Err(_e) = download_cache
                .put_file(&path, output_str, Some(video.id.clone()), None)
                .await
            {
                tracing::warn!("Failed to cache downloaded video: {}", _e);
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
    pub async fn download_video_stream(
        &self,
        video: &Video,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        tracing::debug!("Downloading video stream {}", video.title);

        let best_video = video
            .best_video_format()
            .ok_or_else(|| Error::FormatNotAvailable {
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
        tracing::debug!("Downloading video stream to path {}", video.title);

        let best_video = video
            .best_video_format()
            .ok_or_else(|| Error::FormatNotAvailable {
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
    /// * `output` - The path to save the thumbnail to.
    ///
    /// # Returns
    ///
    /// The path to the downloaded thumbnail.
    pub async fn download_thumbnail(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
    ) -> crate::error::Result<PathBuf> {
        let output: PathBuf = output.into();
        tracing::debug!("Downloading thumbnail for {}", video.title);

        if let Some(thumbnail_url) = &video.thumbnail {
            let fetcher =
                Fetcher::new(thumbnail_url, self.proxy.as_ref(), self.user_agent.clone())?;
            fetcher.fetch_asset(&output).await?;
            Ok(output)
        } else {
            Err(Error::Unknown(
                "No thumbnail found for this video".to_string(),
            ))
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
    pub async fn download_audio_stream(
        &self,
        video: &Video,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_audio_stream_to_path(video, &output_path)
            .await
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
        tracing::debug!("Downloading audio stream {}", video.title);

        let best_audio = video
            .best_audio_format()
            .ok_or_else(|| Error::FormatNotAvailable {
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
    pub async fn download_format(
        &self,
        format: &Format,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
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
        tracing::debug!("Downloading format {}", format.format_id);

        let output_path = output.into();

        // Use the internal function to download the format without preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache-backend")] {
                self.download_format_internal(format, &output_path, None, None, None, None).await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Internal function that handles downloading a format with or without preferences
    async fn download_format_internal(
        &self,
        format: &Format,
        path: &PathBuf,
        #[cfg(feature = "cache-backend")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache-backend")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache-backend")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache-backend")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        // Check if we have specific preferences
        #[cfg(feature = "cache-backend")]
        let has_preferences = video_quality.is_some()
            || audio_quality.is_some()
            || video_codec.is_some()
            || audio_codec.is_some();

        // Check if the format is in the cache
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache
            && let Some(video_id) = format.video_id.as_ref()
        {
            // First try to find by exact format ID
            if let Some((_, cached_path)) = download_cache
                .get_by_video_and_format(video_id, &format.format_id)
                .await
            {
                tracing::debug!("Using cached format by ID: {}", format.format_id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, path).await?;
                return Ok(path.clone());
            }

            // Then try to find by preferences if they exist
            if has_preferences
                && let Some((_, cached_path)) = download_cache
                    .get_by_video_and_preferences(
                        video_id,
                        video_quality,
                        audio_quality,
                        video_codec.clone(),
                        audio_codec.clone(),
                    )
                    .await
            {
                tracing::debug!("Using cached format by preferences");

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, path).await?;
                return Ok(path.clone());
            }
        }

        // Check if URL is available
        let url = format
            .download_info
            .url
            .clone()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: format
                    .video_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                format_id: format.format_id.clone(),
            })?;

        // Create an optimized fetcher with parallel downloading
        let fetcher = Fetcher::new(&url, self.proxy.as_ref(), None)?
            .with_parallel_segments(8) // Use 8 parallel segments
            .with_segment_size(1024 * 1024 * 5) // 5 MB per segment
            .with_retry_attempts(3); // 3 attempts in case of failure

        fetcher.fetch_asset(path.clone()).await?;

        // Don't add metadata for video or audio streams that will be combined later
        // Only add metadata for standalone formats that contain both
        // audio and video, or for audio-only formats intended for direct use
        self.add_metadata_if_needed(path, format).await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache {
            let output_str = utils::try_name(path.as_path()).unwrap_or_default();

            tracing::debug!("Caching format with ID: {}", format.format_id);

            // Use the appropriate function depending on whether we have preferences or not
            if has_preferences {
                if let Some(video_id) = format.video_id.as_ref()
                    && let Err(_e) = download_cache
                        .put_file_with_preferences(
                            path,
                            output_str,
                            Some(video_id.clone()),
                            Some(format),
                            video_quality,
                            audio_quality,
                            video_codec,
                            audio_codec,
                        )
                        .await
                {
                    tracing::warn!("Failed to cache format with preferences: {}", _e);
                }
            } else if let Err(_e) = download_cache
                .put_file(path, output_str, format.video_id.clone(), Some(format))
                .await
            {
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(path.clone())
    }
    /// Downloads a subtitle file for a specific language.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `language_code` - The language code (e.g., "en", "es").
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded subtitle file.
    pub async fn download_subtitle(
        &self,
        video: &Video,
        language_code: impl AsRef<str>,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        let language_code = language_code.as_ref();

        tracing::debug!(
            "Downloading subtitle for video {} in language {}",
            video.id,
            language_code
        );

        let output_path = self.output_dir.join(output.as_ref());

        // Check if subtitle is in the cache
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache
            && let Some((_, cached_path)) = download_cache
                .get_subtitle_by_language(&video.id, language_code)
                .await
        {
            tracing::debug!(
                "Using cached subtitle for video {} in language {}",
                video.id,
                language_code
            );

            // Copy the file from the cache to the output directory
            tokio::fs::copy(&cached_path, &output_path).await?;
            return Ok(output_path);
        }

        // Get subtitles for the language
        let subtitles =
            video
                .subtitles
                .get(language_code)
                .ok_or_else(|| Error::SubtitleNotAvailable {
                    video_id: video.id.clone(),
                    language: language_code.to_string(),
                })?;

        // Prefer SRT format, then VTT, then any available format
        let subtitle = subtitles
            .iter()
            .find(|s| s.is_format(&CaptionExtension::Srt))
            .or_else(|| {
                subtitles
                    .iter()
                    .find(|s| s.is_format(&CaptionExtension::Vtt))
            })
            .or_else(|| subtitles.first())
            .ok_or_else(|| Error::SubtitleNotAvailable {
                video_id: video.id.clone(),
                language: language_code.to_string(),
            })?;

        tracing::debug!(
            "Downloading subtitle from {} to {:?}",
            subtitle.url,
            output_path
        );

        // Download the subtitle file
        let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
        fetcher.fetch_asset(&output_path).await?;

        // Cache the downloaded subtitle
        #[cfg(feature = "cache-backend")]
        if let Some(download_cache) = &self.download_cache {
            tracing::debug!(
                "Caching subtitle for video {} in language {}",
                video.id,
                language_code
            );

            if let Err(_e) = download_cache
                .put_subtitle_file(
                    &output_path,
                    output.as_ref(),
                    video.id.clone(),
                    language_code.to_string(),
                )
                .await
            {
                tracing::warn!("Failed to cache subtitle: {}", _e);
            }
        }

        tracing::info!(
            "Successfully downloaded subtitle for language {} to {:?}",
            language_code,
            output_path
        );

        Ok(output_path)
    }

    /// Downloads all available subtitles for a video.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output_dir` - The directory to save the subtitles to.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded subtitle files.
    pub async fn download_all_subtitles(
        &self,
        video: &Video,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!("Downloading all subtitles for video {}", video.id);

        let output_dir = output_dir.as_ref();
        let mut downloaded_files = Vec::new();

        for (language_code, subtitles) in &video.subtitles {
            if let Some(subtitle) = subtitles.first() {
                let filename = format!(
                    "{}.{}.{}",
                    video.id,
                    language_code,
                    subtitle.file_extension()
                );
                let output_path = output_dir.join(&filename);

                tracing::debug!(
                    "Downloading subtitle for language {} from {}",
                    language_code,
                    subtitle.url
                );

                let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
                fetcher.fetch_asset(&output_path).await?;
                downloaded_files.push(output_path);
            }
        }

        tracing::info!(
            "Successfully downloaded {} subtitle files",
            downloaded_files.len()
        );

        Ok(downloaded_files)
    }

    /// Fetches playlist information from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist.
    ///
    /// # Returns
    ///
    /// A `Playlist` struct containing metadata about the playlist and its videos.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist cannot be fetched or parsed.
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
    /// let playlist = downloader.fetch_playlist_infos("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf").await?;
    /// println!("Playlist title: {}", playlist.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_playlist_infos(
        &self,
        url: impl AsRef<str>,
    ) -> crate::error::Result<Playlist> {
        let url_str = url.as_ref();
        tracing::debug!("Fetching playlist information from {}", url_str);

        // Check if the playlist is in the cache
        #[cfg(feature = "cache-backend")]
        if let Some(cache) = &self.playlist_cache
            && let Some(playlist) = cache.get(url_str).await?
        {
            tracing::debug!("Using cached playlist information for {}", url_str);
            return Ok(playlist);
        }

        // Delegate to the extractor
        let extractor = self.get_extractor(url_str);
        tracing::debug!(
            "Fetching playlist information using {} extractor",
            extractor.name()
        );

        let start = std::time::Instant::now();
        let result = extractor.fetch_playlist(url_str).await;
        let duration = start.elapsed();

        let mut playlist = match result {
            Ok(p) => {
                tracing::debug!(
                    url = url_str,
                    playlist_id = %p.id,
                    entry_count = p.entry_count(),
                    duration = ?duration,
                    "Playlist information fetched successfully"
                );

                self.emit_event(crate::events::DownloadEvent::PlaylistFetched {
                    url: url_str.to_string(),
                    playlist: p.clone(),
                    duration,
                })
                .await;

                p
            }
            Err(e) => {
                tracing::debug!(
                    url = url_str,
                    error = %e,
                    duration = ?duration,
                    "Playlist information fetch failed"
                );

                self.emit_event(crate::events::DownloadEvent::PlaylistFetchFailed {
                    url: url_str.to_string(),
                    error: e.to_string(),
                    duration,
                })
                .await;

                return Err(e);
            }
        };

        // Store the URL in the playlist for caching purposes
        playlist.url = Some(url_str.to_string());

        // Cache the playlist if caching is enabled
        #[cfg(feature = "cache-backend")]
        if let Some(cache) = &self.playlist_cache {
            tracing::debug!("Caching playlist information for {}", url_str);

            if let Err(_e) = cache.put(url_str.to_string(), playlist.clone()).await {
                tracing::warn!("Failed to cache playlist information: {}", _e);
            }
        }

        tracing::info!(
            "Successfully fetched playlist {} with {} videos",
            playlist.id,
            playlist.entry_count()
        );

        Ok(playlist)
    }

    /// Downloads all videos from a playlist.
    ///
    /// This method uses the `download_playlist_parallel` method internally with default concurrency settings.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames (e.g., "%(title)s.%(ext)s").
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
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
    /// let playlist = downloader.fetch_playlist_infos("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf.").await?;
    ///
    /// // Download all videos in the playlist
    /// let paths = downloader.download_playlist(&playlist, "%(title)s.%(ext)s").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_playlist(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            "Downloading playlist {} with {} videos using parallel mode",
            playlist.id,
            playlist.entry_count()
        );

        // Use None to let download_playlist_parallel use its default concurrent limit
        // The limit is already configured in the download manager based on the speed profile
        let max_concurrent = None;

        // Use parallel download mode by default for better performance
        let results = self
            .download_playlist_parallel(playlist, output_pattern, max_concurrent)
            .await?;

        // Convert results to a simple Vec<PathBuf>, filtering out errors
        // and collecting only successful downloads
        let mut downloaded_files = Vec::new();
        let mut errors = Vec::new();

        for (video_idx, result) in results.into_iter().enumerate() {
            match result {
                Ok(path) => downloaded_files.push(path),
                Err(e) => {
                    tracing::error!("Failed to download video at index {video_idx}: {e}");
                    errors.push(e);
                }
            }
        }

        // If there were any errors, return the first one
        // (to maintain backward compatibility with the previous sequential behavior)
        if !errors.is_empty() && downloaded_files.is_empty() {
            return Err(errors.into_iter().next().unwrap());
        }

        tracing::info!(
            "Successfully downloaded {} out of {} videos from playlist {}",
            downloaded_files.len(),
            playlist.entry_count(),
            playlist.id
        );

        Ok(downloaded_files)
    }

    /// Downloads all videos from a playlist in parallel.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames.
    /// * `max_concurrent` - Optional limit on concurrent downloads (defaults to 3).
    ///
    /// # Returns
    ///
    /// A vector of Results, each containing the path to a downloaded video or an error.
    pub async fn download_playlist_parallel(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>> {
        self.download_playlist_parallel_with_progress::<fn(PlaylistDownloadProgress)>(
            playlist,
            output_pattern,
            max_concurrent,
            None,
        )
        .await
    }

    /// Downloads all videos from a playlist in parallel with progress tracking.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames.
    /// * `max_concurrent` - Optional limit on concurrent downloads.
    /// * `progress_callback` - Optional closure called with `PlaylistDownloadProgress` updates.
    ///
    /// # Returns
    ///
    /// A vector of Results, each containing the path to a downloaded video or an error.
    pub async fn download_playlist_parallel_with_progress<F>(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
        progress_callback: Option<F>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>>
    where
        F: Fn(PlaylistDownloadProgress) + Send + Sync + 'static,
    {
        use futures_util::stream::{FuturesUnordered, StreamExt};

        tracing::debug!(
            "Downloading playlist {} with {} videos in parallel (max {} concurrent)",
            playlist.id,
            playlist.entry_count(),
            max_concurrent.unwrap_or(3)
        );

        let max_concurrent = max_concurrent.unwrap_or(3);
        let total_videos = playlist.entry_count();
        let mut completed = 0usize;
        let mut results = Vec::new();
        let mut tasks = FuturesUnordered::new();
        let mut entry_iter = playlist.entries.iter().peekable();

        let output_pattern = output_pattern.as_ref().to_string();
        let progress_callback = progress_callback.map(Arc::new);

        loop {
            // Spawn tasks up to max_concurrent limit
            while tasks.len() < max_concurrent {
                if let Some(entry) = entry_iter.next() {
                    if !entry.is_available() {
                        tracing::warn!(
                            "Skipping unavailable video: {} ({})",
                            entry.title,
                            entry.id
                        );

                        let entry_clone = entry.clone();
                        completed += 1;

                        // Call progress callback for unavailable video
                        if let Some(callback) = &progress_callback {
                            callback(PlaylistDownloadProgress {
                                entry: entry_clone.clone(),
                                result: Err(format!("Video {} is not available", entry_clone.id)),
                                completed,
                                total: total_videos,
                            });
                        }

                        results.push(Err(Error::Unknown(format!(
                            "Video {} is not available",
                            entry.id
                        ))));
                        continue;
                    }

                    let entry = entry.clone();
                    let output_pattern = output_pattern.clone();
                    let youtube = self.clone();
                    let _callback = progress_callback.clone();

                    let task = tokio::spawn(async move {
                        tracing::debug!(
                            "Downloading video {} from playlist (index: {})",
                            entry.id,
                            entry.index.unwrap_or(0)
                        );

                        // Fetch full video info
                        let video_result = youtube.fetch_video_infos(entry.url.clone()).await;
                        let video = match video_result {
                            Ok(v) => v,
                            Err(e) => return (entry, Err(e)),
                        };

                        // Generate filename from pattern
                        let filename = output_pattern
                            .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                            .replace("%(title)s", &entry.title)
                            .replace("%(id)s", &entry.id);

                        // Download the video
                        let download_result = youtube.download_video(&video, &filename).await;

                        if download_result.is_ok() {
                            tracing::info!(
                                "Downloaded video from playlist: {} (index: {})",
                                entry.title,
                                entry.index.unwrap_or(0)
                            );
                        }

                        (entry, download_result)
                    });

                    tasks.push(task);
                } else {
                    // No more entries to spawn
                    break;
                }
            }

            // If no tasks running and no more entries, we're done
            if tasks.is_empty() {
                break;
            }

            // Wait for next task to complete
            if let Some(result) = tasks.next().await {
                completed += 1;

                match result {
                    Ok((entry, download_result)) => {
                        // Call progress callback
                        if let Some(callback) = &progress_callback {
                            let result_for_progress = download_result
                                .as_ref()
                                .map(|p| p.clone())
                                .map_err(|e| e.to_string());

                            callback(PlaylistDownloadProgress {
                                entry,
                                result: result_for_progress,
                                completed,
                                total: total_videos,
                            });
                        }

                        results.push(download_result);
                    }
                    Err(e) => {
                        results.push(Err(Error::Unknown(format!("Task join error: {}", e))));
                    }
                }
            }
        }

        {
            let successful = results.iter().filter(|r| r.is_ok()).count();
            tracing::info!(
                "Downloaded {}/{} videos from playlist {} in parallel",
                successful,
                playlist.entry_count(),
                playlist.id
            );
        }

        Ok(results)
    }

    /// Downloads specific videos from a playlist by their indices.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `indices` - A slice of indices (0-based) of videos to download.
    /// * `output_pattern` - pattern for output filenames.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
    pub async fn download_playlist_items(
        &self,
        playlist: &Playlist,
        indices: &[usize],
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            "Downloading {} specific videos from playlist {}",
            indices.len(),
            playlist.id
        );

        let mut downloaded_files = Vec::new();

        for &index in indices {
            if let Some(entry) = playlist.get_entry_by_index(index) {
                if !entry.is_available() {
                    tracing::warn!(
                        "Skipping unavailable video at index {}: {}",
                        index,
                        entry.title
                    );
                    continue;
                }

                // Fetch full video info
                let video = self.fetch_video_infos(entry.url.clone()).await?;

                // Generate filename from pattern
                let filename = output_pattern
                    .as_ref()
                    .replace("%(playlist_index)s", &index.to_string())
                    .replace("%(title)s", &entry.title)
                    .replace("%(id)s", &entry.id);

                // Download the video
                let video_path = self.download_video(&video, &filename).await?;
                downloaded_files.push(video_path);

                tracing::info!("Downloaded video at index {}: {}", index, entry.title);
            } else {
                tracing::warn!("Index {} is out of bounds for playlist", index);
            }
        }

        Ok(downloaded_files)
    }

    /// Downloads a range of videos from a playlist.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `start` - The start index (inclusive).
    /// * `end` - The end index (inclusive).
    /// * `output_pattern` - pattern for output filenames.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
    pub async fn download_playlist_range(
        &self,
        playlist: &Playlist,
        start: usize,
        end: usize,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            "Downloading videos {}-{} from playlist {}",
            start,
            end,
            playlist.id
        );

        let entries = playlist.get_entries_in_range(start, end);
        let mut downloaded_files = Vec::new();

        for entry in entries {
            if !entry.is_available() {
                tracing::warn!("Skipping unavailable video: {} ({})", entry.title, entry.id);
                continue;
            }

            // Fetch full video info
            let video = self.fetch_video_infos(entry.url.clone()).await?;

            // Generate filename from pattern
            let filename = output_pattern
                .as_ref()
                .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                .replace("%(title)s", &entry.title)
                .replace("%(id)s", &entry.id);

            // Download the video
            let video_path = self.download_video(&video, &filename).await?;
            downloaded_files.push(video_path);

            tracing::info!("Downloaded video: {}", entry.title);
        }

        Ok(downloaded_files)
    }

    /// Downloads a partial range of a video using hybrid approach (yt-dlp with ffmpeg fallback).
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `range` - The partial range (time or chapters) to download.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Returns
    ///
    /// The path to the downloaded partial video file.
    pub async fn download_video_partial(
        &self,
        video: &Video,
        range: &crate::download::partial::PartialRange,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        // Convert chapter ranges to time ranges if needed
        let time_range = if range.needs_chapter_metadata() {
            if !video.chapters.is_empty() {
                range
                    .to_time_range(&video.chapters)
                    .ok_or_else(|| Error::Unknown("Chapter index out of bounds".to_string()))?
            } else {
                return Err(Error::Unknown(
                    "Video does not have chapter information".to_string(),
                ));
            }
        } else {
            range.clone()
        };

        let output_path = self.output_dir.join(output.as_ref());

        // Try yt-dlp approach first
        match self
            .try_download_partial_ytdlp(video, &time_range, &output_path)
            .await
        {
            Ok(path) => {
                tracing::info!("Successfully downloaded partial video using yt-dlp");
                Ok(path)
            }
            Err(_e) => {
                tracing::warn!(
                    "yt-dlp partial download failed: {}, trying ffmpeg fallback",
                    _e
                );

                // Fallback to ffmpeg approach
                self.download_partial_ffmpeg(video, &time_range, &output_path)
                    .await
            }
        }
    }

    /// Attempts to download a partial video using yt-dlp's --download-sections.
    async fn try_download_partial_ytdlp(
        &self,
        video: &Video,
        range: &crate::download::partial::PartialRange,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        let output_str = output_path.to_str().ok_or_else(|| Error::PathValidation {
            path: output_path.to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let download_sections_arg = range.to_ytdlp_arg();
        let video_url = format!("https://www.youtube.com/watch?v={}", video.id);

        let download_args = vec![
            "--no-progress",
            "--download-sections",
            &download_sections_arg,
            "-o",
            output_str,
            &video_url,
        ];

        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(download_args));

        let executor = Executor::new(self.libraries.youtube.clone(), final_args, self.timeout);

        executor.execute().await?;
        Ok(output_path.to_path_buf())
    }

    /// Downloads full video and extracts partial range using ffmpeg.
    async fn download_partial_ffmpeg(
        &self,
        video: &Video,
        range: &crate::download::partial::PartialRange,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        // Get time range
        let (start_time, end_time) = range
            .get_times()
            .ok_or_else(|| Error::Unknown("Cannot extract times from range".to_string()))?;

        // Download full video to temporary file
        let temp_filename = format!("temp_full_{}.mp4", utils::fs::random_filename(8));
        let temp_path = self.download_video(video, &temp_filename).await?;

        // Extract segment using ffmpeg
        let output_str = output_path.to_str().ok_or_else(|| Error::PathValidation {
            path: output_path.to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let temp_str = temp_path.to_str().ok_or_else(|| Error::PathValidation {
            path: temp_path.clone(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

        let start_str = format!("{:.3}", start_time);
        let duration = end_time - start_time;
        let duration_str = format!("{:.3}", duration);

        let args = vec![
            "-i",
            temp_str,
            "-ss",
            &start_str,
            "-t",
            &duration_str,
            "-c",
            "copy",
            "-avoid_negative_ts",
            "1",
            output_str,
        ];

        let executor = Executor::new(
            self.libraries.ffmpeg.clone(),
            utils::to_owned(args),
            self.timeout,
        );

        executor.execute().await?;

        // Clean up temporary file
        utils::remove_temp_file(&temp_path).await;

        Ok(output_path.to_path_buf())
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
        let video_ext = format!("{:?}", video_format.download_info.ext);
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);
        let audio_ext = format!("{:?}", audio_format.download_info.ext);
        let audio_filename = format!("temp_audio_{}.{}", utils::fs::random_filename(8), audio_ext);

        // Download video and audio in parallel
        let (video_result, audio_result) = tokio::join!(
            self.download_format(video_format, &video_filename),
            self.download_format(audio_format, &audio_filename)
        );

        // Check results
        let video_temp_path = video_result?;
        let audio_temp_path = audio_result?;

        // Combine audio and video
        let output_filename = output_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("output.mp4");

        let combined_path = self
            .combine_audio_and_video(&audio_filename, &video_filename, output_filename)
            .await?;

        // If the user specified a different directory than output_dir, move the file
        if combined_path != output_path {
            utils::create_parent_dir(output_path).await?;
            if tokio::fs::rename(&combined_path, output_path)
                .await
                .is_err()
            {
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
}
