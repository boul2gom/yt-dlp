#![doc = include_str!("../README.md")]

use crate::client::deps::{Libraries, LibraryInstaller};
use crate::download::manager::ManagerConfig;
use crate::error::{Error, Result};
use crate::executor::Executor;
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

// Convenience modules
pub mod macros;
pub mod prelude;

// Re-export of common traits to facilitate their use
pub use model::utils::{AllTraits, CommonTraits};

// Re-export main types for easy access
pub use client::{DownloadBuilder, YoutubeBuilder};
pub use download::{DownloadManager, DownloadPriority, DownloadStatus};

/// A YouTube video fetcher that uses yt-dlp to fetch video information and download it.
///
/// The 'yt-dlp' executable and 'ffmpeg' build can be installed with this fetcher.
///
/// The video can be downloaded with or without its audio, and the audio and video can be combined.
/// The video thumbnail can also be downloaded.
///
/// The major implementations of this struct are located in the 'fetcher' module.
///
/// # Examples
///
/// ```rust, no_run
/// # use yt_dlp::Youtube;
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
/// let mut fetcher = Youtube::new(libraries, output_dir)?;
///
/// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
/// let video = fetcher.fetch_video_infos(url).await?;
/// println!("Video title: {}", video.title);
///
/// fetcher.download_video(&video, "video.mp4").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Youtube {
    /// The required libraries.
    pub libraries: Libraries,

    /// The directory where the video (or formats) will be downloaded.
    pub output_dir: PathBuf,
    /// The arguments to pass to 'yt-dlp'.
    pub args: Vec<String>,
    /// The timeout for command execution.
    pub timeout: Duration,
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
}

impl fmt::Display for Youtube {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Youtube: output_dir={:?}, args={:?}",
            self.output_dir, self.args
        )
    }
}

impl Youtube {
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
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let libraries = Libraries::new("libs/yt-dlp", "libs/ffmpeg");
    ///
    /// let youtube = Youtube::builder(libraries, "output")
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
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::selector::{VideoQuality, AudioQuality, VideoCodecPreference};
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new("libs/yt-dlp", "libs/ffmpeg");
    /// # let fetcher = Youtube::new(libraries, "output")?;
    /// let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    ///
    /// let video_path = fetcher.download(url, "my-video.mp4")
    ///     .video_quality(VideoQuality::Q1080p)
    ///     .video_codec(VideoCodecPreference::H264)
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
    /// # use yt_dlp::Youtube;
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
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher");

        fs::create_parent_dir(&output_dir)?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        fs::create_parent_dir(&cache_dir)?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let playlist_cache = PlaylistCache::new(cache_dir.join("playlists.db")).await?;

        // Initialize download manager with default configuration
        let download_manager = DownloadManager::new();

        Ok(Self {
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            timeout: Duration::from_secs(30),
            #[cfg(feature = "cache")]
            cache: Some(Arc::new(cache)),
            #[cfg(feature = "cache")]
            download_cache: Some(Arc::new(download_cache)),
            #[cfg(feature = "cache")]
            playlist_cache: Some(Arc::new(playlist_cache)),
            download_manager: Arc::new(download_manager),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
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
        output_dir: impl AsRef<Path> + std::fmt::Debug,
        download_manager_config: ManagerConfig,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher with custom download manager config");

        fs::create_parent_dir(&output_dir)?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        fs::create_parent_dir(&cache_dir)?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir.clone(), None).await?;
        #[cfg(feature = "cache")]
        let playlist_cache = PlaylistCache::new(cache_dir.join("playlists.db")).await?;

        // Initialize download manager with custom configuration
        let download_manager = DownloadManager::with_config(download_manager_config);

        Ok(Self {
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            timeout: Duration::from_secs(30),
            #[cfg(feature = "cache")]
            cache: Some(Arc::new(cache)),
            #[cfg(feature = "cache")]
            download_cache: Some(Arc::new(download_cache)),
            #[cfg(feature = "cache")]
            playlist_cache: Some(Arc::new(playlist_cache)),
            download_manager: Arc::new(download_manager),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executables_dir = PathBuf::from("libs");
    /// let output_dir = PathBuf::from("output");
    ///
    /// let fetcher = Youtube::with_new_binaries(executables_dir, output_dir).await?;
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

    /// Sets the arguments to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to pass to yt-dlp.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
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
    /// # use yt_dlp::Youtube;
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
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// fetcher.update_downloader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_downloader(&self) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Updating the downloader");

        let args = vec!["--update"];

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: utils::to_owned(args),
        };

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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
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
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Combining audio and video files {} and {}, into {}",
            audio_file,
            video_file,
            output_file
        );

        let audio_path = self.output_dir.join(audio_file.as_ref());
        let video_path = self.output_dir.join(video_file.as_ref());
        let output_path = self.output_dir.join(output_file.as_ref());

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

        let executor = Executor {
            executable_path: self.libraries.ffmpeg.clone(),
            timeout: self.timeout,
            args: utils::to_owned(args),
        };

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
                    if let Err(_e) = metadata::MetadataManager::add_metadata_with_chapters(
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
                    if let Err(e) = metadata::MetadataManager::add_metadata(
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
    async fn find_cached_format(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Option<model::format::Format> {
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// // Enable video metadata caching
    /// fetcher.with_cache(PathBuf::from("cache"), None)?;
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// // Enable downloaded files caching
    /// fetcher.with_download_cache(PathBuf::from("cache"), None)?;
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
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let mut fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// // Enable playlist metadata caching
    /// fetcher.with_playlist_cache(PathBuf::from("cache"), None)?;
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
            PlaylistCache::with_ttl(db_path, ttl_seconds).await?
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
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video with priority: {}", video.id);

        // Get the best format with video and audio
        let format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "audio+video".to_string(),
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

        // Create the output path
        let output_path = self.output_dir.join(output.as_ref());

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
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video with progress tracking: {}", video.id);

        // Get the best format with video and audio
        let format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "audio+video".to_string(),
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

        // Create the output path
        let output_path = self.output_dir.join(output.as_ref());

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
    /// # use yt_dlp::Youtube;
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
    /// # let fetcher = Youtube::new(libraries, output_dir)?;
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
        video_quality: model::selector::VideoQuality,
        video_codec: model::selector::VideoCodecPreference,
        audio_quality: model::selector::AudioQuality,
        audio_codec: model::selector::AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "video".to_string(),
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "audio".to_string(),
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
    /// # use yt_dlp::Youtube;
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
    /// # let fetcher = Youtube::new(libraries, output_dir)?;
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
        quality: model::selector::VideoQuality,
        codec: model::selector::VideoCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(quality, codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "video".to_string(),
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
    /// # use yt_dlp::Youtube;
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
    /// # let fetcher = Youtube::new(libraries, output_dir)?;
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
        quality: model::selector::AudioQuality,
        codec: model::selector::AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(quality, codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: "audio".to_string(),
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

    /// Initiates a graceful shutdown of all ongoing operations.
    ///
    /// This method triggers the cancellation token, signaling all ongoing
    /// downloads and operations to stop gracefully. It does not wait for
    /// operations to complete.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    /// let youtube = Youtube::new(libs, "output").await?;
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
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new("yt-dlp", "ffmpeg");
    /// let (youtube, video) = Youtube::builder(libs, "output")
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
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new("yt-dlp", "ffmpeg");
    /// let youtube = Youtube::builder(libs, "output")
    ///     .build()
    ///     .await?
    ///     .fetch("https://youtube.com/watch?v=dQw4w9WgXcQ")
    ///     .await?
    ///     .0
    ///     .download_and_continue(&video, "output.mp4")
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

    /// Chain multiple operations in a pipeline.
    ///
    /// This method allows you to chain fetch -> download -> metadata operations.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Youtube;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libs = Libraries::new("yt-dlp", "ffmpeg");
    /// Youtube::builder(libs, "output")
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
}
