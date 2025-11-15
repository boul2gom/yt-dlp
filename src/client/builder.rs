//! Builder pattern for Youtube struct.
//!
//! This module provides a fluent API for constructing Youtube instances with various configurations.

use crate::client::{Libraries, Youtube};
use crate::download::manager::{DownloadManager, ManagerConfig};
use crate::error::Result;
#[cfg(feature = "cache")]
use crate::cache::{DownloadCache, VideoCache};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Builder for creating Youtube instances with a fluent API.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::YoutubeBuilder;
/// # use yt_dlp::client::deps::Libraries;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let libraries = Libraries::new("libs/yt-dlp", "libs/ffmpeg");
///
/// let youtube = YoutubeBuilder::new(libraries, PathBuf::from("output"))
///     .with_args(vec!["--no-playlist".to_string()])
///     .with_timeout(std::time::Duration::from_secs(120))
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct YoutubeBuilder {
    libraries: Libraries,
    output_dir: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    #[cfg(feature = "cache")]
    cache_dir: Option<PathBuf>,
    download_manager_config: Option<ManagerConfig>,
}

impl YoutubeBuilder {
    /// Create a new builder with required parameters.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries (yt-dlp and ffmpeg paths)
    /// * `output_dir` - The directory where videos will be downloaded
    pub fn new(libraries: Libraries, output_dir: impl Into<PathBuf>) -> Self {
        Self {
            libraries,
            output_dir: output_dir.into(),
            args: Vec::new(),
            timeout: Duration::from_secs(60),
            #[cfg(feature = "cache")]
            cache_dir: None,
            download_manager_config: None,
        }
    }

    /// Set custom arguments to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to pass to yt-dlp
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add a single argument to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to add
    pub fn add_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set the timeout for command execution.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable caching with the specified cache directory.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory to store cache files
    #[cfg(feature = "cache")]
    pub fn with_cache(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    /// Set the download manager configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The download manager configuration
    pub fn with_download_manager_config(mut self, config: ManagerConfig) -> Self {
        self.download_manager_config = Some(config);
        self
    }

    /// Set the maximum number of concurrent downloads.
    ///
    /// # Arguments
    ///
    /// * `max_concurrent` - Maximum number of concurrent downloads
    pub fn with_max_concurrent_downloads(mut self, max_concurrent: usize) -> Self {
        let mut config = self.download_manager_config.unwrap_or_default();
        config.max_concurrent_downloads = max_concurrent;
        self.download_manager_config = Some(config);
        self
    }

    /// Build the Youtube instance.
    ///
    /// This method is async because it may need to create cache directories
    /// and initialize the download manager.
    ///
    /// # Returns
    ///
    /// A configured Youtube instance ready to use.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The output directory cannot be created
    /// - The cache directories cannot be created (if caching is enabled)
    /// - The download manager cannot be initialized
    pub async fn build(self) -> Result<Youtube> {
        // Create output directory if it doesn't exist
        if !self.output_dir.exists() {
            tokio::fs::create_dir_all(&self.output_dir).await?;
        }

        // Create download manager
        let download_manager = if let Some(config) = self.download_manager_config {
            Arc::new(DownloadManager::with_config(config))
        } else {
            Arc::new(DownloadManager::new())
        };

        // Create caches if enabled
        #[cfg(feature = "cache")]
        let (cache, download_cache) = if let Some(cache_dir) = self.cache_dir {
            (
                Some(Arc::new(VideoCache::new(cache_dir.clone(), None).await?)),
                Some(Arc::new(DownloadCache::new(cache_dir, None).await?)),
            )
        } else {
            (None, None)
        };

        Ok(Youtube {
            libraries: self.libraries,
            output_dir: self.output_dir,
            args: self.args,
            timeout: self.timeout,
            #[cfg(feature = "cache")]
            cache,
            #[cfg(feature = "cache")]
            download_cache,
            download_manager,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        })
    }
}
