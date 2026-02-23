#![doc = include_str!("../README.md")]

use crate::client::deps::{Libraries, LibraryInstaller};
use crate::download::PostProcessConfig;
use crate::download::manager::ManagerConfig;
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::extractor::ExtractorName;
use crate::metadata::MetadataManager;
use crate::utils::fs;
#[cfg(feature = "cache-backend")]
use cache::{DownloadCache, PlaylistCache, VideoCache};
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

// Core modules
#[cfg(feature = "cache-backend")]
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

// Statistics and analytics
#[cfg(feature = "statistics")]
pub mod stats;

// Convenience modules
pub mod macros;
pub mod prelude;

// Re-export of common traits to facilitate their use
use crate::model::Video;
#[cfg(feature = "cache-backend")]
use crate::model::format::Format;
use crate::model::format::FormatType;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
pub use client::streams::selection::VideoSelection;
pub use model::utils::{AllTraits, CommonTraits};

// Re-export main types for easy access
pub use client::{DownloadBuilder, DownloaderBuilder};
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
/// let downloader = Downloader::builder(libraries, "output")
///     .build()
///     .await?;
///
/// // YouTube is automatically detected and optimized
/// let video = downloader.fetch_video_infos("https://youtube.com/watch?v=...".to_string()).await?;
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
    #[cfg(feature = "cache-backend")]
    pub cache: Option<Arc<VideoCache>>,
    /// The cache for downloaded files.
    #[cfg(feature = "cache-backend")]
    pub download_cache: Option<Arc<DownloadCache>>,
    /// The cache for playlist metadata.
    #[cfg(feature = "cache-backend")]
    pub playlist_cache: Option<Arc<PlaylistCache>>,
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
    /// Statistics tracker (feature: statistics).
    #[cfg(feature = "statistics")]
    pub(crate) statistics: Arc<stats::StatisticsTracker>,
}

impl Clone for Downloader {
    fn clone(&self) -> Self {
        // Create extractors with the same library paths
        let youtube_extractor = extractor::Youtube::new(self.libraries.youtube.clone());
        let generic_extractor = extractor::Generic::new(self.libraries.youtube.clone());

        Self {
            youtube_extractor,
            generic_extractor,
            libraries: self.libraries.clone(),
            output_dir: self.output_dir.clone(),
            args: self.args.clone(),
            user_agent: self.user_agent.clone(),
            timeout: self.timeout,
            proxy: self.proxy.clone(),
            #[cfg(feature = "cache-backend")]
            cache: self.cache.clone(),
            #[cfg(feature = "cache-backend")]
            download_cache: self.download_cache.clone(),
            #[cfg(feature = "cache-backend")]
            playlist_cache: self.playlist_cache.clone(),
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
            "Downloader: output_dir={:?}, args={:?}, proxy={}",
            self.output_dir,
            self.args,
            self.proxy.is_some()
        )
    }
}

/// Returns the appropriate FFmpeg audio codec argument for muxing based on container compatibility.
///
/// Uses stream copy (`"copy"`) when the audio format is natively compatible with the output
/// container (e.g., AAC/M4A into MP4, Opus/WebM into WebM, any codec into MKV).
/// Falls back to `"aac"` re-encoding otherwise.
fn audio_codec_for_mux(audio_path: &Path, output_path: &Path) -> &'static str {
    let audio_ext = audio_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let output_ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_aac = matches!(audio_ext.as_str(), "m4a" | "aac");
    let is_opus = matches!(audio_ext.as_str(), "webm" | "opus" | "ogg");

    match output_ext.as_str() {
        "mp4" | "m4a" | "mov" if is_aac => "copy",
        "webm" if is_opus => "copy",
        // Matroska supports any codec natively
        "mkv" | "mka" => "copy",
        _ => "aac",
    }
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
    pub fn download<'a>(
        &'a self,
        video: &'a Video,
        output: impl Into<PathBuf>,
    ) -> DownloadBuilder<'a> {
        DownloadBuilder::new(self, video, output)
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
    /// let downloader = Downloader::with_new_binaries(
    ///     executables_dir,
    ///     output_dir
    /// ).await?.build().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_new_binaries(
        executables_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Result<DownloaderBuilder> {
        let executables_dir: PathBuf = executables_dir.into();
        let output_dir: PathBuf = output_dir.into();

        tracing::debug!(
            executables_dir = ?executables_dir,
            output_dir = ?output_dir,
            "Creating video fetcher with binaries installation"
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
            "Checking for existing binaries"
        );

        let youtube = if youtube_exists {
            tracing::debug!("Using existing yt-dlp binary");
            youtube_path
        } else {
            tracing::debug!("Installing yt-dlp binary");
            installer.install_youtube(None).await?
        };

        let ffmpeg = if ffmpeg_exists {
            tracing::debug!("Using existing ffmpeg binary");
            ffmpeg_path
        } else {
            tracing::debug!("Installing ffmpeg binary");
            installer.install_ffmpeg(None).await?
        };

        tracing::debug!(
            youtube_path = ?youtube,
            ffmpeg_path = ?ffmpeg,
            "Binaries ready"
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
    pub fn generic_extractor(&self) -> &extractor::Generic {
        &self.generic_extractor
    }

    /// Sets the user agent for HTTP requests.
    pub fn with_user_agent(&mut self, user_agent: impl AsRef<str>) -> &mut Self {
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
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// let args = vec!["--no-progress".to_string()];
    /// downloader.with_args(args);
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
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// // Set a longer timeout for large videos
    /// downloader.with_timeout(Duration::from_secs(300));
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
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// downloader.with_arg("--no-progress");
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_arg(&mut self, arg: impl AsRef<str>) -> &mut Self {
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
    pub fn with_cookies(&mut self, path: impl AsRef<Path>) -> &mut Self {
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
    pub fn with_cookies_from_browser(&mut self, browser: impl AsRef<str>) -> &mut Self {
        let b = browser.as_ref();
        self.youtube_extractor.with_cookies_from_browser(b);
        self.generic_extractor.with_cookies_from_browser(b);
        self.args.push(format!("--cookies-from-browser={}", b));
        self
    }

    /// Use .netrc for authentication.
    ///
    /// Pushes `--netrc` to both extractors and the raw yt-dlp arg list.
    pub fn with_netrc(&mut self) -> &mut Self {
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
    /// let downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// downloader.update_downloader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_downloader(&self) -> Result<()> {
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
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::VideoSelection;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=gXtp6C-3JKo");
    /// let video = downloader.fetch_video_infos(url).await?;
    ///
    /// let audio_format = video.best_audio_format().unwrap();
    /// let audio_path = downloader.download_format(&audio_format, "audio-stream.mp3").await?;
    ///
    /// let video_format = video.worst_video_format().unwrap();
    /// let format_path = downloader.download_format(&video_format, "video-stream.mp4").await?;
    ///
    /// let output_path = downloader.combine_audio_and_video("audio-stream.mp3", "video-stream.mp4", "my-output.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn combine_audio_and_video(
        &self,
        audio_file: impl AsRef<str>,
        video_file: impl AsRef<str>,
        output_file: impl AsRef<str>,
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
        audio_file: impl Into<PathBuf>,
        video_file: impl Into<PathBuf>,
        output_file: impl Into<PathBuf>,
    ) -> Result<PathBuf> {
        let audio_path = audio_file.into();
        let video_path = video_file.into();
        let output_path = output_file.into();

        tracing::debug!(
            "Combining audio and video files {:?} and {:?}, into {:?}",
            audio_path,
            video_path,
            output_path
        );

        let operation = crate::events::PostProcessOperation::CombineStreams {
            audio_path: audio_path.clone(),
            video_path: video_path.clone(),
        };
        let start_time = std::time::Instant::now();

        self.emit_event(crate::events::DownloadEvent::PostProcessStarted {
            input_path: audio_path.clone(),
            operation: operation.clone(),
        })
        .await;

        // Perform the combination with FFmpeg
        if let Err(e) = self
            .execute_ffmpeg_combine(&audio_path, &video_path, &output_path, None)
            .await
        {
            self.emit_event(crate::events::DownloadEvent::PostProcessFailed {
                input_path: audio_path,
                operation,
                error: e.to_string(),
            })
            .await;
            return Err(e);
        }

        // Add metadata to the combined file, propagating potential errors
        if let Err(e) = self
            .add_metadata_to_combined_file(&audio_path, &video_path, &output_path)
            .await
        {
            self.emit_event(crate::events::DownloadEvent::PostProcessFailed {
                input_path: audio_path,
                operation,
                error: e.to_string(),
            })
            .await;
            return Err(e);
        }

        let duration = start_time.elapsed();
        self.emit_event(crate::events::DownloadEvent::PostProcessCompleted {
            input_path: audio_path,
            output_path: output_path.clone(),
            operation,
            duration,
        })
        .await;

        Ok(output_path)
    }

    /// Executes the FFmpeg command to combine audio and video files.
    ///
    /// Selects the audio codec automatically: uses stream copy when the audio format is
    /// natively compatible with the output container (e.g., AAC into MP4, Opus into WebM),
    /// otherwise re-encodes to AAC. Optionally embeds a pre-built FFMETADATA1 file
    /// (metadata + chapters) in the same pass when `metadata_file` is provided.
    async fn execute_ffmpeg_combine(
        &self,
        audio_path: &Path,
        video_path: &Path,
        output_path: &Path,
        metadata_file: Option<&Path>,
    ) -> Result<()> {
        let audio = audio_path
            .to_str()
            .ok_or(Error::Unknown("Invalid audio path".to_string()))?;
        let video = video_path
            .to_str()
            .ok_or(Error::Unknown("Invalid video path".to_string()))?;
        let output = output_path
            .to_str()
            .ok_or(Error::Unknown("Invalid output path".to_string()))?;

        let audio_codec = audio_codec_for_mux(audio_path, output_path);

        tracing::debug!(
            audio_path = ?audio_path,
            video_path = ?video_path,
            output_path = ?output_path,
            audio_codec = audio_codec,
            has_metadata = metadata_file.is_some(),
            ffmpeg_path = ?self.libraries.ffmpeg,
            timeout = ?self.timeout,
            "Executing FFmpeg combine operation"
        );

        let mut args = vec![
            "-i".to_string(),
            audio.to_string(),
            "-i".to_string(),
            video.to_string(),
        ];

        if let Some(meta) = metadata_file {
            let meta_str = meta
                .to_str()
                .ok_or(Error::Unknown("Invalid metadata path".to_string()))?;
            args.push("-i".to_string());
            args.push(meta_str.to_string());
        }

        // Map audio from input 0 and video from input 1 explicitly
        args.extend_from_slice(&[
            "-map".to_string(),
            "0:a".to_string(),
            "-map".to_string(),
            "1:v".to_string(),
        ]);

        if metadata_file.is_some() {
            args.extend_from_slice(&[
                "-map_metadata".to_string(),
                "2".to_string(),
                "-map_chapters".to_string(),
                "2".to_string(),
            ]);
        }

        args.extend_from_slice(&[
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            audio_codec.to_string(),
            output.to_string(),
        ]);

        tracing::debug!(
            args = ?args,
            "FFmpeg combine command arguments"
        );

        let executor = Executor::new(self.libraries.ffmpeg.clone(), args, self.timeout);

        executor.execute().await?;

        tracing::debug!(
            output_path = ?output_path,
            "FFmpeg combine operation completed successfully"
        );

        Ok(())
    }

    /// Adds metadata to the combined file by extracting the video ID and
    /// retrieving information from the original audio and video formats
    async fn add_metadata_to_combined_file(
        &self,
        audio_path: impl Into<PathBuf>,
        video_path: impl Into<PathBuf>,
        output_path: impl Into<PathBuf>,
    ) -> Result<()> {
        let audio_path: PathBuf = audio_path.into();
        let video_path: PathBuf = video_path.into();
        let output_path: PathBuf = output_path.into();

        let video_id =
            self.extract_video_id_from_file_paths(video_path.as_path(), audio_path.as_path());

        if let Some(video_id) = video_id
            && let Some(video) = self.get_video_by_id(&video_id).await
        {
            tracing::debug!("Adding metadata to combined file");

            cfg_if::cfg_if! {
                if #[cfg(feature = "cache-backend")] {
                    let video_format = self.find_cached_format(video_path.clone()).await;
                    let audio_format = self.find_cached_format(audio_path.clone()).await;

                    // Add metadata (including chapters) to the combined file with full format information
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(_e) = metadata_manager.add_metadata_with_chapters(
                        &output_path,
                        &video,
                        video_format.as_ref(),
                        audio_format.as_ref(),
                    )
                    .await
                    {
                        tracing::warn!("Failed to add metadata to combined file: {}", _e);
                    } else {
                        tracing::debug!("Successfully added metadata (including chapters) to combined file");
                    }
                } else {
                    // Without cache, we don't have format details, add basic metadata only
                    let metadata_manager = MetadataManager::with_ffmpeg_path(&self.libraries.ffmpeg);
                    if let Err(e) = metadata_manager.add_metadata(
                        output_path.as_path(),
                        &video,
                    )
                    .await
                    {
                        tracing::warn!("Failed to add basic metadata to combined file: {}", e);
                    } else {
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
        video_path: impl Into<PathBuf>,
        audio_path: impl Into<PathBuf>,
    ) -> Option<String> {
        let video_path: PathBuf = video_path.into();
        let audio_path: PathBuf = audio_path.into();

        tracing::trace!(
            video_path = ?video_path,
            audio_path = ?audio_path,
            "Extracting video ID from file paths"
        );

        let video_filename = video_path.as_path().file_name()?.to_str()?;

        if let Some(id) = fs::extract_video_id(video_filename) {
            tracing::trace!(
                video_id = %id,
                source = "video_path",
                "Video ID extracted from video filename"
            );
            return Some(id);
        }

        let audio_filename = audio_path.as_path().file_name()?.to_str()?;
        let id = fs::extract_video_id(audio_filename);

        if let Some(ref id_str) = id {
            tracing::trace!(
                video_id = %id_str,
                source = "audio_path",
                "Video ID extracted from audio filename"
            );
        } else {
            tracing::trace!("No video ID found in file paths");
        }

        id
    }

    /// Finds the format of a file in the cache if it exists
    #[cfg(feature = "cache-backend")]
    async fn find_cached_format(&self, file_path: impl Into<PathBuf>) -> Option<Format> {
        let file_path: PathBuf = file_path.into();

        tracing::trace!(
            file_path = ?file_path,
            has_cache = self.download_cache.is_some(),
            "Looking up format in download cache"
        );

        if let Some(download_cache) = &self.download_cache {
            let file_hash = match DownloadCache::calculate_file_hash(file_path.as_path()).await {
                Ok(hash) => {
                    tracing::trace!(
                        file_path = ?file_path,
                        file_hash = %hash,
                        "Calculated file hash for cache lookup"
                    );
                    hash
                }
                Err(_e) => {
                    tracing::trace!(
                        file_path = ?file_path,
                        error = %_e,
                        "Failed to calculate file hash"
                    );
                    return None;
                }
            };

            if let Some((cached_file, _)) = download_cache.get_by_hash(&file_hash).await
                && let Some(ref format_json) = cached_file.format_json
                && let Ok(format) = serde_json::from_str::<Format>(format_json)
            {
                tracing::trace!(
                    file_path = ?file_path,
                    format_id = %format.format_id,
                    "Format found in cache"
                );
                return Some(format);
            }

            tracing::trace!(
                file_path = ?file_path,
                "Format not found in cache"
            );
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
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// // Enable video metadata caching
    /// downloader.with_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache-backend")]
    pub async fn with_cache(
        &mut self,
        cache_dir: impl Into<PathBuf>,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        let cache_dir = cache_dir.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "Enabling video metadata cache"
        );

        let cache = VideoCache::new(cache_dir, ttl).await?;
        self.cache = Some(Arc::new(cache));

        tracing::debug!("Video metadata cache enabled successfully");

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
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// // Enable downloaded files caching
    /// downloader.with_download_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache-backend")]
    pub async fn with_download_cache(
        &mut self,
        cache_dir: impl Into<PathBuf>,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        let cache_dir = cache_dir.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "Enabling downloaded files cache"
        );

        let download_cache = DownloadCache::new(cache_dir, ttl).await?;
        self.download_cache = Some(Arc::new(download_cache));

        tracing::debug!("Downloaded files cache enabled successfully");

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
    /// let mut downloader = Downloader::builder(libraries, output_dir)
    ///     .build()
    ///     .await?;
    ///
    /// // Enable playlist metadata caching
    /// downloader.with_playlist_cache(PathBuf::from("cache"), None).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cache-backend")]
    pub async fn with_playlist_cache(
        &mut self,
        cache_dir: impl Into<PathBuf>,
        ttl: Option<i64>,
    ) -> Result<&mut Self> {
        let cache_dir = cache_dir.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            ttl = ?ttl,
            "Enabling playlist metadata cache"
        );

        let db_path = cache_dir.join("playlists.db");

        tracing::trace!(
            db_path = ?db_path,
            "Playlist cache database path"
        );

        let playlist_cache = if let Some(ttl_seconds) = ttl {
            PlaylistCache::with_ttl(db_path, ttl_seconds as u64).await?
        } else {
            PlaylistCache::new(db_path).await?
        };
        self.playlist_cache = Some(Arc::new(playlist_cache));

        tracing::debug!("Playlist metadata cache enabled successfully");

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
            video_id = %video.id,
            video_title = %video.title,
            output_path = ?output_path,
            priority = ?priority,
            "Downloading video with priority"
        );

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

        tracing::debug!(
            video_id = %video.id,
            format_id = %format.format_id,
            format_type = ?format.format_type(),
            "Selected format for download"
        );

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: format.format_id.clone(),
            })?;

        // Add to download queue
        let download_id = self
            .download_manager
            .enqueue(url, output_path, priority)
            .await;

        tracing::debug!(
            video_id = %video.id,
            download_id = download_id,
            "Video added to download queue"
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
            video_id = %video.id,
            video_title = %video.title,
            output_path = ?output_path,
            "Downloading video with progress tracking"
        );

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

        tracing::debug!(
            video_id = %video.id,
            format_id = %format.format_id,
            format_type = ?format.format_type(),
            "Selected format for download with progress"
        );

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: format.format_id.clone(),
            })?;

        // Add to download queue with progress callback
        let download_id = self
            .download_manager
            .enqueue_with_progress(
                url,
                output_path,
                Some(DownloadPriority::Normal),
                progress_callback,
            )
            .await;

        tracing::debug!(
            video_id = %video.id,
            download_id = download_id,
            "Video added to download queue with progress tracking"
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

    /// Downloads a video (video + audio combined) with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `video_quality` - The desired video quality.
    /// * `video_codec` - The preferred video codec.
    /// * `audio_quality` - The desired audio quality.
    /// * `audio_codec` - The preferred audio codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference, AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// let path = downloader.download_video_with_quality(
    ///     &video,
    ///     "my-video.mp4",
    ///     VideoQuality::High,
    ///     VideoCodecPreference::VP9,
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_with_quality_to_path(
            video,
            output_path,
            video_quality,
            video_codec,
            audio_quality,
            audio_codec,
        )
        .await
    }

    /// Downloads a video with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_with_quality`](Self::download_video_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `video_quality` - The desired video quality.
    /// * `video_codec` - The preferred video codec.
    /// * `audio_quality` - The desired audio quality.
    /// * `audio_codec` - The preferred audio codec.
    pub async fn download_video_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        self.download_video_with_quality_to_path_inner(
            video,
            output,
            video_quality,
            video_codec,
            audio_quality,
            audio_codec,
        )
        .await
    }

    /// Internal implementation for download_video_with_quality_to_path.
    /// Separated to allow retry on 403 wrapping without duplicating the complex body.
    async fn download_video_with_quality_to_path_inner(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        tracing::debug!(
            video_id = %video.id,
            video_title = %video.title,
            video_quality = ?video_quality,
            video_codec = ?video_codec,
            audio_quality = ?audio_quality,
            audio_codec = ?audio_codec,
            "Selecting formats based on quality preferences"
        );

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        tracing::debug!(
            video_id = %video.id,
            format_id = %video_format.format_id,
            width = ?video_format.video_resolution.width,
            height = ?video_format.video_resolution.height,
            codec = ?video_format.codec_info.video_codec,
            "Selected video format"
        );

        let output_path: PathBuf = output.into();

        // When no explicit codec preference, prefer a codec that is natively compatible
        // with the output container to avoid re-encoding during muxing.
        // For example, AAC audio can be stream-copied into MP4 without re-encoding,
        // while Opus requires an expensive software transcode to AAC (~50x real-time).
        let preferred_audio_codec = if audio_codec == AudioCodecPreference::Any {
            let ext = output_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            match ext.as_str() {
                "mp4" | "m4a" | "mov" => Some(AudioCodecPreference::AAC),
                "webm" => Some(AudioCodecPreference::Opus),
                _ => None,
            }
        } else {
            None
        };

        // Select audio format: try the container-compatible codec first, fall back to any
        let audio_format = if let Some(pref) = preferred_audio_codec {
            tracing::debug!(
                video_id = %video.id,
                preferred_codec = ?pref,
                output_ext = ?output_path.extension(),
                "Trying container-compatible audio codec to avoid re-encoding"
            );
            video.select_audio_format(audio_quality, pref).or_else(|| {
                tracing::debug!(
                    video_id = %video.id,
                    "Container-compatible audio not available, falling back to any codec"
                );
                video.select_audio_format(audio_quality, AudioCodecPreference::Any)
            })
        } else {
            video.select_audio_format(audio_quality, audio_codec)
        }
        .ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Audio,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        tracing::debug!(
            video_id = %video.id,
            format_id = %audio_format.format_id,
            bitrate = ?audio_format.rates_info.audio_rate,
            codec = ?audio_format.codec_info.audio_codec,
            "Selected audio format"
        );

        // Download and combine formats, embedding metadata in a single ffmpeg pass
        self.download_and_combine_with_meta(video, video_format, audio_format, &output_path)
            .await
    }

    /// Downloads a video stream (video-only) with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `quality` - The desired video quality.
    /// * `codec` - The preferred video codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// // Download a medium quality video with AVC1 codec
    /// let video_path = downloader.download_video_stream_with_quality(
    ///     &video,
    ///     "video-only.mp4",
    ///     VideoQuality::Medium,
    ///     VideoCodecPreference::AVC1
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_stream_with_quality_to_path(video, output_path, quality, codec)
            .await
    }

    /// Downloads a video stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_stream_with_quality`](Self::download_video_stream_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `quality` - The desired video quality.
    /// * `codec` - The preferred video codec.
    pub async fn download_video_stream_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        let output: PathBuf = output.into();

        tracing::debug!(
            video_id = %video.id,
            output = ?output,
            quality = ?quality,
            codec = ?codec,
            "Downloading video stream with quality preferences"
        );

        let video_format =
            video
                .select_video_format(quality, codec)
                .ok_or_else(|| Error::FormatNotAvailable {
                    video_id: video.id.clone(),
                    format_type: FormatType::Video,
                    available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
                })?;

        self.download_format_to_path(video_format, &output).await
    }

    /// Downloads an audio stream with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `quality` - The desired audio quality.
    /// * `codec` - The preferred audio codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded audio file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// let audio_path = downloader.download_audio_stream_with_quality(
    ///     &video,
    ///     "audio-only.mp3",
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_audio_stream_with_quality_to_path(video, output_path, quality, codec)
            .await
    }

    /// Downloads an audio stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_audio_stream_with_quality`](Self::download_audio_stream_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `quality` - The desired audio quality.
    /// * `codec` - The preferred audio codec.
    pub async fn download_audio_stream_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let output: PathBuf = output.into();

        tracing::debug!(
            video_id = %video.id,
            output = ?output,
            quality = ?quality,
            codec = ?codec,
            "Downloading audio stream with quality preferences"
        );

        let audio_format =
            video
                .select_audio_format(quality, codec)
                .ok_or_else(|| Error::FormatNotAvailable {
                    video_id: video.id.clone(),
                    format_type: FormatType::Audio,
                    available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
                })?;

        self.download_format_to_path(audio_format, &output).await
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
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let extractor = downloader.detect_extractor("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    /// println!("Extractor: {}", extractor);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect_extractor(&self, url: &str) -> Result<ExtractorName> {
        tracing::debug!(
            url = %url,
            "Detecting extractor for URL"
        );

        let extractor =
            extractor::detector::detect_extractor_type(url, &self.libraries.youtube).await?;

        tracing::debug!(
            url = %url,
            extractor = ?extractor,
            "Extractor detected"
        );

        Ok(extractor)
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

        tracing::debug!(
            url = %url_str,
            "Fetching video info (fluent API)"
        );

        let video = self.fetch_video_infos(url_str.to_string()).await?;

        tracing::debug!(
            video_id = %video.id,
            video_title = %video.title,
            "Video info fetched successfully (fluent API)"
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
    /// downloader.download_and_continue(&video, "output.mp4")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_and_continue(
        self,
        video: &Video,
        output: impl AsRef<str>,
    ) -> Result<Self> {
        tracing::debug!(
            video_id = %video.id,
            video_title = %video.title,
            output = %output.as_ref(),
            "Downloading video (fluent API)"
        );

        self.download_video(video, output).await?;

        tracing::debug!(
            video_id = %video.id,
            "Video downloaded successfully (fluent API)"
        );

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
        video: &Video,
        output: impl Into<PathBuf>,
    ) -> Result<Self> {
        let output_path = output.into();

        tracing::debug!(
            video_id = %video.id,
            video_title = %video.title,
            output = ?output_path,
            "Downloading video to path (fluent API)"
        );

        self.download_video_to_path(video, output_path).await?;

        tracing::debug!(
            video_id = %video.id,
            "Video downloaded to path successfully (fluent API)"
        );

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
    ///     .pipeline("https://youtube.com/watch?v=gXtp6C-3JKo", |yt, video| async move {
    ///         yt.download_video(&video, "video.mp4").await?;
    ///         Ok(yt)
    ///     })
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

        tracing::debug!(
            url = %url_str,
            "Starting pipeline operation"
        );

        let video = self.fetch_video_infos(url_str).await?;

        tracing::debug!(
            video_id = %video.id,
            video_title = %video.title,
            "Video fetched, executing pipeline operation"
        );

        let result = operation(self, video).await?;

        tracing::debug!("Pipeline operation completed successfully");

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
        input_path: impl Into<PathBuf>,
        output: impl AsRef<str>,
        config: download::postprocess::PostProcessConfig,
    ) -> Result<PathBuf> {
        let input = input_path.into();
        let output_path = self.output_dir.join(output.as_ref());

        tracing::debug!(
            input = ?input,
            output = ?output_path,
            video_codec = ?config.video_codec,
            audio_codec = ?config.audio_codec,
            "Applying post-processing to video"
        );

        self.postprocess_video_to_path(input, output_path, config)
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
            "Applying post-processing to video file"
        );

        let result = metadata::postprocess::apply_postprocess(
            input_path,
            output_path,
            &config,
            &self.libraries,
            self.timeout,
        )
        .await?;

        tracing::debug!(
            output = ?result,
            "Post-processing completed successfully"
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
            "Creating event stream"
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
            "Creating event subscription"
        );

        let receiver = self.event_bus.subscribe();

        tracing::debug!(
            subscriber_count = self.event_bus.subscriber_count(),
            "Event subscription created"
        );

        receiver
    }

    /// Returns the number of active event subscribers.
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
    pub fn statistics(&self) -> &stats::StatisticsTracker {
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
        tracing::debug!(
            has_registry = self.hook_registry.is_some(),
            "Registering event hook"
        );

        if let Some(ref mut registry) = self.hook_registry {
            registry.register(hook).await;

            tracing::debug!("Event hook registered successfully");
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
            url = %config.url(),
            has_delivery = self.webhook_delivery.is_some(),
            "Registering webhook"
        );

        if let Some(ref mut delivery) = self.webhook_delivery {
            delivery.register(config).await;

            tracing::debug!("Webhook registered successfully");
        } else {
            tracing::warn!("Webhook delivery not available, webhook not registered");
        }
    }
}
