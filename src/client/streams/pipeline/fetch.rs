use std::future::Future;

use crate::Downloader;
use crate::error::Error;
use crate::model::Video;

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
    pub(crate) async fn check_video_cache(&self, url: &str) -> Option<Video> {
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
    pub(crate) fn get_extractor(&self, url: &str) -> &dyn crate::extractor::VideoExtractor {
        let is_youtube = crate::extractor::Youtube::supports_url(url);

        tracing::debug!(url = url, is_youtube = is_youtube, "📡 Selecting video extractor");

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
    pub async fn fetch_video_infos_fresh(&self, url: impl AsRef<str>) -> crate::error::Result<Video> {
        let url_str = url.as_ref();

        self.fetch_video_infos_internal(url_str, "fetching fresh video information (bypassing cache)")
            .await
    }

    /// Internal helper to fetch video information, emit events, and update cache.
    async fn fetch_video_infos_internal(&self, url: &str, log_message: &str) -> crate::error::Result<Video> {
        tracing::debug!(url = url, message = log_message, "📡 Fetching video information");

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
    pub async fn execute_with_retry<T, F, Fut>(&self, url: String, action: F) -> crate::error::Result<T>
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
}
