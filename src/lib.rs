#![doc = include_str!("../README.md")]

use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(cache)]
use cache::{CacheConfig, CacheLayer};

use crate::client::deps::{Libraries, LibraryInstaller};
use crate::download::manager::ManagerConfig;
use crate::error::Result;
use crate::executor::Executor;
use crate::extractor::{ExtractorConfig, ExtractorName};

// Core modules
#[cfg(cache)]
pub mod cache;
pub mod error;
pub mod executor;
pub mod metadata;
pub use metadata::PlaylistMetadata;
pub mod model;
pub mod utils;

// Architecture modules
pub mod client;
pub mod download;

// Multi-extractor support
pub mod extractor;

// Event system
pub mod events;

// Re-export async_trait so the simple_hook! macro works from downstream crates
#[cfg(feature = "hooks")]
#[doc(hidden)]
pub use async_trait;

// Statistics and analytics
#[cfg(feature = "statistics")]
pub mod stats;

// Live stream recording and streaming
#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
pub mod live;

// Convenience modules
pub mod macros;
pub mod prelude;

// Re-export of common traits to facilitate their use
pub use client::streams::selection::VideoSelection;
// Re-export main types for easy access
pub use client::{DownloadBuilder, DownloaderBuilder};
pub use download::{DownloadManager, DownloadPriority, DownloadStatus};
pub use model::utils::{AllTraits, CommonTraits};

use crate::model::Video;

/// Universal video downloader supporting 1,800+ sites via yt-dlp.
///
/// This struct provides a unified interface for downloading videos from any site
/// supported by yt-dlp, with automatic extractor detection and platform-specific
/// optimizations for YouTube.
///
/// # Architecture
///
/// The `Downloader` uses a trait-based extractor system:
/// - **YouTube URLs**: Uses the highly optimized `Youtube` extractor with platform-specific features
/// - **Other URLs**: Uses the `Generic` extractor for universal support
///
/// Extractor selection is automatic based on URL patterns.
///
/// # Examples
///
/// ## YouTube (with optimizations)
/// ```rust, no_run
/// # use yt_dlp::Downloader;
/// # use std::path::PathBuf;
/// # use yt_dlp::client::deps::Libraries;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// let downloader = Downloader::builder(libraries, "output").build().await?;
///
/// // YouTube is automatically detected and optimized
/// let video = downloader
///     .fetch_video_infos("https://youtube.com/watch?v=...".to_string())
///     .await?;
/// downloader.download_video(&video, "video.mp4").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Fluent Download API (Recommended)
/// ```rust, no_run
/// # use yt_dlp::Downloader;
/// # use std::path::PathBuf;
/// # use yt_dlp::client::deps::Libraries;
/// # use yt_dlp::model::selector::{VideoQuality, AudioQuality, VideoCodecPreference};
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// # let downloader = Downloader::builder(libraries, "output").build().await?;
/// let url = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
/// let video = downloader.fetch_video_infos(url).await?;
///
/// // Configure download with specific preferences
/// downloader.download(&video, "video.mp4")
///     .video_quality(VideoQuality::Best)
///     .audio_quality(AudioQuality::Best)
///     .video_codec(VideoCodecPreference::AVC1)
///     .execute()
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Other sites (Vimeo, TikTok, etc.)
/// ```rust, no_run
/// # use yt_dlp::Downloader;
/// # use std::path::PathBuf;
/// # use yt_dlp::client::deps::Libraries;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// # let downloader = Downloader::builder(libraries, "output")
/// #   .build()
/// #   .await?;
/// // Vimeo - automatically detected
/// let vimeo = downloader.fetch_video_infos("https://vimeo.com/123456".to_string()).await?;
///
/// // TikTok - automatically detected
/// let tiktok = downloader.fetch_video_infos("https://tiktok.com/@user/video/123".to_string()).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Accessing YouTube-specific features
/// ```rust, no_run
/// # use yt_dlp::Downloader;
/// # use std::path::PathBuf;
/// # use yt_dlp::client::deps::Libraries;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// # let downloader = Downloader::builder(libraries, "output")
/// #   .build()
/// #   .await?;
/// // Access YouTube-specific methods
/// let youtube = downloader.youtube_extractor();
/// let channel = youtube.fetch_channel("UC...").await?;
/// let search = youtube.search("rust programming", 10).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Downloader {
    /// The YouTube extractor for optimized YouTube support
    pub(crate) youtube_extractor: extractor::Youtube,
    /// The Generic extractor for all other sites
    pub(crate) generic_extractor: extractor::Generic,
    /// The required libraries.
    pub(crate) libraries: Libraries,

    /// The directory where the video (or formats) will be downloaded.
    pub(crate) output_dir: PathBuf,
    /// The arguments to pass to 'yt-dlp'.
    pub(crate) args: Vec<String>,
    /// The requests user agent
    pub(crate) user_agent: Option<String>,
    /// The timeout for command execution.
    pub(crate) timeout: Duration,
    /// Optional proxy configuration for HTTP requests and yt-dlp.
    pub(crate) proxy: Option<client::proxy::ProxyConfig>,
    /// The unified cache layer (videos, downloads, playlists).
    #[cfg(cache)]
    pub(crate) cache: Option<Arc<CacheLayer>>,
    /// The download manager for managing parallel downloads.
    pub(crate) download_manager: Arc<DownloadManager>,
    /// Cancellation token for graceful shutdown.
    pub(crate) cancellation_token: tokio_util::sync::CancellationToken,
    /// Event bus for broadcasting download events.
    pub(crate) event_bus: events::EventBus,
    /// Hook registry for Rust hooks (feature: hooks).
    #[cfg(feature = "hooks")]
    pub(crate) hook_registry: Option<events::HookRegistry>,
    /// Webhook delivery system (feature: webhooks).
    #[cfg(feature = "webhooks")]
    pub(crate) webhook_delivery: Option<events::WebhookDelivery>,
    /// Statistics tracker (feature: statistics).
    #[cfg(feature = "statistics")]
    pub(crate) statistics: Arc<stats::StatisticsTracker>,
}

impl Downloader {
    /// Creates a new builder for constructing a Downloader instance with a fluent API.
    ///
    /// This is the recommended way to create a Downloader instance as it provides
    /// a clean and intuitive interface for configuration.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries (yt-dlp and ffmpeg paths)
    /// * `output_dir` - The directory where videos will be downloaded
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    ///
    /// let downloader = Downloader::builder(libraries, "output")
    ///     .with_timeout(std::time::Duration::from_secs(120))
    ///     .with_max_concurrent_downloads(4)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(libraries: Libraries, output_dir: impl Into<PathBuf>) -> DownloaderBuilder {
        DownloaderBuilder::new(libraries, output_dir)
    }

    /// Creates a new YouTube fetcher with a custom download manager configuration.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries.
    /// * `output_dir` - The directory where the video will be downloaded.
    /// * `download_manager_config` - The configuration for the download manager.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parent directories of the executables and output directory could not be created.
    pub fn with_download_manager_config(
        libraries: Libraries,
        output_dir: impl Into<PathBuf>,
        download_manager_config: ManagerConfig,
    ) -> DownloaderBuilder {
        Self::builder(libraries, output_dir).with_download_manager_config(download_manager_config)
    }

    /// Creates a new download builder for downloading a video with custom quality and codec preferences.
    ///
    /// This provides a fluent API for configuring and executing downloads with
    /// custom quality, codec preferences, priority, and progress tracking.
    ///
    /// # Arguments
    ///
    /// * `url` - The YouTube video URL to download
    /// * `output` - The output filename for the downloaded video
    ///
    /// # Returns
    ///
    /// A `DownloadBuilder` instance that can be configured with various options
    /// before calling `execute()` to start the download.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::selector::{VideoQuality, AudioQuality, VideoCodecPreference};
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// let downloader = Downloader::builder(libraries, "output")
    ///     .build()
    ///     .await?;
    /// // Fetch metadata first
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// // Download a 1080p video with H264 codec
    /// let video_path = downloader.download(&video, "my-video.mp4")
    ///     .video_quality(VideoQuality::CustomHeight(1080))
    ///     .video_codec(VideoCodecPreference::AVC1)
    ///     .audio_quality(AudioQuality::Best)
    ///     .execute()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn download<'a>(&'a self, video: &'a Video, output: impl Into<PathBuf>) -> DownloadBuilder<'a> {
        DownloadBuilder::new(self, video, output)
    }

    /// Creates a live recording builder for recording a live stream.
    ///
    /// The video must be currently live (`is_currently_live() == true`).
    /// Uses the reqwest engine by default; switch to FFmpeg via `.with_method()`.
    ///
    /// # Arguments
    ///
    /// * `video` - The live stream video metadata.
    /// * `output` - The output filename for the recording.
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;
    ///
    /// let result = downloader.record_live(&video, "live.ts")
    ///     .with_max_duration(Duration::from_secs(3600))
    ///     .execute()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "live-recording")]
    pub fn record_live<'a>(&'a self, video: &'a Video, output: impl Into<PathBuf>) -> live::LiveRecordingBuilder<'a> {
        live::LiveRecordingBuilder::new(self, video, output)
    }

    /// Creates a live stream builder for streaming HLS fragments.
    ///
    /// The video must be currently live (`is_currently_live() == true`).
    /// Only the reqwest engine is supported for fragment streaming.
    ///
    /// # Arguments
    ///
    /// * `video` - The live stream video metadata.
    ///
    /// # Returns
    ///
    /// A [`live::LiveStreamBuilder`] configured for the provided live video.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # use tokio_stream::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://youtube.com/watch?v=LIVE_ID").await?;
    ///
    /// let mut stream = downloader.stream_live(&video)
    ///     .execute()
    ///     .await?;
    ///
    /// while let Some(fragment) = stream.next().await {
    ///     let fragment = fragment?;
    ///     println!("Fragment {} bytes", fragment.data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "live-streaming")]
    pub fn stream_live<'a>(&'a self, video: &'a Video) -> live::LiveStreamBuilder<'a> {
        live::LiveStreamBuilder::new(self, video)
    }

    /// Creates a new YouTube fetcher, and installs the yt-dlp and ffmpeg binaries.
    /// The output directory can be void if you only want to fetch the video information.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `executables_dir` - The directory where the binaries will be installed.
    /// * `output_dir` - The directory where the video will be downloaded.
    ///
    /// # Errors
    ///
    /// This function will return an error if the executables could not be installed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executables_dir = PathBuf::from("libs");
    /// let output_dir = PathBuf::from("output");
    ///
    /// let downloader = Downloader::with_new_binaries(executables_dir, output_dir)
    ///     .await?
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_new_binaries(
        executables_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Result<DownloaderBuilder> {
        let executables_dir: PathBuf = executables_dir.into();
        let output_dir: PathBuf = output_dir.into();

        tracing::info!(
            executables_dir = ?executables_dir,
            output_dir = ?output_dir,
            "📦 Installing dependencies"
        );

        let installer = LibraryInstaller::new(executables_dir.clone());

        // Check if binaries already exist
        let youtube_path = executables_dir.join(utils::find_executable("yt-dlp"));
        let ffmpeg_path = executables_dir.join(utils::find_executable("ffmpeg"));

        let youtube_exists = youtube_path.exists();
        let ffmpeg_exists = ffmpeg_path.exists();

        tracing::debug!(
            youtube_path = ?youtube_path,
            youtube_exists = youtube_exists,
            ffmpeg_path = ?ffmpeg_path,
            ffmpeg_exists = ffmpeg_exists,
            "📦 Checking for existing binaries"
        );

        let youtube = if youtube_exists {
            tracing::debug!("📦 Using existing yt-dlp binary");
            youtube_path
        } else {
            tracing::debug!("📦 Installing yt-dlp binary");
            installer.install_youtube(None).await?
        };

        let ffmpeg = if ffmpeg_exists {
            tracing::debug!("📦 Using existing ffmpeg binary");
            ffmpeg_path
        } else {
            tracing::debug!("📦 Installing ffmpeg binary");
            installer.install_ffmpeg(None).await?
        };

        tracing::info!(
            youtube_path = ?youtube,
            ffmpeg_path = ?ffmpeg,
            "✅ Dependencies ready"
        );

        let libraries = Libraries::new(youtube, ffmpeg);
        Ok(DownloaderBuilder::new(libraries, output_dir))
    }

    /// Returns a reference to the YouTube extractor if one is currently in use.
    ///
    /// This method allows access to YouTube-specific features like search, channel fetching,
    /// and player client selection. Returns `None` if using the generic extractor.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use std::path::PathBuf;
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// let downloader = Downloader::builder(libraries, "output")
    ///     .build()
    ///     .await?;
    ///
    /// let youtube = downloader.youtube_extractor();
    /// // Use YouTube-specific features
    /// let search_results = youtube.search("rust tutorials", 5).await?;
    /// let channel = youtube.fetch_channel("UC_channel_id").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn youtube_extractor(&self) -> &extractor::Youtube {
        &self.youtube_extractor
    }

    /// Returns a reference to the Generic extractor.
    ///
    /// # Returns
    ///
    /// A reference to the [`Generic`](extractor::Generic) extractor.
    pub fn generic_extractor(&self) -> &extractor::Generic {
        &self.generic_extractor
    }

    /// Returns a reference to the library paths.
    ///
    /// # Returns
    ///
    /// A reference to the [`Libraries`] configuration.
    pub fn libraries(&self) -> &Libraries {
        &self.libraries
    }

    /// Returns the output directory path.
    ///
    /// # Returns
    ///
    /// A reference to the output directory [`PathBuf`].
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Returns the current yt-dlp arguments.
    ///
    /// # Returns
    ///
    /// A slice of the current arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns the current user agent, if set.
    ///
    /// # Returns
    ///
    /// The optional user agent string.
    pub fn user_agent(&self) -> Option<&str> {
        self.user_agent.as_deref()
    }

    /// Returns the execution timeout.
    ///
    /// # Returns
    ///
    /// The current [`Duration`] timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns the proxy configuration, if set.
    ///
    /// # Returns
    ///
    /// A reference to the optional [`ProxyConfig`](client::proxy::ProxyConfig).
    pub fn proxy(&self) -> Option<&client::proxy::ProxyConfig> {
        self.proxy.as_ref()
    }

    /// Returns the cache layer, if configured.
    ///
    /// # Returns
    ///
    /// A reference to the optional [`CacheLayer`].
    #[cfg(cache)]
    pub fn cache(&self) -> Option<&Arc<CacheLayer>> {
        self.cache.as_ref()
    }

    /// Returns a reference to the download manager.
    ///
    /// # Returns
    ///
    /// A reference to the [`DownloadManager`].
    pub fn download_manager(&self) -> &Arc<DownloadManager> {
        &self.download_manager
    }

    /// Returns a reference to the event bus.
    ///
    /// # Returns
    ///
    /// A reference to the [`EventBus`](events::EventBus).
    pub fn event_bus(&self) -> &events::EventBus {
        &self.event_bus
    }

    /// Sets the user agent for HTTP requests.
    ///
    /// # Arguments
    ///
    /// * `user_agent` - The user agent string to use for HTTP requests.
    ///
    /// # Returns
    ///
    /// A mutable reference to `self` for method chaining.
    pub fn set_user_agent(&mut self, user_agent: impl AsRef<str>) -> &mut Self {
        tracing::debug!(user_agent = user_agent.as_ref(), "🔧 Setting user agent");
        self.user_agent = Some(user_agent.as_ref().to_string());
        self
    }

    /// Sets the arguments to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to pass to yt-dlp.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// let args = vec!["--no-progress".to_string()];
    /// downloader.append_args(args);
    /// # Ok(())
    /// # }
    /// ```
    pub fn append_args(&mut self, mut args: Vec<String>) -> &mut Self {
        tracing::debug!(arg_count = args.len(), "🔧 Appending custom yt-dlp arguments");
        self.args.append(&mut args);
        self
    }

    /// Replaces all yt-dlp arguments with the provided ones.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to pass to yt-dlp.
    pub fn set_args(&mut self, args: Vec<String>) -> &mut Self {
        tracing::debug!(arg_count = args.len(), "🔧 Setting custom yt-dlp arguments");
        self.args = args;
        self
    }

    /// Sets the timeout for command execution.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration for command execution.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// // Set a longer timeout for large videos
    /// downloader.set_timeout(Duration::from_secs(300));
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_timeout(&mut self, timeout: Duration) -> &mut Self {
        tracing::debug!(timeout = ?timeout, "🔧 Setting command execution timeout");
        self.timeout = timeout;
        self
    }

    /// Adds an argument to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to pass to yt-dlp.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// downloader.add_arg("--no-progress");
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_arg(&mut self, arg: impl AsRef<str>) -> &mut Self {
        tracing::debug!(arg = arg.as_ref(), "🔧 Adding custom yt-dlp argument");
        self.args.push(arg.as_ref().to_string());
        self
    }

    /// Use a Netscape cookie file for authentication.
    ///
    /// Pushes `--cookies=<path>` to both extractors and the raw yt-dlp arg list,
    /// so that metadata fetches and direct downloads are both authenticated.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Netscape cookie file
    pub fn set_cookies(&mut self, path: impl AsRef<Path>) -> &mut Self {
        tracing::debug!(cookies_path = ?path.as_ref(), "🔧 Configuring cookie authentication");
        let s = path.as_ref().display().to_string();
        self.youtube_extractor.with_cookies(path.as_ref());
        self.generic_extractor.with_cookies(path.as_ref());
        self.args.push(format!("--cookies={}", s));
        self
    }

    /// Extract cookies from a browser for authentication.
    ///
    /// Pushes `--cookies-from-browser=<browser>` to both extractors and the raw
    /// yt-dlp arg list.
    ///
    /// # Arguments
    ///
    /// * `browser` - Browser name (e.g. `"chrome"`, `"firefox"`)
    pub fn set_cookies_from_browser(&mut self, browser: impl AsRef<str>) -> &mut Self {
        tracing::debug!(browser = browser.as_ref(), "🔧 Configuring browser cookie extraction");
        let b = browser.as_ref();
        self.youtube_extractor.with_cookies_from_browser(b);
        self.generic_extractor.with_cookies_from_browser(b);
        self.args.push(format!("--cookies-from-browser={}", b));
        self
    }

    /// Use .netrc for authentication.
    ///
    /// Pushes `--netrc` to both extractors and the raw yt-dlp arg list.
    ///
    /// # Returns
    ///
    /// A mutable reference to `self` for method chaining.
    pub fn set_netrc(&mut self) -> &mut Self {
        tracing::debug!("🔧 Configuring .netrc authentication");
        self.youtube_extractor.with_netrc();
        self.generic_extractor.with_netrc();
        self.args.push("--netrc".to_string());
        self
    }

    /// Updates the yt-dlp executable.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Errors
    ///
    /// This function will return an error if the yt-dlp executable could not be updated.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// downloader.update_downloader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_downloader(&self) -> Result<()> {
        tracing::info!("🔄 Updating yt-dlp binary");

        let args = vec!["--update"];

        let executor = Executor::new(self.libraries.youtube.clone(), utils::to_owned(args), self.timeout);

        executor.execute().await?;
        Ok(())
    }

    /// Enables caching of video metadata.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration (directory, TTLs, optional Redis URL).
    ///
    /// # Errors
    ///
    /// Returns an error if the cache layer could not be initialized.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::cache::CacheConfig;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut downloader = Downloader::builder(libraries, output_dir).build().await?;
    ///
    /// // Enable caching with default TTLs
    /// let config = CacheConfig::builder()
    ///     .cache_dir(PathBuf::from("cache"))
    ///     .build();
    /// downloader.set_cache(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(cache)]
    pub async fn set_cache(&mut self, config: CacheConfig) -> Result<&mut Self> {
        tracing::debug!(config = %config, "🔍 Enabling cache layer");

        let layer = CacheLayer::from_config(&config).await?;
        self.cache = Some(Arc::new(layer));

        tracing::debug!("✅ Cache layer enabled");

        Ok(self)
    }

    /// Initiates a graceful shutdown of all ongoing operations.
    ///
    /// This method triggers the cancellation token, signaling all ongoing
    /// downloads and operations to stop gracefully. It does not wait for
    /// operations to complete.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let downloader = Downloader::builder(libs, "output").build().await?;
    ///
    /// // Start some downloads...
    ///
    /// // Initiate graceful shutdown
    /// downloader.shutdown();
    /// # Ok(())
    /// # }
    /// ```
    pub fn shutdown(&self) {
        tracing::info!("🛑 Initiating graceful shutdown");

        self.cancellation_token.cancel();
    }

    /// Checks if a shutdown has been requested.
    ///
    /// # Returns
    ///
    /// Returns `true` if shutdown has been initiated, `false` otherwise.
    pub fn is_shutdown_requested(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Detects which extractor should be used for the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// The name of the extractor (e.g. "youtube", "vimeo", "generic")
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let extractor = downloader.detect_extractor("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    /// println!("Extractor: {}", extractor);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect_extractor(&self, url: &str) -> Result<ExtractorName> {
        tracing::debug!(url = url, "📡 Detecting extractor for URL");

        let extractor = extractor::detector::detect_extractor_type(url, &self.libraries.youtube).await?;

        tracing::debug!(
            url = url,
            extractor = ?extractor,
            "📡 Extractor detected"
        );

        Ok(extractor)
    }
}

impl Clone for Downloader {
    fn clone(&self) -> Self {
        // Clone extractors to preserve auth state (cookies, netrc, browser cookies, args)
        let youtube_extractor = self.youtube_extractor.clone();
        let generic_extractor = self.generic_extractor.clone();

        Self {
            youtube_extractor,
            generic_extractor,
            libraries: self.libraries.clone(),
            output_dir: self.output_dir.clone(),
            args: self.args.clone(),
            user_agent: self.user_agent.clone(),
            timeout: self.timeout,
            proxy: self.proxy.clone(),
            #[cfg(cache)]
            cache: self.cache.clone(),
            download_manager: self.download_manager.clone(),
            cancellation_token: self.cancellation_token.clone(),
            event_bus: self.event_bus.clone(),
            #[cfg(feature = "hooks")]
            hook_registry: self.hook_registry.clone(),
            #[cfg(feature = "webhooks")]
            webhook_delivery: self.webhook_delivery.clone(),
            #[cfg(feature = "statistics")]
            statistics: self.statistics.clone(),
        }
    }
}

impl Display for Downloader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Downloader(output_dir={}, timeout={}s, proxy={})",
            self.output_dir.display(),
            self.timeout.as_secs(),
            self.proxy.is_some()
        )
    }
}
