use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use crate::download::config::postprocess::PostProcessConfig;
use crate::error::Result;
use crate::model::Video;
use crate::{Downloader, download, events, metadata};

impl Downloader {
    /// Fluent method to fetch video info and return self for chaining.
    ///
    /// This is useful for building operation pipelines.
    ///
    /// # Arguments
    ///
    /// * `url` - The YouTube video URL
    ///
    /// # Returns
    ///
    /// A tuple of (self, video) for method chaining
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::path::PathBuf;
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let (downloader, video) = Downloader::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .fetch("https://youtube.com/watch?v=gXtp6C-3JKo")
    ///     .await?;
    ///
    /// println!("Title: {}", video.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch(self, url: impl AsRef<str>) -> Result<(Self, Video)> {
        let url_str = url.as_ref();

        tracing::debug!(url = url_str, "📡 Fetching video info (fluent API)");

        let video = self.fetch_video_infos(url_str.to_string()).await?;

        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            "✅ Video info fetched (fluent API)"
        );

        Ok((self, video))
    }

    /// Fluent method to download a video and return self for chaining.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download
    /// * `output` - The output filename
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::Video;
    /// # use yt_dlp::download::PostProcessConfig;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use std::path::PathBuf;
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let (downloader, video) = Downloader::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .fetch("https://youtube.com/watch?v=gXtp6C-3JKo")
    ///     .await?;
    ///
    /// // Download the video, then enrich it with metadata
    /// let downloader = downloader
    ///     .download_and_continue(&video, "output.mp4")
    ///     .await?
    ///     .postprocess_video(
    ///         "output.mp4",
    ///         "output_processed.mp4",
    ///         PostProcessConfig::new(),
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_and_continue(self, video: &Video, output: impl AsRef<str>) -> Result<Self> {
        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            output = output.as_ref(),
            "📥 Downloading video (fluent API)"
        );

        self.download_video(video, output).await?;

        tracing::debug!(video_id = video.id, "✅ Video downloaded (fluent API)");

        Ok(self)
    }

    /// Fluent method to download a video to a specific path and return self for chaining.
    ///
    /// Unlike [`download_and_continue`](Self::download_and_continue), this method writes
    /// the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download
    /// * `output` - The full output path
    pub async fn download_and_continue_to_path(self, video: &Video, output: impl Into<PathBuf>) -> Result<Self> {
        let output_path = output.into();

        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            output = ?output_path,
            "📥 Downloading video to path (fluent API)"
        );

        self.download_video_to_path(video, output_path).await?;

        tracing::debug!(video_id = video.id, "✅ Video downloaded to path (fluent API)");

        Ok(self)
    }

    /// Chain multiple operations in a pipeline.
    ///
    /// This method allows you to chain fetch -> download -> metadata operations.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use std::path::PathBuf;
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// Downloader::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .pipeline(
    ///         "https://youtube.com/watch?v=gXtp6C-3JKo",
    ///         |yt, video| async move {
    ///             yt.download_video(&video, "video.mp4").await?;
    ///             Ok(yt)
    ///         },
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn pipeline<F, Fut>(self, url: impl AsRef<str>, operation: F) -> Result<Self>
    where
        F: FnOnce(Self, Video) -> Fut,
        Fut: Future<Output = Result<Self>>,
    {
        let url_str = url.as_ref();

        tracing::info!(url = url_str, "📥 Starting download pipeline");

        let video = self.fetch_video_infos(url_str).await?;

        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            "📡 Video fetched, executing pipeline operation"
        );

        let result = operation(self, video).await?;

        tracing::info!("✅ Pipeline completed");

        Ok(result)
    }

    /// Applies post-processing to a video file using FFmpeg.
    ///
    /// This method allows you to apply various post-processing operations such as:
    /// - Codec conversion (H.264, H.265, VP9, AV1)
    /// - Bitrate adjustment
    /// - Resolution scaling
    /// - Video filters (crop, rotate, brightness, contrast, etc.)
    ///
    /// # Arguments
    ///
    /// * `input_path` - Path to the input video file
    /// * `output` - The output filename
    /// * `config` - Post-processing configuration
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg execution fails
    ///
    /// # Returns
    ///
    /// The path to the processed video file
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::download::{PostProcessConfig, VideoCodec, AudioCodec, Resolution};
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let config = PostProcessConfig::new()
    ///     .with_video_codec(VideoCodec::H264)
    ///     .with_audio_codec(AudioCodec::AAC)
    ///     .with_video_bitrate("2M")
    ///     .with_resolution(Resolution::HD);
    ///
    /// let processed = downloader.postprocess_video("input.mp4", "output.mp4", config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn postprocess_video(
        &self,
        input_path: impl Into<PathBuf>,
        output: impl AsRef<str>,
        config: download::config::postprocess::PostProcessConfig,
    ) -> Result<PathBuf> {
        let input = input_path.into();
        let output_path = self.output_dir.join(output.as_ref());

        tracing::debug!(
            input = ?input,
            output = ?output_path,
            video_codec = ?config.video_codec,
            audio_codec = ?config.audio_codec,
            "✂️ Applying post-processing to video"
        );

        self.postprocess_video_to_path(input, output_path, config).await
    }

    /// Applies post-processing to a video file, saving to a specific path.
    ///
    /// Unlike [`postprocess_video`](Self::postprocess_video), this method writes the file
    /// to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `input_path` - Path to the input video file
    /// * `output` - The full path for the processed output file
    /// * `config` - Post-processing configuration
    pub async fn postprocess_video_to_path(
        &self,
        input_path: impl Into<PathBuf>,
        output: impl Into<PathBuf>,
        config: PostProcessConfig,
    ) -> Result<PathBuf> {
        let input_path = input_path.into();
        let output_path = output.into();

        tracing::debug!(
            input = ?input_path,
            output = ?output_path,
            video_codec = ?config.video_codec,
            audio_codec = ?config.audio_codec,
            video_bitrate = ?config.video_bitrate,
            audio_bitrate = ?config.audio_bitrate,
            resolution = ?config.resolution,
            filters_count = config.filters.len(),
            "✂️ Applying post-processing to video file"
        );

        let result =
            metadata::postprocess::apply_postprocess(input_path, output_path, &config, &self.libraries, self.timeout)
                .await?;

        tracing::info!(
            output = ?result,
            "✅ Post-processing completed"
        );

        Ok(result)
    }

    /// Returns a stream of all download events.
    ///
    /// This method creates a new subscriber to the event bus and returns
    /// a stream that can be used to receive all future events.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use tokio_stream::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use std::path::PathBuf;
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let downloader = Downloader::builder(libs, "output").build().await?;
    /// let mut stream = downloader.event_stream();
    ///
    /// while let Some(Ok(event)) = stream.next().await {
    ///     println!("Event: {}", event.event_type());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn event_stream(
        &self,
    ) -> impl tokio_stream::Stream<
        Item = std::result::Result<
            Arc<events::DownloadEvent>,
            tokio_stream::wrappers::errors::BroadcastStreamRecvError,
        >,
    > {
        tracing::debug!(
            subscriber_count = self.event_bus.subscriber_count(),
            "🔔 Creating event stream"
        );

        self.event_bus.stream()
    }

    /// Subscribes to download events.
    ///
    /// Returns a broadcast receiver that can be used to receive events.
    ///
    /// # Returns
    ///
    /// A broadcast receiver for download events
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<Arc<events::DownloadEvent>> {
        tracing::debug!(
            subscriber_count = self.event_bus.subscriber_count(),
            "🔔 Creating event subscription"
        );

        let receiver = self.event_bus.subscribe();

        tracing::debug!(
            subscriber_count = self.event_bus.subscriber_count(),
            "🔔 Event subscription created"
        );

        receiver
    }

    /// Returns the number of active event subscribers.
    ///
    /// # Returns
    ///
    /// The number of currently active event subscribers.
    pub fn event_subscriber_count(&self) -> usize {
        self.event_bus.subscriber_count()
    }

    #[cfg(feature = "statistics")]
    /// Returns a reference to the statistics tracker.
    ///
    /// Call [`stats::StatisticsTracker::snapshot`] to obtain aggregate metrics for all
    /// downloads and metadata fetches that have occurred since the tracker was created.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let downloader = Downloader::builder(libs, "output").build().await?;
    ///
    /// // ... perform downloads ...
    ///
    /// let snapshot = downloader.statistics().snapshot().await;
    /// println!("Completed: {}", snapshot.downloads.completed);
    /// # Ok(())
    /// # }
    /// ```
    pub fn statistics(&self) -> &crate::stats::StatisticsTracker {
        &self.statistics
    }

    #[cfg(feature = "hooks")]
    /// Registers a Rust hook for download events.
    ///
    /// Hooks are called asynchronously for each event and can be filtered
    /// to only receive specific event types.
    ///
    /// # Arguments
    ///
    /// * `hook` - The hook to register
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "hooks")]
    /// # {
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::events::{EventHook, EventFilter, DownloadEvent, HookResult};
    /// # use async_trait::async_trait;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// # let mut downloader = Downloader::builder(libs, "output").build().await?;
    /// #[derive(Clone)]
    /// struct MyHook;
    ///
    /// #[async_trait]
    /// impl EventHook for MyHook {
    ///     async fn on_event(&self, event: &DownloadEvent) -> HookResult {
    ///         println!("Event: {}", event.event_type());
    ///         Ok(())
    ///     }
    ///
    ///     fn filter(&self) -> EventFilter {
    ///         EventFilter::only_terminal()
    ///     }
    /// }
    ///
    /// downloader.register_hook(MyHook).await;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    pub async fn register_hook(&mut self, hook: impl events::EventHook + 'static) {
        tracing::debug!(has_registry = self.hook_registry.is_some(), "🔔 Registering event hook");

        if let Some(ref mut registry) = self.hook_registry {
            registry.register(hook).await;

            tracing::debug!("✅ Event hook registered");
        } else {
            tracing::warn!("Hook registry not available, hook not registered");
        }
    }

    #[cfg(feature = "webhooks")]
    /// Registers a webhook for download events.
    ///
    /// Webhooks are called via HTTP POST with a JSON payload containing the event.
    ///
    /// # Arguments
    ///
    /// * `config` - The webhook configuration
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(feature = "webhooks")]
    /// # {
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::events::{WebhookConfig, WebhookMethod, EventFilter};
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// # let mut downloader = Downloader::builder(libs, "output").build().await?;
    /// let webhook = WebhookConfig::new("https://example.com/webhook")
    ///     .with_method(WebhookMethod::Post)
    ///     .with_filter(EventFilter::only_completed());
    ///
    /// downloader.register_webhook(webhook).await;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    pub async fn register_webhook(&mut self, config: events::WebhookConfig) {
        tracing::debug!(
            url = config.url(),
            has_delivery = self.webhook_delivery.is_some(),
            "🔔 Registering webhook"
        );

        if let Some(ref mut delivery) = self.webhook_delivery {
            delivery.register(config).await;

            tracing::debug!("✅ Webhook registered");
        } else {
            tracing::warn!("Webhook delivery not available, webhook not registered");
        }
    }
}
