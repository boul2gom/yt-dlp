//! Builder pattern for Downloader struct.
//!
//! This module provides a fluent API for constructing Downloader instances with various configurations.

#[cfg(feature = "cache")]
use crate::cache::{DownloadCache, PlaylistCache, VideoCache};
use crate::client::proxy::ProxyConfig;
use crate::client::{Downloader, Libraries};
use crate::download::manager::{DownloadManager, ManagerConfig};
use crate::download::speed_profile::SpeedProfile;
use crate::error::Result;
#[cfg(feature = "cache")]
use crate::utils::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Builder for creating Downloader instances with a fluent API.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::DownloaderBuilder;
/// # use yt_dlp::client::deps::Libraries;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
///
/// let downloader = DownloaderBuilder::new(libraries, PathBuf::from("output"))
///     .with_args(vec!["--no-playlist".to_string()])
///     .with_timeout(std::time::Duration::from_secs(120))
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct DownloaderBuilder {
    libraries: Libraries,
    output_dir: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    proxy: Option<ProxyConfig>,
    #[cfg(feature = "cache")]
    cache_dir: Option<PathBuf>,
    download_manager_config: Option<ManagerConfig>,
}

impl DownloaderBuilder {
    /// Create a new builder with required parameters.
    ///
    /// # Arguments
    ///
    /// * `libraries` - The required libraries (yt-dlp and ffmpeg paths)
    /// * `output_dir` - The directory where videos will be downloaded
    pub fn new(libraries: Libraries, output_dir: impl Into<PathBuf>) -> Self {
        let output_dir = output_dir.into();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            output_dir = ?output_dir,
            timeout = ?crate::client::DEFAULT_TIMEOUT,
            "Creating new DownloaderBuilder"
        );

        Self {
            libraries,
            output_dir,
            args: Vec::new(),
            timeout: crate::client::DEFAULT_TIMEOUT,
            proxy: None,
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
        #[cfg(feature = "tracing")]
        tracing::debug!(
            args = ?args,
            arg_count = args.len(),
            "Setting custom yt-dlp arguments"
        );

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
        #[cfg(feature = "tracing")]
        tracing::debug!(
            timeout = ?timeout,
            "Setting command execution timeout"
        );

        self.timeout = timeout;
        self
    }

    /// Set proxy configuration for HTTP requests and yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `proxy` - The proxy configuration
    pub fn with_proxy(mut self, proxy: ProxyConfig) -> Self {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            proxy_type = ?proxy.proxy_type(),
            proxy_url = proxy.url(),
            has_auth = proxy.username().is_some(),
            "Setting proxy configuration"
        );

        self.proxy = Some(proxy);
        self
    }

    /// Enable caching with the specified cache directory.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory to store cache files
    #[cfg(feature = "cache")]
    pub fn with_cache(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        let cache_dir = cache_dir.into();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            cache_dir = ?cache_dir,
            "Enabling cache with directory"
        );

        self.cache_dir = Some(cache_dir);
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

    /// Set the speed profile for download optimization.
    ///
    /// This automatically configures all download parameters (concurrent downloads,
    /// parallel segments, segment size, buffer size) based on the selected profile.
    ///
    /// # Arguments
    ///
    /// * `profile` - The speed profile to use (Conservative, Balanced, or Aggressive)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::DownloaderBuilder;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::download::SpeedProfile;
    /// # use std::path::PathBuf;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    ///
    /// // Use aggressive profile for high-speed connections
    /// let downloader = DownloaderBuilder::new(libraries, PathBuf::from("output"))
    ///     .with_speed_profile(SpeedProfile::Aggressive)
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_speed_profile(mut self, profile: SpeedProfile) -> Self {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            profile = ?profile,
            max_concurrent = profile.max_concurrent_downloads(),
            segment_size = profile.segment_size(),
            parallel_segments = profile.parallel_segments(),
            max_buffer_size = profile.max_buffer_size(),
            "Setting speed profile"
        );

        if let Some(config) = &mut self.download_manager_config {
            config.max_concurrent_downloads = profile.max_concurrent_downloads();
            config.segment_size = profile.segment_size();
            config.parallel_segments = profile.parallel_segments();
            config.max_buffer_size = profile.max_buffer_size();
            config.speed_profile = profile;
        } else {
            self.download_manager_config = Some(ManagerConfig::from_speed_profile(profile));
        }
        self
    }

    /// Build the Downloader instance.
    ///
    /// This method is async because it may need to create cache directories
    /// and initialize the download manager.
    ///
    /// # Returns
    ///
    /// A configured Downloader instance ready to use.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The output directory cannot be created
    /// - The cache directories cannot be created (if caching is enabled)
    /// - The download manager cannot be initialized
    pub async fn build(self) -> Result<Downloader> {
        #[cfg(feature = "tracing")]
        {
            #[cfg(feature = "cache")]
            tracing::debug!(
                output_dir = ?self.output_dir,
                args_count = self.args.len(),
                timeout = ?self.timeout,
                has_proxy = self.proxy.is_some(),
                has_cache = self.cache_dir.is_some(),
                "Building Downloader instance"
            );

            #[cfg(not(feature = "cache"))]
            tracing::debug!(
                output_dir = ?self.output_dir,
                args_count = self.args.len(),
                timeout = ?self.timeout,
                has_proxy = self.proxy.is_some(),
                "Building Downloader instance"
            );
        }

        // Create output directory if it doesn't exist
        if !self.output_dir.exists() {
            tokio::fs::create_dir_all(&self.output_dir).await?;
        }

        // Create event bus first
        let event_bus = crate::events::EventBus::with_default_capacity();

        // Create download manager with proxy configuration and event bus
        let download_manager = if let Some(mut config) = self.download_manager_config {
            config.proxy = self.proxy.clone();
            Arc::new(DownloadManager::with_config_and_event_bus(
                config,
                Some(event_bus.clone()),
            ))
        } else {
            let config = ManagerConfig {
                proxy: self.proxy.clone(),
                ..Default::default()
            };
            Arc::new(DownloadManager::with_config_and_event_bus(
                config,
                Some(event_bus.clone()),
            ))
        };

        // Add proxy argument to yt-dlp args if configured
        let mut args = self.args;
        if let Some(ref proxy) = self.proxy {
            args.push("--proxy".to_string());
            args.push(proxy.to_ytdlp_arg());
        }

        // Create caches if enabled
        #[cfg(feature = "cache")]
        let (cache, download_cache, playlist_cache) = if let Some(cache_dir) = self.cache_dir {
            // Ensure cache directory exists
            if !cache_dir.exists() {
                fs::create_dir(&cache_dir).await?;
            }
            (
                Some(Arc::new(VideoCache::new(cache_dir.clone(), None).await?)),
                Some(Arc::new(DownloadCache::new(cache_dir.clone(), None).await?)),
                Some(Arc::new(
                    PlaylistCache::new(cache_dir.join("playlists.db")).await?,
                )),
            )
        } else {
            (None, None, None)
        };

        // Create extractors
        let youtube_extractor = crate::extractor::Youtube::new(self.libraries.youtube.clone());
        let generic_extractor = crate::extractor::Generic::new(self.libraries.youtube.clone());

        Ok(Downloader {
            youtube_extractor,
            generic_extractor,
            libraries: self.libraries,
            output_dir: self.output_dir,
            args,
            user_agent: None,
            timeout: self.timeout,
            proxy: self.proxy,
            #[cfg(feature = "cache")]
            cache,
            #[cfg(feature = "cache")]
            download_cache,
            #[cfg(feature = "cache")]
            playlist_cache,
            download_manager,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            event_bus,
            #[cfg(feature = "hooks")]
            hook_registry: Some(crate::events::HookRegistry::new()),
            #[cfg(feature = "webhooks")]
            webhook_delivery: Some(crate::events::WebhookDelivery::new()),
        })
    }
}
