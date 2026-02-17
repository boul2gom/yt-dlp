use crate::Downloader;
use crate::client::streams::selection::VideoSelection;
use crate::download::Fetcher;
use crate::error::Error;
use crate::executor::Executor;
use crate::model::caption::Extension as CaptionExtension;
use crate::model::format::{Format, FormatType};
use crate::model::playlist::{Playlist, PlaylistDownloadProgress};
use crate::model::{AudioCodecPreference, AudioQuality, Video, VideoCodecPreference, VideoQuality};
use crate::utils;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::Arc;

impl Downloader {
    /// Helper to check if a video is in the cache by URL.
    async fn check_video_cache(&self, url: &str) -> Option<Video> {
        #[cfg(feature = "cache")]
        {
            let cache = self.cache.as_ref()?;
            cache.get(url).await.ok().flatten()
        }
        #[cfg(not(feature = "cache"))]
        None
    }

    /// Fetch the video information from the given URL.
    pub async fn fetch_video_infos(&self, url: String) -> crate::error::Result<Video> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching video infos for {}", url);

        if let Some(video) = self.check_video_cache(&url).await {
            #[cfg(feature = "tracing")]
            tracing::debug!("Cache hit for {}", url);
            return Ok(video);
        }

        let args = vec!["-j", "--flat-playlist", &url];
        let executor = Executor::new(
            self.libraries.youtube.clone(),
            utils::to_owned(args),
            self.timeout,
        );
        let output = executor.execute().await?;
        let output = output.stdout;
        let video: Video =
            serde_json::from_str(&output).map_err(|e| Error::Unknown(e.to_string()))?;

        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            let _ = cache.put(url.clone(), video.clone()).await;
        }

        Ok(video)
    }

    /// Fetch the video information from the given URL, bypassing the cache.
    /// Fetch the video information from the given URL, bypassing the cache.
    pub async fn fetch_video_infos_fresh(&self, url: &str) -> crate::error::Result<Video> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching fresh video infos for {}", url);

        let args = vec!["-j", "--flat-playlist", url];
        let executor = Executor::new(
            self.libraries.youtube.clone(),
            utils::to_owned(args),
            self.timeout,
        );
        let output = executor.execute().await?;
        let output = output.stdout;
        let video: Video =
            serde_json::from_str(&output).map_err(|e| Error::Unknown(e.to_string()))?;

        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            let _ = cache.put(url.to_string(), video.clone()).await;
        }

        Ok(video)
    }

    /// Helper to get video by ID from cache (if available)
    pub async fn get_video_by_id(&self, id: &str) -> Option<Video> {
        #[cfg(feature = "cache")]
        {
            let cache = self.cache.as_ref()?;
            let cached_video = cache.get_by_id(id).await.ok()?;
            cached_video.video().ok()
        }
        #[cfg(not(feature = "cache"))]
        None
    }

    /// Helper to execute an action with automatic URL expiry retry.
    pub async fn execute_with_retry<T, F, Fut>(
        &self,
        url: String,
        action: F,
    ) -> crate::error::Result<T>
    where
        F: Fn(Video) -> Fut + Send + Sync + Clone,
        Fut: std::future::Future<Output = crate::error::Result<T>> + Send,
    {
        // First attempt with potentially cached metadata
        let video = self.fetch_video_infos(url.clone()).await?;

        match action(video.clone()).await {
            Ok(result) => Ok(result),
            Err(Error::UrlExpired) => {
                #[cfg(feature = "tracing")]
                tracing::warn!("URL expired, refreshing metadata and retrying...");

                // Refresh metadata bypassing cache
                let video = self.fetch_video_infos_fresh(&url).await?;
                // Retry action with fresh metadata
                action(video).await
            }
            Err(e) => Err(e),
        }
    }

    /// Retrieve a video by its ID, checking the cache first if available
    /// Fetch the video from the given URL, download it (video with audio) and returns its path.
    pub async fn download_video_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display + Clone + Send + Sync + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video from URL: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move { downloader.download_video(&video, output).await }
        })
        .await
    }

    /// Fetch the video from the given URL, download it (video with audio) to a specific path.
    pub async fn download_video_from_url_to_path(
        &self,
        url: String,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video from URL to path: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move { downloader.download_video_to_path(&video, output).await }
        })
        .await
    }

    /// Fetch the video, download it (video with audio) and returns its path.
    pub async fn download_video(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_to_path(video, &output_path).await
    }

    /// Fetch the video, download it (video with audio) to a specific path.
    pub async fn download_video_to_path(
        &self,
        video: &Video,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video {}", video.title);

        let path = output.as_ref().to_path_buf();

        // Check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the video in the cache by its ID
            if let Some((_, cached_path)) = download_cache.get_by_hash(&video.id).await {
                #[cfg(feature = "tracing")]
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

        // Create temporary names for audio and video files
        let audio_name = format!("temp_audio_{}.m4a", video.id);
        let video_name = format!("temp_video_{}.mp4", video.id);

        // Download audio and video streams in parallel
        let (audio_result, video_result) = tokio::join!(
            self.download_format(best_audio, &audio_name),
            self.download_format(best_video, &video_name)
        );

        // Check the results
        let _audio_path = audio_result?;
        let _video_path = video_result?;

        // Combine audio and video streams
        let output_path = self
            .combine_audio_and_video(
                &audio_name,
                &video_name,
                path.file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("output.mp4"),
            )
            .await?;

        // If the user specified a different directory than output_dir, move the file
        if output_path != path {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            if (tokio::fs::rename(&output_path, &path).await).is_err() {
                // rename fails across filesystems, fall back to copy+delete
                tokio::fs::copy(&output_path, &path).await?;
                tokio::fs::remove_file(&output_path).await?;
            }
        }

        // Clean up temporary files
        if let Err(_e) = tokio::fs::remove_file(&_video_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary video file: {}", _e);
        }
        if let Err(_e) = tokio::fs::remove_file(&_audio_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary audio file: {}", _e);
        }

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching downloaded video with ID: {}", video.id);

            let output_str = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_string();

            if let Err(_e) = download_cache
                .put_file(&path, output_str, Some(video.id.clone()), None)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache downloaded video: {}", _e);
            }
        }

        Ok(path)
    }

    /// Fetch the video from the given URL, download it and returns its path.
    pub async fn download_video_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display + Clone + Send + Sync + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video stream from URL: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move { downloader.download_video_stream(&video, output).await }
        })
        .await
    }

    /// Fetch the video from the given URL, download the video stream to a specific path.
    pub async fn download_video_stream_from_url_to_path(
        &self,
        url: String,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video stream from URL to path: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move {
                downloader
                    .download_video_stream_to_path(&video, output)
                    .await
            }
        })
        .await
    }

    /// Download the video only, and returns its path.
    pub async fn download_video_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
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
    pub async fn download_video_stream_to_path(
        &self,
        video: &Video,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
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

    /// Fetch the audio stream from the given URL, download it and returns its path.
    pub async fn download_audio_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display + Clone + Send + Sync + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move { downloader.download_audio_stream(&video, output).await }
        })
        .await
    }

    /// Fetch the audio stream from the given URL, download it to a specific path.
    pub async fn download_audio_stream_from_url_to_path(
        &self,
        url: String,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL to path: {}", url);

        self.execute_with_retry(url, move |video| {
            let output = output.clone();
            let downloader = self.clone();
            async move {
                downloader
                    .download_audio_stream_to_path(&video, output)
                    .await
            }
        })
        .await
    }

    /// Fetch the thumbnail from the given URL and download it to the specified path.
    pub async fn download_thumbnail_from_url(
        &self,
        url: String,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone + 'static,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading thumbnail from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;

        if let Some(thumbnail_url) = &video.thumbnail {
            let fetcher =
                Fetcher::new(thumbnail_url, self.proxy.as_ref(), self.user_agent.clone())?;
            fetcher.fetch_asset(output.clone()).await?;
            Ok(output.as_ref().to_path_buf())
        } else {
            Err(Error::Unknown(
                "No thumbnail found for this video".to_string(),
            ))
        }
    }

    /// Fetch the audio stream, download it and returns its path.
    pub async fn download_audio_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_audio_stream_to_path(video, &output_path)
            .await
    }

    /// Fetch the audio stream, download it to a specific path.
    pub async fn download_audio_stream_to_path(
        &self,
        video: &Video,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
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
    pub async fn download_format(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_format_to_path(format, &output_path).await
    }

    /// Downloads a format to a specific path.
    pub async fn download_format_to_path(
        &self,
        format: &Format,
        output: impl AsRef<Path> + std::fmt::Debug,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading format {}", format.format_id);

        let output_path = output.as_ref().to_path_buf();

        // Use the internal function to download the format without preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(format, &output_path, None, None, None, None).await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Downloads a format with specific quality and codec preferences.
    pub async fn download_format_with_preferences(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());

        // Use the internal function to download the format with preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(
                    format,
                    &output_path,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
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
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        // Check if we have specific preferences
        #[cfg(feature = "cache")]
        let has_preferences = video_quality.is_some()
            || audio_quality.is_some()
            || video_codec.is_some()
            || audio_codec.is_some();

        // Check if the format is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache
            && let Some(video_id) = format.video_id.as_ref()
        {
            // First try to find by exact format ID
            if let Some((_, cached_path)) = download_cache
                .get_by_video_and_format(video_id, &format.format_id)
                .await
            {
                #[cfg(feature = "tracing")]
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
                #[cfg(feature = "tracing")]
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
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            let output_str = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_string();

            #[cfg(feature = "tracing")]
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
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Failed to cache format with preferences: {}", _e);
                }
            } else if let Err(_e) = download_cache
                .put_file(path, output_str, format.video_id.clone(), Some(format))
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(path.clone())
    }
    /// Downloads a subtitle file for a specific language.
    pub async fn download_subtitle(
        &self,
        video: &Video,
        language_code: impl AsRef<str>,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        let language_code = language_code.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading subtitle for video {} in language {}",
            video.id,
            language_code
        );

        let output_path = self.output_dir.join(output.as_ref());

        // Check if subtitle is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache
            && let Some((_, cached_path)) = download_cache
                .get_subtitle_by_language(&video.id, language_code)
                .await
        {
            #[cfg(feature = "tracing")]
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

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading subtitle from {} to {:?}",
            subtitle.url,
            output_path
        );

        // Download the subtitle file
        let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
        fetcher.fetch_asset(&output_path).await?;

        // Cache the downloaded subtitle
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
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
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache subtitle: {}", _e);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded subtitle for language {} to {:?}",
            language_code,
            output_path
        );

        Ok(output_path)
    }

    /// Downloads all available subtitles for a video.
    pub async fn download_all_subtitles(
        &self,
        video: &Video,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
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

                #[cfg(feature = "tracing")]
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

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded {} subtitle files",
            downloaded_files.len()
        );

        Ok(downloaded_files)
    }

    /// Fetches playlist information from a URL.
    pub async fn fetch_playlist_infos(
        &self,
        url: impl Into<String>,
    ) -> crate::error::Result<Playlist> {
        let url = url.into();
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching playlist information from {}", url);

        // Check if the playlist is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.playlist_cache
            && let Some(playlist) = cache.get(&url).await?
        {
            #[cfg(feature = "tracing")]
            tracing::debug!("Using cached playlist information for {}", url);
            return Ok(playlist);
        }

        // Delegate to the extractor
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Fetching playlist information using {} extractor",
            self.extractor.name()
        );

        let mut playlist = self.extractor.fetch_playlist(&url).await?;

        // Store the URL in the playlist for caching purposes
        playlist.url = Some(url.clone());

        // Cache the playlist if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.playlist_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching playlist information for {}", url);

            if let Err(_e) = cache.put(url.clone(), playlist.clone()).await {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache playlist information: {}", _e);
            }
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully fetched playlist {} with {} videos",
            playlist.id,
            playlist.entry_count()
        );

        Ok(playlist)
    }

    /// Downloads all videos from a playlist.
    pub async fn download_playlist(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
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

        for (_idx, result) in results.into_iter().enumerate() {
            match result {
                Ok(path) => downloaded_files.push(path),
                Err(e) => {
                    #[cfg(feature = "tracing")]
                    tracing::error!("Failed to download video at index {}: {}", _idx, e);
                    errors.push(e);
                }
            }
        }

        // If there were any errors, return the first one
        // (to maintain backward compatibility with the previous sequential behavior)
        if !errors.is_empty() && downloaded_files.is_empty() {
            return Err(errors.into_iter().next().unwrap());
        }

        #[cfg(feature = "tracing")]
        tracing::info!(
            "Successfully downloaded {} out of {} videos from playlist {}",
            downloaded_files.len(),
            playlist.entry_count(),
            playlist.id
        );

        Ok(downloaded_files)
    }

    /// Downloads all videos from a playlist in parallel.
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

        #[cfg(feature = "tracing")]
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
                        #[cfg(feature = "tracing")]
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
                        #[cfg(feature = "tracing")]
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

                        #[cfg(feature = "tracing")]
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

        #[cfg(feature = "tracing")]
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
    pub async fn download_playlist_items(
        &self,
        playlist: &Playlist,
        indices: &[usize],
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Downloading {} specific videos from playlist {}",
            indices.len(),
            playlist.id
        );

        let mut downloaded_files = Vec::new();

        for &index in indices {
            if let Some(entry) = playlist.get_entry_by_index(index) {
                if !entry.is_available() {
                    #[cfg(feature = "tracing")]
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

                #[cfg(feature = "tracing")]
                tracing::info!("Downloaded video at index {}: {}", index, entry.title);
            } else {
                #[cfg(feature = "tracing")]
                tracing::warn!("Index {} is out of bounds for playlist", index);
            }
        }

        Ok(downloaded_files)
    }

    /// Downloads a range of videos from a playlist.
    pub async fn download_playlist_range(
        &self,
        playlist: &Playlist,
        start: usize,
        end: usize,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        #[cfg(feature = "tracing")]
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
                #[cfg(feature = "tracing")]
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

            #[cfg(feature = "tracing")]
            tracing::info!("Downloaded video: {}", entry.title);
        }

        Ok(downloaded_files)
    }

    /// Downloads a partial range of a video using hybrid approach (yt-dlp with ffmpeg fallback).
    pub async fn download_video_partial(
        &self,
        video: &crate::model::Video,
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
                #[cfg(feature = "tracing")]
                tracing::info!("Successfully downloaded partial video using yt-dlp");
                Ok(path)
            }
            Err(_e) => {
                #[cfg(feature = "tracing")]
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
        video: &crate::model::Video,
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
        video: &crate::model::Video,
        range: &crate::download::partial::PartialRange,
        output_path: &Path,
    ) -> crate::error::Result<PathBuf> {
        // Get time range
        let (start_time, end_time) = range
            .get_times()
            .ok_or_else(|| Error::Unknown("Cannot extract times from range".to_string()))?;

        // Download full video to temporary file
        let temp_filename = format!("temp_full_{}.mp4", crate::utils::fs::random_filename(8));
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
        tokio::fs::remove_file(&temp_path).await.ok();

        Ok(output_path.to_path_buf())
    }
}
