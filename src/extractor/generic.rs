//! Generic extractor for all non-YouTube sites supported by yt-dlp.
//!
//! This extractor provides universal video downloading from 1,800+ sites
//! with optional authentication support.

use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::Result;
use crate::extractor::VideoExtractor;
use crate::model::Video;
use crate::model::playlist::Playlist;

/// Generic extractor for all non-YouTube sites.
///
/// This extractor provides a simple wrapper around yt-dlp that works
/// with any supported site. It includes helpers for authentication.
#[derive(Debug)]
pub struct Generic {
    executable_path: PathBuf,
    extractor_name: Option<String>,
    args: Vec<String>,
    timeout: Duration,
}

impl Generic {
    /// Create a new generic extractor with automatic detection.
    ///
    /// # Arguments
    /// * `executable_path` - Path to the yt-dlp executable
    pub fn new(executable_path: PathBuf) -> Self {
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
    /// * `executable_path` - Path to the yt-dlp executable
    /// * `name` - Name of the extractor to use
    pub fn for_extractor(executable_path: PathBuf, name: String) -> Self {
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
    /// * `extractor` - Name of the extractor
    /// * `args` - Arguments to pass to the extractor
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_extractor_args("tiktok", "api_hostname=api-h2.tiktokv.com");
    /// ```
    pub fn with_extractor_args(&mut self, extractor: &str, args: &str) -> &mut Self {
        self.args
            .push(format!("--extractor-args={}:{}", extractor, args));
        self
    }

    /// Enable cookies for authentication.
    ///
    /// # Arguments
    /// * `cookie_file` - Path to the cookie file
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_cookies("instagram_cookies.txt");
    /// ```
    pub fn with_cookies(&mut self, cookie_file: &str) -> &mut Self {
        self.args.push(format!("--cookies={}", cookie_file));
        self
    }

    /// Use credentials for sites requiring login.
    ///
    /// # Arguments
    /// * `username` - Username for authentication
    /// * `password` - Password for authentication
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_credentials("user@email.com", "password");
    /// ```
    pub fn with_credentials(&mut self, username: &str, password: &str) -> &mut Self {
        self.args.push(format!("--username={}", username));
        self.args.push(format!("--password={}", password));
        self
    }

    /// Use .netrc for authentication.
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use yt_dlp::extractor::Generic;
    /// # use std::path::PathBuf;
    /// let mut extractor = Generic::new(PathBuf::from("yt-dlp"));
    /// extractor.with_netrc();
    /// ```
    pub fn with_netrc(&mut self) -> &mut Self {
        self.args.push("--netrc".to_string());
        self
    }

    /// Add custom argument to yt-dlp.
    ///
    /// # Arguments
    /// * `arg` - The argument to add
    pub fn with_arg(&mut self, arg: String) -> &mut Self {
        self.args.push(arg);
        self
    }

    /// Set timeout for yt-dlp operations.
    ///
    /// # Arguments
    /// * `timeout` - The timeout duration
    pub fn with_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    fn build_base_args(&self) -> Vec<String> {
        let mut args = vec!["--no-progress".to_string(), "--dump-json".to_string()];
        args.extend(self.args.clone());
        args
    }

    async fn execute_for_video(&self, args: &[String]) -> Result<Video> {
        super::execute_and_parse_video(self.executable_path.clone(), args, self.timeout).await
    }

    async fn execute_for_playlist(&self, args: &[String]) -> Result<Playlist> {
        super::execute_and_parse_playlist(self.executable_path.clone(), args, self.timeout).await
    }
}

#[async_trait]
impl VideoExtractor for Generic {
    async fn fetch_video(&self, url: &str) -> Result<Video> {
        let mut args = self.build_base_args();
        args.push(url.to_string());

        self.execute_for_video(&args).await
    }

    async fn fetch_playlist(&self, url: &str) -> Result<Playlist> {
        let mut args = vec![
            "--flat-playlist".to_string(),
            "--dump-json".to_string(),
            "--no-progress".to_string(),
        ];

        args.extend(self.args.clone());
        args.push(url.to_string());

        self.execute_for_playlist(&args).await
    }

    fn name(&self) -> crate::extractor::ExtractorName {
        crate::extractor::ExtractorName::Generic(self.extractor_name.clone())
    }

    fn supports_url(&self, _url: &str) -> bool {
        // Generic extractor supports everything (will validate at runtime)
        true
    }
}
