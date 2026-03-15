//! Generic extractor for all non-YouTube sites supported by yt-dlp.
//!
//! This extractor provides universal video downloading from 1,800+ sites
//! with optional authentication support.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;

use crate::error::Result;
use crate::extractor::{ExtractorBase, VideoExtractor};
use crate::model::Video;
use crate::model::playlist::Playlist;

/// Generic extractor for all non-YouTube sites.
///
/// This extractor provides a simple wrapper around yt-dlp that works
/// with any supported site. It includes helpers for authentication.
#[derive(Debug, Clone)]
pub struct Generic {
    executable_path: PathBuf,
    extractor_name: Option<String>,
    args: Vec<String>,
    timeout: Duration,
}

crate::extractor::impl_extractor_config!(Generic);

impl Generic {
    /// Create a new generic extractor with automatic detection.
    ///
    /// # Arguments
    ///
    /// * `executable_path` - Path to the yt-dlp executable
    ///
    /// # Returns
    ///
    /// A new Generic extractor instance
    pub fn new(executable_path: PathBuf) -> Self {
        tracing::debug!(
            executable = ?executable_path,
            "⚙️ Creating new Generic extractor"
        );

        Self {
            executable_path,
            extractor_name: None,
            args: Vec::new(),
            timeout: crate::client::DEFAULT_TIMEOUT,
        }
    }

    /// Create for specific extractor (skip detection).
    ///
    /// # Arguments
    ///
    /// * `executable_path` - Path to the yt-dlp executable
    /// * `name` - Name of the extractor to use
    ///
    /// # Returns
    ///
    /// A new Generic extractor instance for the specified extractor
    pub fn for_extractor(executable_path: PathBuf, name: String) -> Self {
        tracing::debug!(
            executable = ?executable_path,
            extractor_name = name,
            "⚙️ Creating Generic extractor for specific extractor"
        );

        Self {
            executable_path,
            extractor_name: Some(name),
            args: Vec::new(),
            timeout: crate::client::DEFAULT_TIMEOUT,
        }
    }

    /// Add extractor-specific arguments.
    ///
    /// # Arguments
    ///
    /// * `extractor` - Name of the extractor
    /// * `args` - Arguments to pass to the extractor
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_extractor_args("tiktok", "api_hostname=api-h2.tiktokv.com");
    /// ```
    pub fn with_extractor_args(&mut self, extractor: &str, args: &str) -> &mut Self {
        tracing::debug!(
            extractor = extractor,
            args = args,
            "⚙️ Adding extractor-specific arguments"
        );

        self.args.push(format!("--extractor-args={}:{}", extractor, args));
        self
    }

    /// Use credentials for sites requiring login.
    ///
    /// **Security note:** Credentials are passed via `--username` and `--password` CLI arguments,
    /// which may be visible in process listings. For sensitive environments, prefer
    /// [`with_netrc`] or [`with_cookies`] on the `Downloader` instead.
    ///
    /// # Arguments
    ///
    /// * `username` - Username for authentication
    /// * `password` - Password for authentication
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_credentials("user@email.com", "password");
    /// ```
    pub fn with_credentials(&mut self, username: &str, password: &str) -> &mut Self {
        tracing::debug!(
            has_password = !password.is_empty(),
            "⚙️ Adding credentials for authentication"
        );
        tracing::warn!(
            "Credentials passed as CLI arguments are visible in process listings — consider using netrc or cookies instead"
        );

        self.args.push(format!("--username={}", username));
        self.args.push(format!("--password={}", password));
        self
    }
}

#[async_trait]
impl ExtractorBase for Generic {
    fn executable_path(&self) -> PathBuf {
        self.executable_path.clone()
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn build_base_args(&self) -> Vec<String> {
        let mut args = vec!["--no-progress".to_string(), "--dump-single-json".to_string()];
        args.extend(self.args.clone());
        args
    }
}

#[async_trait]
impl VideoExtractor for Generic {
    async fn fetch_video(&self, url: &str) -> Result<Video> {
        tracing::debug!(
            url = url,
            extractor_name = ?self.extractor_name,
            arg_count = self.args.len(),
            "📡 Fetching video with Generic extractor"
        );
        self.log_and_fetch_video(url, "Generic").await
    }

    async fn fetch_playlist(&self, url: &str) -> Result<Playlist> {
        tracing::debug!(
            url = url,
            extractor_name = ?self.extractor_name,
            arg_count = self.args.len(),
            "📡 Fetching playlist with Generic extractor"
        );
        self.log_and_fetch_playlist(url, "Generic").await
    }

    fn name(&self) -> crate::extractor::ExtractorName {
        crate::extractor::ExtractorName::Generic(self.extractor_name.clone())
    }

    fn supports_url(&self, _url: &str) -> bool {
        // Generic extractor supports everything (will validate at runtime)
        true
    }
}
