#![doc = include_str!("../README.md")]

use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::fetcher::deps::{Libraries, LibraryInstaller};
use crate::fetcher::download_manager::{DownloadManager, ManagerConfig};
use crate::utils::file_system;
#[cfg(feature = "cache")]
use cache::{DownloadCache, VideoCache};
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "cache")]
pub mod cache;
pub mod error;
pub mod executor;
pub mod fetcher;
pub mod metadata;
pub mod model;
pub mod utils;

// Re-export of common traits to facilitate their use
pub use model::utils::{AllTraits, CommonTraits};

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
/// # use yt_dlp::fetcher::deps::Libraries;
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
    /// The download manager for managing parallel downloads.
    pub download_manager: Arc<DownloadManager>,
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    pub fn new(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher");

        file_system::create_parent_dir(&output_dir)?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        file_system::create_parent_dir(&cache_dir)?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None)?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir, None)?;

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
            download_manager: Arc::new(download_manager),
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
    pub fn with_download_manager_config(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug,
        download_manager_config: ManagerConfig,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new video fetcher with custom download manager config");

        file_system::create_parent_dir(&output_dir)?;

        // Initialize cache in the output directory
        let cache_dir = output_dir.as_ref().join("cache");
        file_system::create_parent_dir(&cache_dir)?;
        #[cfg(feature = "cache")]
        let cache = VideoCache::new(cache_dir.clone(), None)?;
        #[cfg(feature = "cache")]
        let download_cache = DownloadCache::new(cache_dir, None)?;

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
            download_manager: Arc::new(download_manager),
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
        executables_dir: impl AsRef<Path> + std::fmt::Debug,
        output_dir: impl AsRef<Path> + std::fmt::Debug,
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
        Self::new(libraries, output_dir)
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
            .ok_or(Error::Path("Invalid audio path".to_string()))?;
        let video = video_path
            .as_ref()
            .to_str()
            .ok_or(Error::Path("Invalid video path".to_string()))?;
        let output = output_path
            .as_ref()
            .to_str()
            .ok_or(Error::Path("Invalid output path".to_string()))?;

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

        if let Some(video_id) = video_id {
            if let Some(video) = self.get_video_by_id(&video_id).await {
                #[cfg(feature = "tracing")]
                tracing::debug!("Adding metadata to combined file");

                cfg_if::cfg_if! {
                    if #[cfg(feature = "cache")] {
                        let video_format = self.find_cached_format(video_path.as_ref()).await;
                        let audio_format = self.find_cached_format(audio_path.as_ref()).await;
                    } else {
                        let video_format: Option<model::format::Format> = None;
                        let audio_format: Option<model::format::Format> = None;
                    }
                }

                // Add metadata, log error on failure, then propagate
                crate::metadata::MetadataManager::add_metadata_with_format(
                    output_path.as_ref(),
                    &video,
                    video_format.as_ref(),
                    audio_format.as_ref(),
                )
                .await
                .inspect_err(|_e| {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Failed to add metadata to combined file: {}", _e);
                })?;
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

        if let Some(id) = utils::file_system::extract_video_id(video_filename) {
            return Some(id);
        }

        let audio_filename = audio_path.as_ref().file_name()?.to_str()?;
        utils::file_system::extract_video_id(audio_filename)
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

            if let Some((cached_file, _)) = download_cache.get_by_hash(&file_hash) {
                if let Some(format_json) = cached_file.format_json {
                    if let Ok(format) = serde_json::from_str(&format_json) {
                        return Some(format);
                    }
                }
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    pub fn with_cache(
        &mut self,
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Enabling video metadata cache");

        let cache = VideoCache::new(cache_dir.as_ref(), ttl)?;
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
    pub fn with_download_cache(
        &mut self,
        cache_dir: impl AsRef<Path> + std::fmt::Debug,
        ttl: Option<u64>,
    ) -> Result<&mut Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Enabling downloaded files cache");

        let download_cache = DownloadCache::new(cache_dir.as_ref(), ttl)?;
        self.download_cache = Some(Arc::new(download_cache));
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
        priority: Option<fetcher::download_manager::DownloadPriority>,
    ) -> Result<u64> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video with priority: {}", video.id);

        // Get the best format with video and audio
        let format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .ok_or_else(|| Error::MissingFormat("audio+video".to_string()))?;

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::MissingUrl(format.format_id.clone()))?;

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
            .ok_or_else(|| Error::MissingFormat("audio+video".to_string()))?;

        // Get the URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::MissingUrl(format.format_id.clone()))?;

        // Create the output path
        let output_path = self.output_dir.join(output.as_ref());

        // Add to download queue with progress callback
        let download_id = self
            .download_manager
            .enqueue_with_progress(
                url,
                output_path,
                Some(fetcher::download_manager::DownloadPriority::Normal),
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
    ) -> Option<fetcher::download_manager::DownloadStatus> {
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
    ) -> Option<fetcher::download_manager::DownloadStatus> {
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
        video_quality: model::format_selector::VideoQuality,
        video_codec: model::format_selector::VideoCodecPreference,
        audio_quality: model::format_selector::AudioQuality,
        audio_codec: model::format_selector::AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::MissingFormat("video".to_string()))?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(audio_quality, audio_codec.clone())
            .ok_or_else(|| Error::MissingFormat("audio".to_string()))?;

        // Download video format with preferences
        let video_ext = format!("{:?}", video_format.download_info.ext);
        let video_filename = format!(
            "temp_video_{}.{}",
            utils::file_system::random_filename(8),
            video_ext
        );

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
        let audio_filename = format!(
            "temp_audio_{}.{}",
            utils::file_system::random_filename(8),
            audio_ext
        );
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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
        quality: model::format_selector::VideoQuality,
        codec: model::format_selector::VideoCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(quality, codec.clone())
            .ok_or_else(|| Error::MissingFormat("video".to_string()))?;

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
    /// # use yt_dlp::fetcher::deps::Libraries;
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
        quality: model::format_selector::AudioQuality,
        codec: model::format_selector::AudioCodecPreference,
    ) -> Result<PathBuf> {
        let video = self.fetch_video_infos(url.to_string()).await?;

        // Select audio format based on quality and codec preferences
        let audio_format = video
            .select_audio_format(quality, codec.clone())
            .ok_or_else(|| Error::MissingFormat("audio".to_string()))?;

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
}
