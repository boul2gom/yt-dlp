//! Generic media downloader that supports multiple platforms.

#[cfg(feature = "cache")]
use crate::cache::{DownloadCache, VideoCache};
use crate::error::{Error, Result};
use crate::executor::Executor;
use crate::extractor::{Extractor, ExtractorConfig, ExtractorDetector};
use crate::fetcher::deps::Libraries;
use crate::fetcher::download_manager::DownloadManager;
use crate::model::Video;
use crate::utils;
use crate::utils::file_system;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Generic media downloader that supports multiple video platforms.
///
/// This is the main entry point for downloading videos from various platforms
/// including YouTube, Vimeo, Twitch, TikTok, Instagram, Twitter, and Facebook.
///
/// # Examples
///
/// ```rust, no_run
/// use yt_dlp::{MediaDownloader, extractor::ExtractorConfig};
/// use std::path::PathBuf;
/// use yt_dlp::fetcher::deps::Libraries;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let libraries_dir = PathBuf::from("libs");
///     let output_dir = PathBuf::from("output");
///     
///     let yt_dlp = libraries_dir.join("yt-dlp");
///     let ffmpeg = libraries_dir.join("ffmpeg");
///     
///     let libraries = Libraries::new(yt_dlp, ffmpeg);
///     let downloader = MediaDownloader::new(libraries, output_dir)?;
///     
///     // Download from YouTube
///     let youtube_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
///     let video_path = downloader.download_video_from_url(youtube_url, "youtube-video.mp4").await?;
///     
///     // Download from Vimeo
///     let vimeo_url = "https://vimeo.com/1084537";
///     let vimeo_path = downloader.download_video_from_url(vimeo_url, "vimeo-video.mp4").await?;
///     
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct MediaDownloader {
    /// The required libraries (yt-dlp and ffmpeg).
    pub libraries: Libraries,
    /// The directory where the video (or formats) will be downloaded.
    pub output_dir: PathBuf,
    /// The arguments to pass to 'yt-dlp'.
    pub args: Vec<String>,
    /// The timeout for command execution.
    pub timeout: Duration,
    /// The cache for video metadata.
    #[cfg(feature = "cache")]
    pub cache: Option<Arc<VideoCache>>,
    /// The cache for downloaded files.
    #[cfg(feature = "cache")]
    pub download_cache: Option<Arc<DownloadCache>>,
    /// The download manager for managing parallel downloads.
    pub download_manager: Arc<DownloadManager>,
    /// Extractor detector for URL pattern matching.
    pub extractor_detector: ExtractorDetector,
    /// Extractor-specific configuration options.
    pub extractor_config: ExtractorConfig,
}

impl fmt::Display for MediaDownloader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MediaDownloader: output_dir={:?}, args={:?}",
            self.output_dir, self.args
        )
    }
}

impl MediaDownloader {
    /// Creates a new media downloader with the given yt-dlp executable, ffmpeg executable.
    /// The output directory can be void if you only want to fetch the video information.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries (yt-dlp and ffmpeg).
    /// * `output_dir` - The directory where the video will be downloaded.
    ///
    /// # Errors
    ///
    /// This function will return an error if the parent directories of the executables and output directory could not be created.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::MediaDownloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let libraries_dir = PathBuf::from("libs");
    /// let output_dir = PathBuf::from("output");
    ///
    /// let yt_dlp = libraries_dir.join("yt-dlp");
    /// let ffmpeg = libraries_dir.join("ffmpeg");
    ///
    /// let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = MediaDownloader::new(libraries, output_dir)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        libraries: Libraries,
        output_dir: impl AsRef<Path> + std::fmt::Debug,
    ) -> Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Creating a new media downloader");

        file_system::create_parent_dir(&output_dir)?;

        let download_manager = Arc::new(DownloadManager::new());
        let extractor_detector = ExtractorDetector::new();
        let extractor_config = ExtractorConfig::default();

        Ok(Self {
            libraries,
            output_dir: output_dir.as_ref().to_path_buf(),
            args: Vec::new(),
            timeout: Duration::from_secs(60),
            #[cfg(feature = "cache")]
            cache: None,
            #[cfg(feature = "cache")]
            download_cache: None,
            download_manager,
            extractor_detector,
            extractor_config,
        })
    }

    /// Detect the extractor for a given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to analyze
    ///
    /// # Returns
    ///
    /// The detected extractor.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::{MediaDownloader, extractor::Extractor};
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = MediaDownloader::new(libraries, output_dir)?;
    ///
    /// let youtube_extractor = downloader.detect_extractor("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// assert_eq!(youtube_extractor, Extractor::YouTube);
    ///
    /// let vimeo_extractor = downloader.detect_extractor("https://vimeo.com/1084537");
    /// assert_eq!(vimeo_extractor, Extractor::Vimeo);
    /// # Ok(())
    /// # }
    /// ```
    pub fn detect_extractor(&self, url: &str) -> Extractor {
        self.extractor_detector.detect(url)
    }

    /// Check if a URL is supported by any extractor.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    ///
    /// # Returns
    ///
    /// `true` if the URL is supported, `false` otherwise.
    pub fn is_url_supported(&self, url: &str) -> bool {
        self.extractor_detector.is_supported(url)
    }

    /// Get all supported extractors.
    pub fn supported_extractors(&self) -> Vec<Extractor> {
        self.extractor_detector.supported_extractors()
    }

    /// Set extractor-specific configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The extractor configuration
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::{MediaDownloader, extractor::ExtractorConfig};
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let mut downloader = MediaDownloader::new(libraries, output_dir)?;
    ///
    /// let mut config = ExtractorConfig::default();
    /// config.youtube.skip_unavailable = true;
    /// config.vimeo.include_password_protected = false;
    ///
    /// downloader.set_extractor_config(config);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_extractor_config(&mut self, config: ExtractorConfig) {
        self.extractor_config = config;
    }

    /// Get the current extractor configuration.
    pub fn extractor_config(&self) -> &ExtractorConfig {
        &self.extractor_config
    }

    /// Set custom arguments for yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `args` - Vector of arguments to pass to yt-dlp
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::MediaDownloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let mut downloader = MediaDownloader::new(libraries, output_dir)?;
    ///
    /// // Set custom quality preferences
    /// downloader.set_args(vec![
    ///     "--format".to_string(),
    ///     "best[height<=720]".to_string(),
    /// ]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_args(&mut self, args: Vec<String>) {
        self.args = args;
    }

    /// Add arguments to the existing yt-dlp arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - Vector of arguments to add
    pub fn add_args(&mut self, mut args: Vec<String>) {
        self.args.append(&mut args);
    }

    /// Set the timeout for yt-dlp command execution.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Fetch video information from a URL.
    ///
    /// This method automatically detects the extractor based on the URL
    /// and fetches the video metadata using yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to fetch information for
    ///
    /// # Returns
    ///
    /// A `Video` struct containing all the metadata for the video.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The URL is not supported
    /// - The video information could not be fetched
    /// - The yt-dlp command fails
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::MediaDownloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = MediaDownloader::new(libraries, output_dir)?;
    ///
    /// // Fetch YouTube video info
    /// let youtube_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    /// let video = downloader.fetch_video_infos(youtube_url).await?;
    /// println!("Title: {}", video.title);
    ///
    /// // Fetch Vimeo video info
    /// let vimeo_url = "https://vimeo.com/1084537";
    /// let video = downloader.fetch_video_infos(vimeo_url).await?;
    /// println!("Title: {}", video.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_video_infos(&self, url: impl AsRef<str>) -> Result<Video> {
        let url = url.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching video information for URL: {}", url);

        // Detect the extractor for this URL
        let extractor = self.detect_extractor(url);

        #[cfg(feature = "tracing")]
        tracing::debug!("Detected extractor: {}", extractor);

        // Check cache first if enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            if let Some(cached_video) = cache.get(url) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Found cached video information for URL: {}", url);
                return Ok(cached_video);
            }
        }

        // Build yt-dlp arguments
        let download_args = vec!["--no-progress", "--dump-json", url];
        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(download_args));

        // Add extractor-specific arguments
        self.add_extractor_specific_args(&extractor, &mut final_args);

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: final_args,
        };

        let output = executor.execute().await?;
        let mut video: Video = serde_json::from_str(&output.stdout).map_err(Error::Serde)?;

        // Set the video ID on each format for caching purposes
        for format in &mut video.formats {
            format.video_id = Some(video.id.clone());
        }

        // Cache the video information if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            let _ = cache.put(url.to_string(), video.clone());
        }

        Ok(video)
    }

    /// Add extractor-specific arguments to the yt-dlp command.
    ///
    /// # Arguments
    ///
    /// * `extractor` - The detected extractor
    /// * `args` - Mutable reference to the arguments vector
    fn add_extractor_specific_args(&self, extractor: &Extractor, args: &mut Vec<String>) {
        match extractor {
            Extractor::YouTube => {
                if self.extractor_config.youtube.skip_unavailable {
                    args.push("--ignore-errors".to_string());
                }
                if !self.extractor_config.youtube.include_live {
                    args.push("--match-filter".to_string());
                    args.push("!is_live".to_string());
                }
            }
            Extractor::Vimeo => {
                if let Some(password) = &self.extractor_config.vimeo.password {
                    args.push("--video-password".to_string());
                    args.push(password.clone());
                }
            }
            Extractor::Twitch => {
                if self.extractor_config.twitch.include_chat {
                    args.push("--write-info-json".to_string());
                }
            }
            Extractor::TikTok => {
                if !self.extractor_config.tiktok.include_watermark {
                    args.push("--format".to_string());
                    args.push("best[ext=mp4]/best".to_string());
                }
            }
            Extractor::Instagram => {
                // Instagram-specific arguments can be added here
            }
            Extractor::Twitter => {
                // Twitter-specific arguments can be added here
            }
            Extractor::Facebook => {
                // Facebook-specific arguments can be added here
            }
            Extractor::Generic => {
                // No specific arguments for generic extractor
            }
        }
    }

    /// Download a video from a URL.
    ///
    /// This method fetches video information and downloads the best available format
    /// that contains both video and audio.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download
    /// * `output` - The filename for the downloaded video
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The URL is not supported
    /// - The video information could not be fetched
    /// - No suitable format is available
    /// - The download fails
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::MediaDownloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let yt_dlp = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(yt_dlp, ffmpeg);
    /// let downloader = MediaDownloader::new(libraries, output_dir)?;
    ///
    /// // Download from YouTube
    /// let youtube_url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
    /// let video_path = downloader.download_video_from_url(youtube_url, "youtube-video.mp4").await?;
    ///
    /// // Download from Vimeo
    /// let vimeo_url = "https://vimeo.com/1084537";
    /// let video_path = downloader.download_video_from_url(vimeo_url, "vimeo-video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_from_url(
        &self,
        url: impl AsRef<str> + std::fmt::Debug + Display,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> Result<PathBuf> {
        let url_str = url.as_ref();

        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video from URL: {}", url_str);

        // Detect the extractor for this URL
        let _extractor = self.detect_extractor(url_str);

        #[cfg(feature = "tracing")]
        tracing::debug!("Detected extractor: {} for URL: {}", extractor, url_str);

        // Fetch video information
        let video = self.fetch_video_infos(url_str).await?;

        // Download the video using the best available format
        self.download_video(&video, output).await
    }

    /// Download a video using video metadata.
    ///
    /// This method downloads the best available format that contains both video and audio.
    ///
    /// # Arguments
    ///
    /// * `video` - The video metadata obtained from `fetch_video_infos`
    /// * `output` - The filename for the downloaded video
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Errors
    ///
    /// This function will return an error if no suitable format is available or the download fails.
    pub async fn download_video(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video: {}", video.title);

        // Get the best format with both video and audio
        let best_format = video
            .formats
            .iter()
            .find(|f| f.format_type().is_audio_and_video())
            .or_else(|| video.best_video_format())
            .ok_or_else(|| Error::MissingFormat("video".to_string()))?;

        self.download_format(best_format, output).await
    }

    /// Download a specific format.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download
    /// * `output` - The filename for the downloaded file
    ///
    /// # Returns
    ///
    /// The path to the downloaded file.
    pub async fn download_format(
        &self,
        format: &crate::model::format::Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> Result<PathBuf> {
        let output_str = output.as_ref();
        let path = self.output_dir.join(output_str);

        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading format {} to {}", format.format_id, output_str);

        // Check cache first if enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            if let Some((_, cached_path)) = download_cache.get_by_hash(&format.format_id) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached format: {}", format.format_id);

                // Copy the file from cache to output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        // Get the download URL
        let url = format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Error::MissingUrl(format.format_id.clone()))?;

        // Use the download manager for the actual download
        let download_id = self.download_manager.enqueue(url, &path, None).await;

        // Wait for download completion
        if let Some(status) = self.download_manager.wait_for_completion(download_id).await {
            match status {
                crate::fetcher::download_manager::DownloadStatus::Completed => {
                    #[cfg(feature = "cache")]
                    if let Some(download_cache) = &self.download_cache {
                        // Cache the downloaded file
                        if let Err(_e) = download_cache
                            .put_file(&path, output_str, None, Some(format))
                            .await
                        {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("Failed to cache downloaded format: {}", _e);
                        }
                    }

                    Ok(path)
                }
                crate::fetcher::download_manager::DownloadStatus::Failed { reason } => {
                    Err(Error::Download(reason))
                }
                _ => Err(Error::Download(
                    "Download did not complete successfully".to_string(),
                )),
            }
        } else {
            Err(Error::Download("Download ID not found".to_string()))
        }
    }
}
