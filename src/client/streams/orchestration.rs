use crate::client::streams::selection::VideoSelection;
use crate::download::Fetcher;
use crate::error::Error;
use crate::executor::Executor;
use crate::metadata::MetadataManager;
use crate::model::Video;
use crate::model::caption::Extension as CaptionExtension;
use crate::model::format::{Format, FormatType};
use crate::model::playlist::{Playlist, PlaylistDownloadProgress};
#[cfg(cache)]
use crate::model::selector::FormatPreferences;
use crate::model::selector::{StoryboardQuality, ThumbnailQuality};
use crate::utils;
use crate::{DownloadStatus, Downloader};

use futures_util::stream::{FuturesUnordered, StreamExt};
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
        #[cfg(cache)]
        {
            tracing::debug!(url = url, "🔍 Checking video cache");

            let cache = self.cache.as_ref()?;
            let video = cache.videos.get(url).await.ok().flatten()?;

            // If format URLs have expired according to available_at, invalidate and force re-fetch
            if !video.are_format_urls_fresh() {
                tracing::debug!(
                    url = url,
                    video_id = %video.id,
                    "🔍 Cached video has expired format URLs, invalidating"
                );
                let _ = cache.videos.remove(url).await;
                return None;
            }

            tracing::debug!(
                url = url,
                video_id = %video.id,
                "🔍 Video cache hit with fresh format URLs"
            );

            Some(video)
        }
        #[cfg(not(cache))]
        {
            tracing::debug!(url = url, "🔍 Cache feature disabled");
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
            "📡 Selecting video extractor"
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

        self.event_bus.emit_if_subscribed(event);
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

        tracing::info!(url = url_str, "📡 Fetching video information");

        if let Some(video) = self.check_video_cache(url_str).await {
            tracing::debug!(
                url = url_str,
                video_id = %video.id,
                video_title = %video.title,
                "🔍 Cache hit, returning cached video"
            );
            return Ok(video);
        }

        self.fetch_video_infos_internal(url_str, "fetching from extractor")
            .await
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

        self.fetch_video_infos_internal(
            url_str,
            "fetching fresh video information (bypassing cache)",
        )
        .await
    }

    /// Internal helper to fetch video information, emit events, and update cache.
    async fn fetch_video_infos_internal(
        &self,
        url: &str,
        log_message: &str,
    ) -> crate::error::Result<Video> {
        tracing::debug!(
            url = url,
            message = log_message,
            "📡 Fetching video information"
        );

        let start = std::time::Instant::now();
        let result = self.get_extractor(url).fetch_video(url).await;
        let duration = start.elapsed();

        let video = match result {
            Ok(v) => {
                tracing::debug!(
                    url = url,
                    video_id = %v.id,
                    video_title = %v.title,
                    format_count = v.formats.len(),
                    duration = ?duration,
                    "✅ Video information fetched"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetched {
                    url: url.to_string(),
                    video: Box::new(v.clone()),
                    duration,
                })
                .await;

                v
            }
            Err(e) => {
                tracing::debug!(
                    url = url,
                    error = %e,
                    duration = ?duration,
                    "📡 Video information fetch failed"
                );

                self.emit_event(crate::events::DownloadEvent::VideoFetchFailed {
                    url: url.to_string(),
                    error: e.to_string(),
                    duration,
                })
                .await;

                return Err(e);
            }
        };

        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(video_id = %video.id, "🔍 Updating cache with video data");

            let _ = cache.videos.put(url.to_string(), video.clone()).await;
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
        #[cfg(cache)]
        {
            tracing::debug!(video_id = id, "🔍 Getting video from cache by ID");

            let cache = self.cache.as_ref()?;
            let cached_video = cache.videos.get_by_id(id).await.ok()?;
            let video = cached_video.video().ok();

            tracing::debug!(
                video_id = id,
                found = video.is_some(),
                "🔍 Video cache lookup by ID completed"
            );

            video
        }
        #[cfg(not(cache))]
        {
            tracing::debug!(video_id = id, "🔍 Cache feature disabled");
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
                tracing::warn!("🔄 URL expired, refreshing metadata and retrying...");

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
        tracing::info!(title = video.title, "📥 Downloading video");

        let path = output.into();

        // Check if the video is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            // Try to find the video in the cache by its ID
            if let Some((_, cached_path)) = cache.downloads.get_by_hash(&video.id).await {
                tracing::debug!(video_id = video.id, "🔍 Cache hit for downloaded video");

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
    pub async fn download_video_stream(
        &self,
        video: &Video,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        tracing::debug!(title = video.title, "📥 Downloading video stream");

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
        tracing::debug!(title = video.title, "📥 Downloading video stream to path");

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

        let thumbnail =
            video
                .select_thumbnail(quality)
                .ok_or_else(|| crate::error::Error::NoThumbnail {
                    video_id: video.id.clone(),
                })?;

        let http_headers = self
            .user_agent
            .clone()
            .map(|ua| crate::model::format::HttpHeaders {
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
            Some(DownloadStatus::Canceled) => {
                Err(crate::error::Error::DownloadCancelled { download_id: id })
            }
            _ => Err(crate::error::Error::download_failed(
                id,
                "Unexpected download status",
            )),
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
        tracing::debug!(title = video.title, "📥 Downloading audio stream");

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
    async fn download_format_internal(
        &self,
        format: &Format,
        path: &PathBuf,
        #[cfg(cache)] preferences: FormatPreferences,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(cache)]
        let has_preferences = preferences.has_any();

        // Check if the format is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache
            && let Some(video_id) = format.video_id.as_ref()
        {
            // First try to find by exact format ID
            if let Some((_, cached_path)) = cache
                .downloads
                .get_by_video_and_format(video_id, &format.format_id)
                .await
            {
                tracing::debug!(format_id = format.format_id, "🔍 Using cached format");

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, path).await?;
                return Ok(path.clone());
            }

            // Then try to find by preferences if they exist
            if has_preferences
                && let Some((_, cached_path)) = cache
                    .downloads
                    .get_by_video_and_preferences(video_id, &preferences)
                    .await
            {
                tracing::debug!("🔍 Using cached format by preferences");

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
        if let Some(cache) = &self.cache {
            let output_str = utils::try_name(path.as_path()).unwrap_or_default();

            tracing::debug!(format_id = format.format_id, "🔍 Caching format");

            // Use the appropriate function depending on whether we have preferences or not
            if has_preferences {
                if let Some(video_id) = format.video_id.as_ref()
                    && let Err(_e) = cache
                        .downloads
                        .put_file_with_preferences(
                            path,
                            output_str,
                            Some(video_id.clone()),
                            Some(format),
                            &preferences,
                        )
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
        fallback_to_automatic: bool,
    ) -> crate::error::Result<PathBuf> {
        let language_code = language_code.as_ref();

        tracing::debug!(
            video_id = video.id,
            language = language_code,
            "💬 Downloading subtitle"
        );

        let output_path = self.output_dir.join(output.as_ref());

        // Check if subtitle is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache
            && let Some((_, cached_path)) = cache
                .downloads
                .get_subtitle_by_language(&video.id, language_code)
                .await
        {
            tracing::debug!(
                video_id = video.id,
                language = language_code,
                "🔍 Using cached subtitle"
            );

            // Copy the file from the cache to the output directory
            tokio::fs::copy(&cached_path, &output_path).await?;
            return Ok(output_path);
        }

        // Resolve subtitles for the language: prefer user-uploaded subtitles,
        // then fall back to automatic captions (e.g. YouTube auto-generated).
        let owned_fallback: Vec<crate::model::caption::Subtitle>;
        let subtitles: &[crate::model::caption::Subtitle] =
            if let Some(subs) = video.subtitles.get(language_code) {
                subs.as_slice()
            } else if fallback_to_automatic {
                if let Some(captions) = video.automatic_captions.get(language_code) {
                    owned_fallback = captions
                        .iter()
                        .map(|c| {
                            crate::model::caption::Subtitle::from_automatic_caption(
                                c,
                                language_code.to_string(),
                            )
                        })
                        .collect();
                    owned_fallback.as_slice()
                } else {
                    return Err(Error::SubtitleNotAvailable {
                        video_id: video.id.clone(),
                        language: language_code.to_string(),
                    });
                }
            } else {
                return Err(Error::SubtitleNotAvailable {
                    video_id: video.id.clone(),
                    language: language_code.to_string(),
                });
            };

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

        tracing::debug!(url = subtitle.url, path = ?output_path, "💬 Downloading subtitle file");

        // Download the subtitle file
        let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
        fetcher.fetch_asset(&output_path).await?;

        // Cache the downloaded subtitle
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(
                video_id = video.id,
                language = language_code,
                "🔍 Caching subtitle"
            );

            if let Err(_e) = cache
                .downloads
                .put_subtitle_file(
                    &output_path,
                    output.as_ref(),
                    video.id.clone(),
                    language_code.to_string(),
                )
                .await
            {
                tracing::warn!(error = %_e, "Failed to cache subtitle");
            }
        }

        tracing::info!(language = language_code, path = ?output_path, "✅ Subtitle downloaded");

        Ok(output_path)
    }

    /// Downloads all available subtitles and automatic captions for a video.
    ///
    /// Iterates over user-uploaded subtitles first, then merges automatic captions
    /// for any language not already covered. Prefers SRT, then VTT, then any format.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output_dir` - The directory to save the subtitle files to.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded subtitle files.
    pub async fn download_all_subtitles(
        &self,
        video: &Video,
        output_dir: impl AsRef<Path>,
        fallback_to_automatic: bool,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            video_id = %video.id,
            subtitle_langs = video.subtitles.len(),
            caption_langs = video.automatic_captions.len(),
            "💬 Downloading all subtitles and automatic captions"
        );

        let output_dir = output_dir.as_ref();
        let mut downloaded_files = Vec::new();

        // Merge language sources: manual subtitles take priority over automatic captions
        let mut all_languages: std::collections::HashMap<
            &str,
            Vec<crate::model::caption::Subtitle>,
        > = std::collections::HashMap::new();

        if fallback_to_automatic {
            for (lang, captions) in &video.automatic_captions {
                let subs: Vec<_> = captions
                    .iter()
                    .map(|c| {
                        crate::model::caption::Subtitle::from_automatic_caption(c, lang.clone())
                    })
                    .collect();
                all_languages.entry(lang.as_str()).or_insert(subs);
            }
        }
        for (lang, subs) in &video.subtitles {
            // Manual subtitles override automatic captions for the same language
            all_languages.insert(lang.as_str(), subs.clone());
        }

        for (language_code, subtitles) in &all_languages {
            // Prefer SRT → VTT → first available
            let Some(subtitle) = subtitles
                .iter()
                .find(|s| s.is_format(&CaptionExtension::Srt))
                .or_else(|| {
                    subtitles
                        .iter()
                        .find(|s| s.is_format(&CaptionExtension::Vtt))
                })
                .or_else(|| subtitles.first())
            else {
                continue;
            };

            let filename = format!(
                "{}.{}.{}",
                video.id,
                language_code,
                subtitle.file_extension()
            );
            let output_path = output_dir.join(&filename);

            tracing::debug!(
                video_id = %video.id,
                language_code = language_code,
                url = %subtitle.url,
                "💬 Downloading subtitle/caption"
            );

            let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
            fetcher.fetch_asset(&output_path).await?;
            downloaded_files.push(output_path);
        }

        tracing::info!(
            video_id = %video.id,
            count = downloaded_files.len(),
            "✅ Subtitle/caption files downloaded"
        );

        Ok(downloaded_files)
    }

    /// Downloads all MHTML fragments of a storyboard format.
    ///
    /// Each fragment is a grid of preview images for a contiguous time range.
    /// Files are named `{video_id}_sb_{format_id}_{index}.mhtml` where `video_id` comes
    /// from `format.video_id` when set (populated automatically when fetching via this library).
    ///
    /// # Arguments
    ///
    /// * `format` - A storyboard `Format` obtained via [`VideoSelection::best_storyboard_format`].
    /// * `output_dir` - Directory where fragment files will be written.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded MHTML fragment files.
    ///
    /// # Errors
    ///
    /// Returns an error if the format is not a storyboard, if fragments are missing, or if any
    /// fragment download fails.
    pub async fn download_storyboard_format(
        &self,
        format: &Format,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        let output_dir = output_dir.as_ref();

        if format.format_type() != FormatType::Storyboard {
            return Err(Error::FormatNotAvailable {
                video_id: format.video_id.clone().unwrap_or_default(),
                format_type: FormatType::Storyboard,
                available_formats: vec![format.format_id.clone()],
            });
        }

        let fragments = format
            .storyboard_info
            .fragments
            .as_deref()
            .unwrap_or_default();

        // Use video_id when available (set by the library), fall back to format_id
        let prefix = format
            .video_id
            .as_deref()
            .unwrap_or(format.format_id.as_str());

        tracing::debug!(
            video_id = prefix,
            format_id = %format.format_id,
            fragment_count = fragments.len(),
            resolution = ?format.video_resolution.resolution,
            "🖼️ Downloading storyboard fragments"
        );

        let mut paths = Vec::with_capacity(fragments.len());
        let mut download_ids = Vec::with_capacity(fragments.len());

        for (index, fragment) in fragments.iter().enumerate() {
            let filename = format!("{}_sb_{}_{:04}.mhtml", prefix, format.format_id, index);
            let output_path = output_dir.join(&filename);

            tracing::debug!(
                index = index,
                url = %fragment.url,
                path = ?output_path,
                "🖼️ Enqueuing storyboard fragment for download"
            );

            let id = self
                .download_manager
                .enqueue(
                    &fragment.url,
                    output_path.clone(),
                    Some(crate::download::DownloadPriority::Normal),
                )
                .await;

            paths.push(output_path);
            download_ids.push(id);
        }

        for id in download_ids {
            match self.wait_for_download(id).await {
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

        tracing::info!(
            video_id = prefix,
            format_id = %format.format_id,
            downloaded = paths.len(),
            "✅ Storyboard fragments downloaded"
        );

        Ok(paths)
    }

    /// Downloads the storyboard of the requested quality for a video.
    ///
    /// Selects the best or worst storyboard format via [`VideoSelection`] and delegates
    /// to [`Downloader::download_storyboard_format`].
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `quality` - [`StoryboardQuality::Best`] for highest resolution, [`StoryboardQuality::Worst`] for lowest.
    /// * `output_dir` - Directory where fragment files will be written.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded MHTML fragment files.
    ///
    /// # Errors
    ///
    /// Returns an error if no storyboard formats are available or if a download fails.
    pub async fn download_storyboard(
        &self,
        video: &Video,
        quality: crate::model::selector::StoryboardQuality,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            video_id = %video.id,
            quality = ?quality,
            "🖼️ Selecting storyboard format for download"
        );

        let format = match quality {
            StoryboardQuality::Best => video.best_storyboard_format(),
            StoryboardQuality::Worst => video.worst_storyboard_format(),
        }
        .ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Storyboard,
            available_formats: vec![],
        })?;

        self.download_storyboard_format(format, output_dir).await
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
        tracing::info!(url = url_str, "📋 Fetching playlist information");

        // Check if the playlist is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache
            && let Some(playlist) = cache.playlists.get(url_str).await?
        {
            tracing::debug!(url = url_str, "🔍 Using cached playlist information");
            return Ok(playlist);
        }

        // Delegate to the extractor
        let extractor = self.get_extractor(url_str);
        tracing::debug!(extractor = %extractor.name(), "📡 Fetching playlist information from extractor");

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
                    "✅ Playlist information fetched"
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
                    "📋 Playlist information fetch failed"
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
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(url = url_str, "🔍 Caching playlist information");

            if let Err(_e) = cache
                .playlists
                .put(url_str.to_string(), playlist.clone())
                .await
            {
                tracing::warn!(error = %_e, "Failed to cache playlist information");
            }
        }

        tracing::info!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            "✅ Playlist fetched"
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
        tracing::info!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            "📋 Downloading playlist"
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
                    tracing::error!(index = video_idx, error = %e, "Failed to download video");
                    errors.push(e);
                }
            }
        }

        // If there were any errors, return the first one
        // (to maintain backward compatibility with the previous sequential behavior)
        if !errors.is_empty()
            && downloaded_files.is_empty()
            && let Some(e) = errors.into_iter().next()
        {
            return Err(e);
        }

        tracing::info!(
            downloaded = downloaded_files.len(),
            total = playlist.entry_count(),
            playlist_id = playlist.id,
            "✅ Playlist download completed"
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
        tracing::debug!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            max_concurrent = max_concurrent.unwrap_or(3),
            "📋 Downloading playlist in parallel"
        );

        let max_concurrent = max_concurrent.unwrap_or(3);
        let total_videos = playlist.entry_count();
        let mut completed = 0usize;
        let mut results = Vec::new();
        let mut tasks = FuturesUnordered::new();
        let mut entry_iter = playlist.entries.iter().peekable();

        let output_pattern = output_pattern.as_ref().to_string();
        let progress_callback = progress_callback.map(Arc::new);
        let playlist_start = std::time::Instant::now();

        loop {
            // Spawn tasks up to max_concurrent limit
            while tasks.len() < max_concurrent {
                if let Some(entry) = entry_iter.next() {
                    if !entry.is_available() {
                        tracing::warn!(
                            title = entry.title,
                            id = entry.id,
                            "📋 Skipping unavailable video"
                        );

                        let entry_clone = entry.clone();
                        completed += 1;

                        self.emit_event(crate::events::DownloadEvent::PlaylistItemFailed {
                            playlist_id: playlist.id.clone(),
                            index: entry.index.unwrap_or(0),
                            total: total_videos,
                            video_id: entry.id.clone(),
                            error: format!("Video {} is not available", entry.id),
                        })
                        .await;

                        // Call progress callback for unavailable video
                        if let Some(callback) = &progress_callback {
                            callback(PlaylistDownloadProgress {
                                entry: entry_clone.clone(),
                                result: Err(format!("Video {} is not available", entry_clone.id)),
                                completed,
                                total: total_videos,
                            });
                        }

                        results.push(Err(Error::video_fetch(
                            &entry.url,
                            format!("Video {} is not available", entry.id),
                        )));
                        continue;
                    }

                    let entry = entry.clone();
                    let output_pattern = output_pattern.clone();
                    let youtube = self.clone();
                    let _callback = progress_callback.clone();
                    let playlist_id = playlist.id.clone();

                    self.emit_event(crate::events::DownloadEvent::PlaylistItemStarted {
                        playlist_id: playlist_id.clone(),
                        index: entry.index.unwrap_or(0),
                        total: total_videos,
                        video_id: entry.id.clone(),
                    })
                    .await;

                    let task = tokio::spawn(async move {
                        tracing::debug!(
                            video_id = entry.id,
                            index = entry.index.unwrap_or(0),
                            "📥 Downloading video from playlist"
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
                                title = entry.title,
                                index = entry.index.unwrap_or(0),
                                "✅ Downloaded video from playlist"
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
                        match &download_result {
                            Ok(path) => {
                                self.emit_event(
                                    crate::events::DownloadEvent::PlaylistItemCompleted {
                                        playlist_id: playlist.id.clone(),
                                        index: entry.index.unwrap_or(0),
                                        total: total_videos,
                                        video_id: entry.id.clone(),
                                        output_path: path.clone(),
                                    },
                                )
                                .await;
                            }
                            Err(e) => {
                                self.emit_event(crate::events::DownloadEvent::PlaylistItemFailed {
                                    playlist_id: playlist.id.clone(),
                                    index: entry.index.unwrap_or(0),
                                    total: total_videos,
                                    video_id: entry.id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                            }
                        }

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
                        results.push(Err(Error::runtime("playlist download task", e)));
                    }
                }
            }
        }

        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - successful;
        let playlist_duration = playlist_start.elapsed();

        self.emit_event(crate::events::DownloadEvent::PlaylistCompleted {
            playlist_id: playlist.id.clone(),
            total_items: total_videos,
            successful,
            failed,
            duration: playlist_duration,
        })
        .await;

        tracing::info!(
            successful = successful,
            total = playlist.entry_count(),
            playlist_id = playlist.id,
            "✅ Parallel playlist download completed"
        );

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
            count = indices.len(),
            playlist_id = playlist.id,
            "📋 Downloading specific videos from playlist"
        );

        let mut downloaded_files = Vec::new();

        for &index in indices {
            if let Some(entry) = playlist.get_entry_by_index(index) {
                if !entry.is_available() {
                    tracing::warn!(
                        index = index,
                        title = entry.title,
                        "📋 Skipping unavailable video"
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

                tracing::info!(index = index, title = entry.title, "✅ Downloaded video");
            } else {
                tracing::warn!(index = index, "Index out of bounds for playlist");
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
            start = start,
            end = end,
            playlist_id = playlist.id,
            "📋 Downloading playlist range"
        );

        let entries = playlist.get_entries_in_range(start, end);
        let mut downloaded_files = Vec::new();

        for entry in entries {
            if !entry.is_available() {
                tracing::warn!(
                    title = entry.title,
                    id = entry.id,
                    "📋 Skipping unavailable video"
                );
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

            tracing::info!(title = entry.title, "✅ Downloaded video");
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
                    .ok_or_else(|| Error::VideoMissingField {
                        video_id: video.id.clone(),
                        field: "chapter at requested index".to_string(),
                    })?
            } else {
                return Err(Error::VideoMissingField {
                    video_id: video.id.clone(),
                    field: "chapters".to_string(),
                });
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
                tracing::info!("✅ Partial video downloaded via yt-dlp");
                Ok(path)
            }
            Err(_e) => {
                tracing::warn!(error = %_e, "🔄 yt-dlp partial download failed, trying ffmpeg fallback");

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
        let (start_time, end_time) = range.get_times().ok_or_else(|| {
            Error::Unknown("Cannot extract time boundaries from partial range".to_string())
        })?;

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

        let args = crate::executor::FfmpegArgs::new()
            .input(temp_str)
            .args(["-ss", &start_str, "-t", &duration_str])
            .codec_copy()
            .args(["-avoid_negative_ts", "1"])
            .output(output_str)
            .build();

        let executor = Executor::new(self.libraries.ffmpeg.clone(), args, self.timeout);

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
        let video_ext = video_format.download_info.ext.as_str();
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);
        let audio_ext = audio_format.download_info.ext.as_str();
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
        let metadata_file = tokio::task::spawn_blocking(move || {
            MetadataManager::create_combined_metadata_file(&video_clone)
        })
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
}
