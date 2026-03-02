//! Builder pattern for Downloader struct.
//!
//! This module provides a fluent API for constructing Downloader instances with various configurations.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[cfg(cache)]
use crate::cache::{CacheConfig, CacheLayer};
use crate::client::proxy::ProxyConfig;
use crate::client::{Downloader, Libraries};
use crate::download::config::speed_profile::SpeedProfile;
use crate::download::manager::{DownloadManager, ManagerConfig};
use crate::error::Result;
use crate::extractor::ExtractorConfig;
#[cfg(cache)]
use crate::utils::fs;

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
    user_agent: Option<String>,
    timeout: Duration,
    proxy: Option<ProxyConfig>,
    cookies: Option<PathBuf>,
    cookies_from_browser: Option<String>,
    use_netrc: bool,
    #[cfg(cache)]
    cache_config: Option<CacheConfig>,
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

        tracing::debug!(
            output_dir = ?output_dir,
            timeout = ?crate::client::DEFAULT_TIMEOUT,
            "🔧 Creating new DownloaderBuilder"
        );

        Self {
            libraries,
            output_dir,
            args: Vec::new(),
            user_agent: None,
            timeout: crate::client::DEFAULT_TIMEOUT,
            proxy: None,
            cookies: None,
            cookies_from_browser: None,
            use_netrc: false,
            #[cfg(cache)]
            cache_config: None,
            download_manager_config: None,
        }
    }

    /// Set custom arguments to pass to yt-dlp.
    ///
    /// # Arguments
    ///
    /// * `args` - The arguments to pass to yt-dlp
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        tracing::debug!(
            args = ?args,
            arg_count = args.len(),
            "🔧 Setting custom yt-dlp arguments"
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
        tracing::debug!(
            timeout = ?timeout,
            "🔧 Setting command execution timeout"
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
        tracing::debug!(
            proxy_type = ?proxy.proxy_type(),
            proxy_url = proxy.url(),
            has_auth = proxy.username().is_some(),
            "🔧 Setting proxy configuration"
        );

        self.proxy = Some(proxy);
        self
    }

    /// Use a Netscape cookie file for authentication.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Netscape cookie file
    pub fn with_cookies(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        tracing::debug!(cookies_path = ?path, "🔧 Setting cookie file");

        self.cookies = Some(path);
        self
    }

    /// Extract cookies from a browser for authentication.
    ///
    /// # Arguments
    ///
    /// * `browser` - Browser name (e.g. `"chrome"`, `"firefox"`)
    pub fn with_cookies_from_browser(mut self, browser: impl Into<String>) -> Self {
        let browser = browser.into();
        tracing::debug!(browser = %browser, "🔧 Setting cookies from browser");

        self.cookies_from_browser = Some(browser);
        self
    }

    /// Use .netrc for authentication.
    pub fn with_netrc(mut self) -> Self {
        tracing::debug!("🔧 Enabling .netrc authentication");

        self.use_netrc = true;
        self
    }

    /// Enable caching with the specified cache directory.
    ///
    /// # Arguments
    ///
    /// * `cache_dir` - The directory to store cache files
    #[cfg(cache)]
    pub fn with_cache(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        let cache_dir = cache_dir.into();

        tracing::debug!(
            cache_dir = ?cache_dir,
            "🔧 Enabling cache with directory"
        );

        self.cache_config = Some(CacheConfig::builder().cache_dir(cache_dir).build());
        self
    }

    /// Enable caching with a full configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The cache configuration (directory, TTLs, optional Redis URL)
    #[cfg(cache)]
    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        tracing::debug!(
            config = %config,
            "🔧 Enabling cache with config"
        );

        self.cache_config = Some(config);
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
        let mut config = self.download_manager_config.take().unwrap_or_default();
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
        tracing::debug!(
            profile = ?profile,
            max_concurrent = profile.max_concurrent_downloads(),
            segment_size = profile.segment_size(),
            parallel_segments = profile.parallel_segments(),
            max_buffer_size = profile.max_buffer_size(),
            "🔧 Setting speed profile"
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

    /// Set a custom User-Agent header for HTTP requests.
    ///
    /// # Arguments
    ///
    /// * `user_agent` - The User-Agent string to use
    ///
    /// # Returns
    ///
    /// The builder with the user agent configured.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        let ua = user_agent.into();
        tracing::debug!(user_agent = ua, "🔧 Setting user agent");
        self.user_agent = Some(ua);
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
        {
            #[cfg(cache)]
            tracing::debug!(
                output_dir = ?self.output_dir,
                args_count = self.args.len(),
                timeout = ?self.timeout,
                has_proxy = self.proxy.is_some(),
                has_cache = self.cache_config.is_some(),
                "🔧 Building Downloader instance"
            );

            #[cfg(not(cache))]
            tracing::debug!(
                output_dir = ?self.output_dir,
                args_count = self.args.len(),
                timeout = ?self.timeout,
                has_proxy = self.proxy.is_some(),
                "🔧 Building Downloader instance"
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

        // Create extractors (must be mut so cookie args can be pushed)
        let mut youtube_extractor = crate::extractor::Youtube::new(self.libraries.youtube.clone());
        let mut generic_extractor = crate::extractor::Generic::new(self.libraries.youtube.clone());

        // Propagate cookie configuration to both extractors and raw args
        if let Some(ref path) = self.cookies {
            youtube_extractor.with_cookies(path);
            generic_extractor.with_cookies(path);
            args.push(format!("--cookies={}", path.display()));
        }
        if let Some(ref browser) = self.cookies_from_browser {
            youtube_extractor.with_cookies_from_browser(browser);
            generic_extractor.with_cookies_from_browser(browser);
            args.push(format!("--cookies-from-browser={}", browser));
        }
        if self.use_netrc {
            youtube_extractor.with_netrc();
            generic_extractor.with_netrc();
            args.push("--netrc".to_string());
        }

        // Create cache layer if configured
        #[cfg(cache)]
        let cache = if let Some(config) = self.cache_config {
            // Ensure cache directory exists
            if !config.cache_dir.exists() {
                fs::create_dir(&config.cache_dir).await?;
            }
            Some(Arc::new(CacheLayer::from_config(&config).await?))
        } else {
            None
        };

        #[cfg(feature = "statistics")]
        let statistics = Arc::new(crate::stats::StatisticsTracker::new(&event_bus));

        Ok(Downloader {
            youtube_extractor,
            generic_extractor,
            libraries: self.libraries,
            output_dir: self.output_dir,
            args,
            user_agent: self.user_agent,
            timeout: self.timeout,
            proxy: self.proxy,
            #[cfg(cache)]
            cache,
            download_manager,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            event_bus,
            #[cfg(feature = "hooks")]
            hook_registry: Some(crate::events::HookRegistry::new()),
            #[cfg(feature = "webhooks")]
            webhook_delivery: Some(crate::events::WebhookDelivery::new()),
            #[cfg(feature = "statistics")]
            statistics,
        })
    }
}

impl std::fmt::Display for DownloaderBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DownloaderBuilder(output_dir={}, timeout={}s, proxy={})",
            self.output_dir.display(),
            self.timeout.as_secs(),
            self.proxy.is_some()
        )
    }
}
