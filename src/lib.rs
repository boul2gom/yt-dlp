#![doc = include_str!("../README.md")]

use crate::client::deps::{Libraries, LibraryInstaller};
use crate::download::manager::ManagerConfig;
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::extractor::ExtractorName;
use crate::metadata::MetadataManager;
use crate::utils::fs;
#[cfg(feature = "cache")]
use cache::{DownloadCache, PlaylistCache, VideoCache};
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

// Core modules
#[cfg(feature = "cache")]
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

// Convenience modules
pub mod macros;
pub mod prelude;

// Re-export of common traits to facilitate their use
use crate::model::Video;
use crate::model::format::{Format, FormatType};
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
pub use client::streams::selection::VideoSelection;
pub use model::utils::{AllTraits, CommonTraits};

// Re-export main types for easy access
pub use client::{DownloadBuilder, YoutubeBuilder};
pub use download::{DownloadManager, DownloadPriority, DownloadStatus};

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
/// let downloader = Downloader::new(libraries, "output").await?;
///
/// // YouTube is automatically detected and optimized
/// let video = downloader.fetch_video_infos("https://youtube.com/watch?v=...".to_string()).await?;
/// downloader.download_video(&video, "video.mp4").await?;
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
/// # let downloader = Downloader::new(libraries, "output").await?;
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
/// # let mut downloader = Downloader::new(libraries, "output").await?;
/// // Access YouTube-specific methods
/// if let Some(youtube) = downloader.youtube_extractor() {
///     let channel = youtube.fetch_channel("UC...").await?;
///     let search = youtube.search("rust programming", 10).await?;
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Downloader {
    /// The video extractor (Youtube or Generic)
    pub(crate) extractor: Box<dyn extractor::VideoExtractor>,
    /// The required libraries.
    pub libraries: Libraries,

    /// The directory where the video (or formats) will be downloaded.
    pub output_dir: PathBuf,
    /// The arguments to pass to 'yt-dlp'.
    pub args: Vec<String>,
    /// The requests user agent
    pub user_agent: Option<String>,
    /// The timeout for command execution.
    pub timeout: Duration,
    /// Optional proxy configuration for HTTP requests and yt-dlp.
    pub proxy: Option<client::proxy::ProxyConfig>,
    /// The cache for video metadata.
    #[cfg(feature = "cache")]
    pub cache: Option<Arc<cache::VideoCache>>,
    /// The cache for downloaded files.
    #[cfg(feature = "cache")]
    pub download_cache: Option<Arc<cache::DownloadCache>>,
    /// The cache for playlist metadata.
    #[cfg(feature = "cache")]
    pub playlist_cache: Option<Arc<cache::PlaylistCache>>,
    /// The download manager for managing parallel downloads.
    pub download_manager: Arc<DownloadManager>,
    /// Cancellation token for graceful shutdown.
    pub(crate) cancellation_token: tokio_util::sync::CancellationToken,
    /// Event bus for broadcasting download events.
    pub event_bus: events::EventBus,
    /// Hook registry for Rust hooks (feature: hooks).
    #[cfg(feature = "hooks")]
    pub(crate) hook_registry: Option<events::HookRegistry>,
    /// Webhook delivery system (feature: webhooks).
    #[cfg(feature = "webhooks")]
    pub(crate) webhook_delivery: Option<events::WebhookDelivery>,
}

impl Clone for Downloader {
    fn clone(&self) -> Self {
        // Create a new Generic extractor with the same configuration
        let extractor = extractor::Generic::new(self.libraries.youtube.clone());

        Self {
            extractor: Box::new(extractor),
            libraries: self.libraries.clone(),
            output_dir: self.output_dir.clone(),
            args: self.args.clone(),
            user_agent: self.user_agent.clone(),
            timeout: self.timeout,
            proxy: self.proxy.clone(),
            #[cfg(feature = "cache")]
            cache: self.cache.clone(),
            #[cfg(feature = "cache")]
            download_cache: self.download_cache.clone(),
            #[cfg(feature = "cache")]
            playlist_cache: self.playlist_cache.clone(),
            download_manager: self.download_manager.clone(),
            cancellation_token: self.cancellation_token.clone(),
            event_bus: self.event_bus.clone(),
            #[cfg(feature = "hooks")]
            hook_registry: self.hook_registry.clone(),
            #[cfg(feature = "webhooks")]
            webhook_delivery: self.webhook_delivery.clone(),
        }
    }
}

impl fmt::Display for Downloader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Downloader ({}): output_dir={:?}, args={:?}, proxy={}",
            self.extractor.name(),
            self.output_dir,
            self.args,
            self.proxy.is_some()
        )
    }
}

impl Downloader {
    /// Creates a new builder for constructing a Youtube instance with a fluent API.
    ///
    /// This is the recommended way to create a Youtube instance as it provides
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
    /// let youtube = Downloader::builder(libraries, "output")
    ///     .with_timeout(std::time::Duration::from_secs(120))
    ///     .with_max_concurrent_downloads(4)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(libraries: Libraries, output_dir: impl Into<PathBuf>) -> YoutubeBuilder {
        YoutubeBuilder::new(libraries, output_dir)
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
    /// let fetcher = Downloader::new(libraries, "output").await?;
    /// let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    ///
    /// // Download a 1080p video with H264 codec
    /// let video_path = fetcher.download(url, "my-video.mp4")
    ///     .video_quality(VideoQuality::CustomHeight(1080))
    ///     .video_codec(VideoCodecPreference::AVC1)
    ///     .audio_quality(AudioQuality::Best)
    ///     .execute()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn download(
        &self,
        url: impl Into<String>,
        output: impl Into<PathBuf>,
    ) -> client::DownloadBuilder<'_> {
        client::DownloadBuilder::new(self, url, output)
    }

    /// Creates a new YouTube fetcher with the given yt-dlp executable, ffmpeg executable and video URL.
    /// The output directory can be void if you only want to fetch the video information.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries.
    /// * `output_dir` - The directory where the video will be downloaded.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parent directories of the executables and output directory could not be created.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let libraries_dir = PathBuf::from("libs");
    /// let output_dir = PathBuf::from("output");
    ///
    /// let youtube = libraries_dir.join("yt-dlp");
    /// let ffmpeg = libraries_dir.join("ffmpeg");
    ///
    /// let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Downloader::new(libraries, output_dir).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher");

        fs::create_parent_dir(&output_dir).await?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        fs::create_parent_dir(&cache_dir).await?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let playlist_cache = PlaylistCache::new(cache_dir.join("playlists.db")).await?;

        // Initialize event bus first
        let event_bus = events::EventBus::with_default_capacity();

        // Initialize download manager with default configuration and event bus
        let download_manager = DownloadManager::with_config_and_event_bus(
            ManagerConfig::default(),
            Some(event_bus.clone()),
        );

        // Create Generic extractor (supports all sites including YouTube)
        let extractor = extractor::Generic::new(libraries.youtube.clone());

        Ok(Self {
            extractor: Box::new(extractor),
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            user_agent: None,
            timeout: Duration::from_secs(30),
            proxy: None,
            #[cfg(feature = "cache")]
            cache: Some(Arc::new(cache)),
            #[cfg(feature = "cache")]
            download_cache: Some(Arc::new(download_cache)),
            #[cfg(feature = "cache")]
            playlist_cache: Some(Arc::new(playlist_cache)),
            download_manager: Arc::new(download_manager),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            event_bus,
            #[cfg(feature = "hooks")]
            hook_registry: Some(events::HookRegistry::new()),
            #[cfg(feature = "webhooks")]
            webhook_delivery: Some(events::WebhookDelivery::new()),
        })
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
    pub async fn with_download_manager_config(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
        download_manager_config: ManagerConfig,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher with custom download manager config");

        fs::create_parent_dir(&output_dir).await?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        fs::create_parent_dir(&cache_dir).await?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let playlist_cache = PlaylistCache::new(cache_dir.join("playlists.db")).await?;

        // Initialize event bus first
        let event_bus = events::EventBus::with_default_capacity();

        // Initialize download manager with custom configuration and event bus
        let download_manager = DownloadManager::with_config_and_event_bus(
            download_manager_config,
            Some(event_bus.clone()),
        );

        // Create Generic extractor (supports all sites including YouTube)
        let extractor = extractor::Generic::new(libraries.youtube.clone());

        Ok(Self {
            extractor: Box::new(extractor),
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            user_agent: None,
            timeout: Duration::from_secs(30),
            proxy: None,
            #[cfg(feature = "cache")]
            cache: Some(Arc::new(cache)),
            #[cfg(feature = "cache")]
            download_cache: Some(Arc::new(download_cache)),
            #[cfg(feature = "cache")]
            playlist_cache: Some(Arc::new(playlist_cache)),
            download_manager: Arc::new(download_manager),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            event_bus,
            #[cfg(feature = "hooks")]
            hook_registry: Some(events::HookRegistry::new()),
            #[cfg(feature = "webhooks")]
            webhook_delivery: Some(events::WebhookDelivery::new()),
        })
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
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executables_dir = PathBuf::from("libs");
    /// let output_dir = PathBuf::from("output");
    ///
    /// let fetcher = Downloader::with_new_binaries(executables_dir, output_dir).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_new_binaries(
        executables_dir: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
        output_dir: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher with binaries installation");

        let installer = LibraryInstaller::new(executables_dir.as_ref().to_path_buf());

        // Check if binaries already exist
        let youtube_path = executables_dir
            .as_ref()
            .join(utils::find_executable("yt-dlp"));
        let ffmpeg_path = executables_dir
            .as_ref()
            .join(utils::find_executable("ffmpeg"));

        let youtube = if youtube_path.exists() {
            youtube_path
        } else {
            installer.install_youtube(None).await?
        };

        let ffmpeg = if ffmpeg_path.exists() {
            ffmpeg_path
        } else {
            installer.install_ffmpeg(None).await?
        };

        let libraries = Libraries::new(youtube, ffmpeg);
        Self::new(libraries, output_dir).await
    }

    /// Creates a new Downloader with YouTube-optimized extractor.
    ///
    /// This constructor creates a Downloader that uses the highly optimized YouTube extractor
    /// instead of the generic one, providing access to YouTube-specific features like search,
    /// channel fetching, and player client selection.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries (yt-dlp and ffmpeg paths)
    /// * `output_dir` - The directory where videos will be downloaded
    ///
    /// # Errors
    ///
    /// Returns an error if the directories cannot be created
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
    /// let downloader = Downloader::for_youtube(libraries, "output").await?;
    ///
    /// // Access YouTube-specific features
    /// if let Some(youtube) = downloader.youtube_extractor() {
    ///     let results = youtube.search("rust programming", 10).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn for_youtube(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new Downloader with YouTube extractor");

        fs::create_parent_dir(&output_dir).await?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        fs::create_parent_dir(&cache_dir).await?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let playlist_cache = PlaylistCache::new(cache_dir.join("playlists.db")).await?;

        // Initialize event bus first
        let event_bus = events::EventBus::with_default_capacity();

        // Initialize download manager with default configuration and event bus
        let download_manager = DownloadManager::with_config_and_event_bus(
            ManagerConfig::default(),
            Some(event_bus.clone()),
        );

        // Create YouTube extractor for optimized YouTube support
        let extractor = extractor::Youtube::new(libraries.youtube.clone());

        Ok(Self {
            extractor: Box::new(extractor),
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            user_agent: None,
            timeout: Duration::from_secs(30),
            proxy: None,
            #[cfg(feature = "cache")]
            cache: Some(Arc::new(cache)),
            #[cfg(feature = "cache")]
            download_cache: Some(Arc::new(download_cache)),
            #[cfg(feature = "cache")]
            playlist_cache: Some(Arc::new(playlist_cache)),
            download_manager: Arc::new(download_manager),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            event_bus,
            #[cfg(feature = "hooks")]
            hook_registry: Some(events::HookRegistry::new()),
            #[cfg(feature = "webhooks")]
            webhook_delivery: Some(events::WebhookDelivery::new()),
        })
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
    /// let downloader = Downloader::for_youtube(libraries, "output").await?;
    ///
    /// if let Some(youtube) = downloader.youtube_extractor() {
    ///     // Use YouTube-specific features
    ///     let search_results = youtube.search("rust tutorials", 5).await?;
    ///     let channel = youtube.fetch_channel("UC_channel_id").await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn youtube_extractor(&self) -> Option<&extractor::Youtube> {
        self.extractor.as_any().downcast_ref::<extractor::Youtube>()
    }

    /// Sets the user agent for HTTP requests.
    pub fn with_user_agent(&mut self, user_agent: impl Into<String>) -> &mut Self {
        self.user_agent = Some(user_agent.into());
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// let args = vec!["--no-progress".to_string()];
    /// fetcher.with_args(args);
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_args(&mut self, mut args: Vec<String>) -> &mut Self {
        self.args.append(&mut args);
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
    /// ```rust, no_run
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// // Set a longer timeout for large videos
    /// fetcher.with_timeout(Duration::from_secs(300));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_timeout(&mut self, timeout: Duration) -> &mut Self {
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// fetcher.with_arg("--no-progress");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_arg(&mut self, arg: impl AsRef<str>) -> &mut Self {
        self.args.push(arg.as_ref().to_string());
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
    /// let fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// fetcher.update_downloader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_downloader(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Updating the downloader");

        let args = vec!["--update"];

        let executor = Executor::new(
            self.libraries.youtube.clone(),
            utils::to_owned(args),
            self.timeout,
        );

        executor.execute().await?;
        Ok(())
    }

    /// Combines the audio and video files into a single file.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `audio_file` - The name of the audio file to combine.
    /// * `video_file` - The name of the video file to combine.
    /// * `output_file` - The name of the output file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the audio and video files could not be combined.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::VideoSelection;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let audio_format = video.best_audio_format().unwrap();
    /// let audio_path = fetcher.download_format(&audio_format, "audio-stream.mp3").await?;
    ///
    /// let video_format = video.worst_video_format().unwrap();
    /// let format_path = fetcher.download_format(&video_format, "video-stream.mp4").await?;
    ///
    /// let output_path = fetcher.combine_audio_and_video("audio-stream.mp3", "video-stream.mp4", "my-output.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn combine_audio_and_video(
        &self,
        audio_file: impl AsRef<str> + std::fmt::Debug + Display,
        video_file: impl AsRef<str> + std::fmt::Debug + Display,
        output_file: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> Result<PathBuf> {
        let audio_path = self.output_dir.join(audio_file.as_ref());
        let video_path = self.output_dir.join(video_file.as_ref());
        let output_path = self.output_dir.join(output_file.as_ref());
        self.combine_audio_and_video_to_path(&audio_path, &video_path, &output_path)
            .await
    }

    /// Combines audio and video files into a single file at a specific path.
    ///
    /// Unlike [`combine_audio_and_video`](Self::combine_audio_and_video), this method uses
    /// the exact paths specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `audio_file` - The full path to the audio file.
    /// * `video_file` - The full path to the video file.
    /// * `output_file` - The full path for the combined output file.
    pub async fn combine_audio_and_video_to_path(
        &self,
        audio_file: impl AsRef<Path> + std::fmt::Debug,
        video_file: impl AsRef<Path> + std::fmt::Debug,
        output_file: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<PathBuf> {
        let audio_path = audio_file.as_ref().to_path_buf();
        let video_path = video_file.as_ref().to_path_buf();
        let output_path = output_file.as_ref().to_path_buf();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Combining audio and video files {:?} and {:?}, into {:?}",
            audio_path,
            video_path,
            output_path
        );

        // Perform the combination with FFmpeg
        self.execute_ffmpeg_combine(&audio_path, &video_path, &output_path)
            .await?;

        // Add metadata to the combined file, propagating potential errors
        self.add_metadata_to_combined_file(&audio_path, &video_path, &output_path)
            .await?;

        Ok(output_path)
    }

    /// Executes the FFmpeg command to combine audio and video files
    async fn execute_ffmpeg_combine(
        &self,
        audio_path: impl AsRef<Path>,
        video_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let audio = audio_path
            .as_ref()
            .to_str()
            .ok_or(Error::Unknown("Invalid audio path".to_string()))?;
        let video = video_path
            .as_ref()
            .to_str()
            .ok_or(Error::Unknown("Invalid video path".to_string()))?;
        let output = output_path
            .as_ref()
            .to_str()
            .ok_or(Error::Unknown("Invalid output path".to_string()))?;

        let args = vec![
            "-i", audio, "-i", video, "-c:v", "copy", "-c:a", "aac", output,
        ];

        let executor = Executor::new(
            self.libraries.ffmpeg.clone(),
            utils::to_owned(args),
            self.timeout,
        );

        executor.execute().await?;
        Ok(())
    }

    /// Adds metadata to the combined file by extracting the video ID and
    /// retrieving information from the original audio and video formats
    async fn add_metadata_to_combined_file(
        &self,
        audio_path: impl AsRef<Path>,
        video_path: impl AsRef<Path>,
        output_path: impl AsRef<Path>,
    ) -> Result<()> {
        let video_id =
            self.extract_video_id_from_file_paths(video_path.as_ref(), audio_path.as_ref());

        if let Some(video_id) = video_id
            && let Some(video) = self.get_video_by_id(&video_id).await
        {
            #[cfg(feature = "tracing")]
            tracing::debug!("Adding metadata to combined file");

            cfg_if::cfg_if! {
                if #[cfg(feature = "cache")] {
                    let video_format = self.find_cached_format(video_path.as_ref()).await;
                    let audio_format = self.find_cached_format(audio_path.as_ref()).await;

                    // Add metadata (including chapters) to the combined file with full format information
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(_e) = metadata_manager.add_metadata_with_chapters(
                        output_path.as_ref(),
                        &video,
                        video_format.as_ref(),
                        audio_format.as_ref(),
                    )
                    .await
                    {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to add metadata to combined file: {}", _e);
                    } else {
                        #[cfg(feature = "tracing")]
                        tracing::debug!("Successfully added metadata (including chapters) to combined file");
                    }
                } else {
                    // Without cache, we don't have format details, add basic metadata only
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(e) = metadata_manager.add_metadata(
                        output_path.as_ref(),
                        &video,
                    )
                    .await
                    {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to add basic metadata to combined file: {}", e);
                    } else {
                        #[cfg(feature = "tracing")]
                        tracing::debug!("Successfully added basic metadata to combined file");
                    }
                }
            }
        }

        Ok(())
    }

    /// Extracts the video ID from audio and video file paths
    fn extract_video_id_from_file_paths(
        &self,
        video_path: impl AsRef<Path>,
        audio_path: impl AsRef<Path>,
    ) -> Option<String> {
        let video_filename = video_path.as_ref().file_name()?.to_str()?;

        if let Some(id) = utils::fs::extract_video_id(video_filename) {
            return Some(id);
        }

        let audio_filename = audio_path.as_ref().file_name()?.to_str()?;
        utils::fs::extract_video_id(audio_filename)
    }

    /// Finds the format of a file in the cache if it exists
    #[cfg(feature = "cache")]
    async fn find_cached_format(&self, file_path: impl AsRef<Path>) -> Option<Format> {
        if let Some(download_cache) = &self.download_cache {
            let file_hash = match DownloadCache::calculate_file_hash(file_path.as_ref()).await {
                Ok(hash) => hash,
                Err(_) => return None,
            };

            if let Some((cached_file, _)) = download_cache.get_by_hash(&file_hash).await
                && let Some(ref format_json) = cached_file.format_json
                && let Ok(format) = serde_json::from_str(format_json)
            {
                return Some(format);
            }
        }

        None
    }

    /// Enables caching of video metadata.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 24 hours).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory could not be created.
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// // Enable video metadata caching
    /// fetcher.with_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache")]
    pub async fn with_cache(
        &mut self,
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Enabling video metadata cache");

        let cache = VideoCache::new(cache_dir.as_ref(), ttl).await?;
        self.cache = Some(Arc::new(cache));
        Ok(self)
    }

    /// Enables caching of downloaded files.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 7 days).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory could not be created.
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// // Enable downloaded files caching
    /// fetcher.with_download_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache")]
    pub async fn with_download_cache(
        &mut self,
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Enabling downloaded files cache");

        let download_cache = DownloadCache::new(cache_dir.as_ref(), ttl).await?;
        self.download_cache = Some(Arc::new(download_cache));
        Ok(self)
    }

    /// Enables caching of playlist metadata.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory where to store the cache.
    /// * `ttl` - The time-to-live for cache entries in seconds (default: 6 hours).
    ///
    /// # Errors
    ///
    /// This function will return an error if the cache directory could not be created.
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
    /// let mut fetcher = Downloader::new(libraries, output_dir).await?;
    ///
    /// // Enable playlist metadata caching
    /// fetcher.with_playlist_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache")]
    pub async fn with_playlist_cache(
        &mut self,
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<i64>,
    ) -> Result<&mut Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Enabling playlist metadata cache");

        let db_path = cache_dir.as_ref().join("playlists.db");
        let playlist_cache = if let Some(ttl_seconds) = ttl {
            PlaylistCache::with_ttl(db_path, ttl_seconds as u64).await?
        } else {
            PlaylistCache::new(db_path).await?
        };
        self.playlist_cache = Some(Arc::new(playlist_cache));
        Ok(self)
    }

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
        video: &model::Video,
        output: impl AsRef<str> + std::fmt::Debug,
        priority: Option<download::manager::DownloadPriority>,
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
        video: &model::Video,
        output: impl AsRef<Path> + std::fmt::Debug,
        priority: Option<download::manager::DownloadPriority>,
    ) -> Result<u64> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video with priority: {}", video.id);

        // Get the best format with video and audio
        let format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::AudioVideo,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: format.format_id.clone(),
            })?;

        // Use the provided path directly
        let output_path = output.as_ref().to_path_buf();

        // Add to download queue
        let download_id = self
            .download_manager
            .enqueue(url, output_path, priority)
            .await;

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
        video: &model::Video,
        output: impl AsRef<str> + std::fmt::Debug,
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
        video: &model::Video,
        output: impl AsRef<Path> + std::fmt::Debug,
        progress_callback: F,
    ) -> Result<u64>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video with progress tracking: {}", video.id);

        // Get the best format with video and audio
        let format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::AudioVideo,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: format.format_id.clone(),
            })?;

        // Use the provided path directly
        let output_path = output.as_ref().to_path_buf();

        // Add to download queue with progress callback
        let download_id = self
            .download_manager
            .enqueue_with_progress(
                url,
                output_path,
                Some(download::manager::DownloadPriority::Normal),
                progress_callback,
            )
            .await;

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
    pub async fn get_download_status(
        &self,
        download_id: u64,
    ) -> Option<download::manager::DownloadStatus> {
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
    pub async fn wait_for_download(
        &self,
        download_id: u64,
    ) -> Option<download::manager::DownloadStatus> {
        self.download_manager.wait_for_completion(download_id).await
    }

    /// Downloads a video with the specified video and audio quality preferences.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The name of the output file
    /// * `video_quality` - The desired video quality
    /// * `video_codec` - The preferred video codec
    /// * `audio_quality` - The desired audio quality
    /// * `audio_codec` - The preferred audio codec
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file
    ///
    /// # Example
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference, AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// # let fetcher = Downloader::new(libraries, output_dir).await?;
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    ///
    /// // Download a high quality video with VP9 codec and high quality audio with Opus codec
    /// let video_path = fetcher.download_video_with_quality(
    ///     url,
    ///     "my-video.mp4",
    ///     VideoQuality::High,
    ///     VideoCodecPreference::VP9,
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_with_quality(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Audio,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Download video format with preferences
        let video_ext = format!("{:?}", video_format.download_info.ext);
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);

        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let video_path = self
                    .download_format_with_preferences(
                        video_format,
                        &video_filename,
                        Some(video_quality),
                        None,
                        Some(video_codec),
                        None,
                    )
                    .await?;
            } else {
                let video_path = self
                    .download_format(video_format, &video_filename)
                    .await?;
            }
        }

        // Download audio format with preferences
        let audio_ext = format!("{:?}", audio_format.download_info.ext);
        let audio_filename = format!("temp_audio_{}.{}", utils::fs::random_filename(8), audio_ext);
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let audio_path = self
                    .download_format_with_preferences(
                        audio_format,
                        &audio_filename,
                        None,
                        Some(audio_quality),
                        None,
                        Some(audio_codec),
                    )
                    .await?;
            } else {
                let audio_path = self
                    .download_format(audio_format, &audio_filename)
                    .await?;
            }
        }

        // Combine audio and video
        let output_path = self
            .combine_audio_and_video(&audio_filename, &video_filename, output)
            .await?;

        // Clean up temporary files
        if let Err(_e) = tokio::fs::remove_file(&video_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary video file: {}", _e);
        }

        if let Err(_e) = tokio::fs::remove_file(&audio_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary audio file: {}", _e);
        }

        Ok(output_path)
    }

    /// Downloads a video with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_with_quality`](Self::download_video_with_quality), this method writes
    /// the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The full path where the file will be saved
    /// * `video_quality` - The desired video quality
    /// * `video_codec` - The preferred video codec
    /// * `audio_quality` - The desired audio quality
    /// * `audio_codec` - The preferred audio codec
    pub async fn download_video_with_quality_to_path(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let url_str = url.as_ref().to_string();
        self.execute_with_retry(url_str, move |video| {
            let output = output.as_ref().to_path_buf();
            let downloader = self.clone();
            let video_codec = video_codec.clone();
            let audio_codec = audio_codec.clone();
            async move {
                downloader
                    .download_video_with_quality_to_path_inner(
                        &video,
                        output,
                        video_quality,
                        video_codec,
                        audio_quality,
                        audio_codec,
                    )
                    .await
            }
        })
        .await
    }

    /// Internal implementation for download_video_with_quality_to_path.
    /// Separated to allow retry on 403 wrapping without duplicating the complex body.
    async fn download_video_with_quality_to_path_inner(
        &self,
        video: &Video,
        output: impl AsRef<Path> + std::fmt::Debug,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Audio,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Download video format with preferences
        let video_ext = format!("{:?}", video_format.download_info.ext);
        let video_filename = format!("temp_video_{}.{}", utils::fs::random_filename(8), video_ext);

        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let video_path = self
                    .download_format_with_preferences(
                        video_format,
                        &video_filename,
                        Some(video_quality),
                        None,
                        Some(video_codec),
                        None,
                    )
                    .await?;
            } else {
                let video_path = self
                    .download_format(video_format, &video_filename)
                    .await?;
            }
        }

        // Download audio format with preferences
        let audio_ext = format!("{:?}", audio_format.download_info.ext);
        let audio_filename = format!("temp_audio_{}.{}", utils::fs::random_filename(8), audio_ext);
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let audio_path = self
                    .download_format_with_preferences(
                        audio_format,
                        &audio_filename,
                        None,
                        Some(audio_quality),
                        None,
                        Some(audio_codec),
                    )
                    .await?;
            } else {
                let audio_path = self
                    .download_format(audio_format, &audio_filename)
                    .await?;
            }
        }

        // Combine audio and video to the user's path
        let output_ref = output.as_ref();
        let output_filename = output_ref
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("output.mp4");
        let combined_path = self
            .combine_audio_and_video(&audio_filename, &video_filename, output_filename)
            .await?;

        // If the user specified a different directory than output_dir, move the file
        let final_path = output_ref.to_path_buf();
        if combined_path != final_path {
            if let Some(parent) = final_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            if (tokio::fs::rename(&combined_path, &final_path).await).is_err() {
                tokio::fs::copy(&combined_path, &final_path).await?;
                tokio::fs::remove_file(&combined_path).await?;
            }
        }

        // Clean up temporary files
        if let Err(_e) = tokio::fs::remove_file(&video_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary video file: {}", _e);
        }

        if let Err(_e) = tokio::fs::remove_file(&audio_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary audio file: {}", _e);
        }

        Ok(final_path)
    }

    /// Downloads a video stream with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The name of the output file
    /// * `quality` - The desired video quality
    /// * `codec` - The preferred video codec
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file
    ///
    /// # Example
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// # let fetcher = Downloader::new(libraries, output_dir).await?;
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    ///
    /// // Download a medium quality video with AVC1 codec
    /// let video_path = fetcher.download_video_stream_with_quality(
    ///     url,
    ///     "video-only.mp4",
    ///     VideoQuality::Medium,
    ///     VideoCodecPreference::AVC1
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream_with_quality(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(quality, codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Download video format with preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_with_preferences(
                    video_format,
                    output,
                    Some(quality),
                    None,
                    Some(codec),
                    None,
                )
                .await
            } else {
                self.download_format(video_format, output)
                    .await
            }
        }
    }

    /// Downloads a video stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_stream_with_quality`](Self::download_video_stream_with_quality), this method
    /// writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The full path where the file will be saved
    /// * `quality` - The desired video quality
    /// * `codec` - The preferred video codec
    pub async fn download_video_stream_with_quality_to_path(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        let url_str = url.as_ref().to_string();
        self.execute_with_retry(url_str, move |video| {
            let output = output.as_ref().to_path_buf();
            let downloader = self.clone();
            let codec = codec.clone();
            async move {
                // Select video format based on quality and codec preferences
                let video_format = video
                    .select_video_format(quality, codec.clone())
                    .ok_or_else(|| Error::FormatNotAvailable {
                        video_id: video.id.clone(),
                        format_type: FormatType::Video,
                        available_formats: video
                            .formats
                            .iter()
                            .map(|f| f.format_id.clone())
                            .collect(),
                    })?;

                // Download directly to the specified path
                downloader
                    .download_format_to_path(video_format, &output)
                    .await
            }
        })
        .await
    }

    /// Downloads an audio stream with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The name of the output file
    /// * `quality` - The desired audio quality
    /// * `codec` - The preferred audio codec
    ///
    /// # Returns
    ///
    /// The path to the downloaded audio file
    ///
    /// # Example
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// # let fetcher = Downloader::new(libraries, output_dir).await?;
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    ///
    /// // Download a high quality audio with Opus codec
    /// let audio_path = fetcher.download_audio_stream_with_quality(
    ///     url,
    ///     "audio-only.mp3",
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream_with_quality(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(quality, codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Audio,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Download audio format with preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_with_preferences(
                    audio_format,
                    output,
                    None,
                    Some(quality),
                    None,
                    Some(codec),
                )
                .await
            } else {
                self.download_format(audio_format, output)
                    .await
            }
        }
    }

    /// Downloads an audio stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_audio_stream_with_quality`](Self::download_audio_stream_with_quality), this method
    /// writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The full path where the file will be saved
    /// * `quality` - The desired audio quality
    /// * `codec` - The preferred audio codec
    pub async fn download_audio_stream_with_quality_to_path(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync + Clone,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let url_str = url.as_ref().to_string();
        self.execute_with_retry(url_str, move |video| {
            let output = output.as_ref().to_path_buf();
            let downloader = self.clone();
            let codec = codec.clone();
            async move {
                // Select audio format based on quality and codec preferences
                let audio_format = video
                    .select_audio_format(quality, codec.clone())
                    .ok_or_else(|| Error::FormatNotAvailable {
                        video_id: video.id.clone(),
                        format_type: FormatType::Audio,
                        available_formats: video
                            .formats
                            .iter()
                            .map(|f| f.format_id.clone())
                            .collect(),
                    })?;

                // Download directly to the specified path
                downloader
                    .download_format_to_path(audio_format, &output)
                    .await
            }
        })
        .await
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
    /// let youtube = Downloader::new(libs, "output").await?;
    ///
    /// // Start some downloads...
    ///
    /// // Initiate graceful shutdown
    /// youtube.shutdown();
    /// # Ok(())
    /// # }
    /// ```
    pub fn shutdown(&self) {
        #[cfg(feature = "tracing")]
        tracing::info!("Initiating graceful shutdown");

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
    /// # let downloader = Downloader::new(libraries, "output").await?;
    /// let extractor = downloader.detect_extractor("https://www.youtube.com/watch?v=dQw4w9WgXcQ").await?;
    /// println!("Extractor: {}", extractor);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect_extractor(&self, url: &str) -> Result<ExtractorName> {
        crate::extractor::detector::detect_extractor_type(url, &self.libraries.youtube).await
    }

    // ==================== Fluent API Methods ====================

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
    /// let (youtube, video) = Downloader::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .fetch("https://youtube.com/watch?v=dQw4w9WgXcQ")
    ///     .await?;
    ///
    /// println!("Title: {}", video.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch(self, url: impl Into<String>) -> Result<(Self, model::Video)> {
        let video = self.fetch_video_infos(url.into()).await?;
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
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use std::path::PathBuf;
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let (downloader, video) = Downloader::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .fetch("https://youtube.com/watch?v=dQw4w9WgXcQ")
    ///     .await?;
    ///
    /// downloader.download_and_continue(&video, "output.mp4")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_and_continue(
        self,
        video: &model::Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> Result<Self> {
        self.download_video(video, output).await?;
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
    pub async fn download_and_continue_to_path(
        self,
        video: &model::Video,
        output: impl AsRef<Path> + std::fmt::Debug + Send + Sync,
    ) -> Result<Self> {
        self.download_video_to_path(video, output).await?;
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
    ///     .pipeline("https://youtube.com/watch?v=dQw4w9WgXcQ", |yt, video| async move {
    ///         yt.download_video(&video, "video.mp4").await?;
    ///         Ok(yt)
    ///     })
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn pipeline<F, Fut>(self, url: impl Into<String>, operation: F) -> Result<Self>
    where
        F: FnOnce(Self, model::Video) -> Fut,
        Fut: std::future::Future<Output = Result<Self>>,
    {
        let video = self.fetch_video_infos(url.into()).await?;
        operation(self, video).await
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
    /// # use yt_dlp::download::postprocess::{PostProcessConfig, VideoCodec, AudioCodec, Resolution};
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
        input_path: impl AsRef<std::path::Path>,
        output: impl AsRef<str>,
        config: download::postprocess::PostProcessConfig,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.postprocess_video_to_path(input_path, &output_path, config)
            .await
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
        input_path: impl AsRef<std::path::Path>,
        output: impl AsRef<Path>,
        config: download::postprocess::PostProcessConfig,
    ) -> Result<PathBuf> {
        let output_path = output.as_ref().to_path_buf();

        metadata::postprocess::apply_postprocess(
            input_path,
            &output_path,
            &config,
            &self.libraries,
            self.timeout,
        )
        .await
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
    /// let youtube = Downloader::builder(libs, "output").build().await?;
    /// let mut stream = youtube.event_stream();
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
        self.event_bus.subscribe()
    }

    /// Returns the number of active event subscribers.
    pub fn event_subscriber_count(&self) -> usize {
        self.event_bus.subscriber_count()
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
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// # let mut youtube = Downloader::builder(libs, "output").build().await?;
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
    /// youtube.register_hook(MyHook).await;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    pub async fn register_hook(&mut self, hook: impl events::EventHook + 'static) {
        if let Some(ref mut registry) = self.hook_registry {
            registry.register(hook).await;
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
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// # let mut youtube = Downloader::builder(libs, "output").build().await?;
    /// let webhook = WebhookConfig::new("https://example.com/webhook")
    ///     .with_method(WebhookMethod::Post)
    ///     .with_filter(EventFilter::only_completed());
    ///
    /// youtube.register_webhook(webhook).await;
    /// # Ok(())
    /// # }
    /// # }
    /// ```
    pub async fn register_webhook(&mut self, config: events::WebhookConfig) {
        if let Some(ref mut delivery) = self.webhook_delivery {
            delivery.register(config).await;
        }
    }
}
