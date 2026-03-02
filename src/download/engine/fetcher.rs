//! HTTP fetcher for downloading files with parallel segment support.
//!
//! This module provides the core HTTP fetching functionality with:
//! - Parallel segment downloads
//! - Connection pooling
//! - Retry logic with exponential backoff
//! - Progress tracking

use std::cmp::min;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::header::{HeaderMap, HeaderValue};

use crate::client::proxy::ProxyConfig;
use crate::download::config::speed_profile::SpeedProfile;
use crate::download::types::ProgressCallback;
use crate::error::{Error, Result};
use crate::model::format::HttpHeaders;
use crate::utils::fs;
use crate::utils::retry::RetryPolicy;

// Download configuration constants
const DEFAULT_PARALLEL_SEGMENTS: usize = 4;
const DEFAULT_SEGMENT_SIZE: usize = 5 * 1024 * 1024; // 5 MB
const DEFAULT_RETRY_ATTEMPTS: usize = 3;

/// The fetcher is responsible for downloading data from a URL.
/// This optimized implementation uses parallel downloads, download resumption,
/// and connection pooling for optimal performance.
pub struct Fetcher {
    /// The URL from which to download the data.
    pub(super) url: String,
    /// The number of parallel segments to use for downloading.
    /// A higher value can improve performance but consumes more resources.
    pub(super) parallel_segments: usize,
    /// The size of each segment in bytes.
    pub(super) segment_size: usize,
    /// The number of download attempts in case of failure.
    pub(super) retry_attempts: usize,
    /// Retry policy with exponential backoff for HTTP requests.
    pub(super) retry_policy: RetryPolicy,
    /// Shared HTTP client with connection pooling for efficient request handling.
    pub(super) client: Arc<reqwest::Client>,
    /// Per-request headers applied on top of the shared client's defaults.
    pub(super) extra_headers: Option<reqwest::header::HeaderMap>,
    /// Callback optional for tracking download progress
    pub(super) progress_callback: Option<ProgressCallback>,
    /// Speed profile for optimizing download parameters
    pub(super) speed_profile: SpeedProfile,
    /// Optional byte-range constraint: only download `[start, end]` from the URL.
    ///
    /// When set, [`fetch_asset`] delegates to [`fetch_asset_range`] and writes the
    /// sub-range from offset 0 in the destination file.
    pub(super) range_constraint: Option<(u64, u64)>,
}

impl fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fetcher")
            .field("url", &self.url)
            .field("parallel_segments", &self.parallel_segments)
            .field("segment_size", &self.segment_size)
            .field("retry_attempts", &self.retry_attempts)
            .field("speed_profile", &self.speed_profile)
            .field("range_constraint", &self.range_constraint)
            .field("has_callback", &self.progress_callback.is_some())
            .finish()
    }
}

impl fmt::Display for Fetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Fetcher(url={}, segments={}, profile={}, range={:?})",
            self.url, self.parallel_segments, self.speed_profile, self.range_constraint
        )
    }
}

/// RAII guard that removes the `.parts` tracking file on drop unless `commit()` is called.
pub(super) struct PartsGuard {
    path: PathBuf,
    keep: bool,
}

impl PartsGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    pub(super) fn commit(&mut self) {
        self.keep = true;
    }
}

impl Drop for PartsGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Fetcher {
    /// Creates a new fetcher for the given URL.
    ///
    /// The fetcher uses a shared HTTP client with connection pooling for optimal performance.
    /// Connections are kept alive and reused across multiple requests.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL from which to download the data.
    /// * `proxy` - Optional proxy configuration
    /// * `http_headers` - Optional HTTP headers
    pub fn new(url: impl AsRef<str>, proxy: Option<&ProxyConfig>, http_headers: Option<HttpHeaders>) -> Result<Self> {
        tracing::debug!(
            url = %url.as_ref(),
            has_proxy = proxy.is_some(),
            has_headers = http_headers.is_some(),
            "⚙️ Creating fetcher"
        );

        let (user_agent, default_headers) = match &http_headers {
            Some(headers) => (Some(headers.user_agent.clone()), Some(headers.to_header_map())),
            None => (None, None),
        };

        let client = crate::utils::http::build_http_client(crate::utils::http::HttpClientConfig {
            proxy,
            user_agent,
            default_headers,
            http2_adaptive_window: true,
            ..Default::default()
        })?;

        Ok(Self::with_client(url, client))
    }

    /// Creates a new fetcher reusing an existing HTTP client.
    ///
    /// This avoids the cost of building a new connection pool, TLS session cache,
    /// and DNS resolver for every download. Prefer this over [`Fetcher::new`] when
    /// a shared client is available.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL from which to download the data.
    /// * `client` - A shared HTTP client with connection pooling.
    pub fn with_client(url: impl AsRef<str>, client: Arc<reqwest::Client>) -> Self {
        tracing::debug!(
            url = %url.as_ref(),
            "⚙️ Creating fetcher with custom client"
        );

        Self {
            url: url.as_ref().to_string(),
            parallel_segments: DEFAULT_PARALLEL_SEGMENTS,
            segment_size: DEFAULT_SEGMENT_SIZE,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_policy: RetryPolicy::default(),
            client,
            extra_headers: None,
            progress_callback: None,
            speed_profile: SpeedProfile::default(),
            range_constraint: None,
        }
    }

    /// Creates a new fetcher reusing an existing HTTP client with per-request headers.
    ///
    /// This preserves the shared connection pool while applying format-specific headers
    /// (User-Agent, cookies) to each request.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL from which to download the data.
    /// * `client` - A shared HTTP client with connection pooling.
    /// * `headers` - Format-specific HTTP headers to apply per-request.
    pub fn with_client_and_headers(
        url: impl AsRef<str>,
        client: Arc<reqwest::Client>,
        headers: crate::model::format::HttpHeaders,
    ) -> Self {
        let mut header_map = headers.to_header_map();
        if let Ok(ua) = reqwest::header::HeaderValue::from_str(&headers.user_agent) {
            header_map.insert(reqwest::header::USER_AGENT, ua);
        }

        let mut fetcher = Self::with_client(url, client);
        fetcher.extra_headers = Some(header_map);
        fetcher
    }

    /// Configures the number of parallel segments for downloading.
    ///
    /// # Arguments
    ///
    /// * `segments` - The number of parallel segments to use.
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn with_parallel_segments(mut self, segments: usize) -> Self {
        tracing::debug!(
            segments = segments,
            url = %self.url,
            "⚙️ Configuring parallel segments for fetcher"
        );

        self.parallel_segments = segments;
        self
    }

    /// Configures the size of each segment in bytes.
    ///
    /// # Arguments
    ///
    /// * `size` - The size of each segment in bytes.
    pub fn with_segment_size(mut self, size: usize) -> Self {
        self.segment_size = size;
        self
    }

    /// Configures the number of download attempts in case of failure.
    ///
    /// # Arguments
    ///
    /// * `attempts` - The number of attempts.
    pub fn with_retry_attempts(mut self, attempts: usize) -> Self {
        self.retry_attempts = attempts;
        self
    }

    /// Configure a callback for tracking download progress.
    ///
    /// # Arguments
    ///
    /// * `callback` - A function that will be called with the downloaded size and total size.
    pub fn with_progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
        self
    }

    /// Configure the speed profile for automatic optimization
    ///
    /// This will automatically adjust segment size and parallel segments
    /// based on the profile settings during download.
    ///
    /// # Arguments
    ///
    /// * `profile` - The speed profile to use
    pub fn with_speed_profile(mut self, profile: SpeedProfile) -> Self {
        self.speed_profile = profile;
        self
    }

    /// Constrains the download to `[start, end]` bytes of the URL.
    ///
    /// When set, [`fetch_asset`] downloads only those bytes and writes them
    /// starting from offset 0 in the destination file. HTTP requests still use
    /// absolute `Range: bytes=start-end` headers against the URL.
    ///
    /// # Arguments
    ///
    /// * `start` - First byte to download (URL-absolute, inclusive).
    /// * `end` - Last byte to download (URL-absolute, inclusive).
    pub fn with_range(mut self, start: u64, end: u64) -> Self {
        self.range_constraint = Some((start, end));
        self
    }

    /// Fetch the data from the URL and return it as Serde value.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - An optional authentication token to use for the request.
    ///
    /// # Errors
    ///
    /// This function will return an error if the data could not be fetched or parsed.
    pub async fn fetch_json(&self, auth_token: Option<String>) -> Result<serde_json::Value> {
        let response = self.fetch_internal(auth_token).await?;
        let json = response.json().await?;
        Ok(json)
    }

    /// Fetch the data from the URL and return it as text.
    ///
    /// # Arguments
    ///
    /// * `auth_token` - An optional authentication token to use for the request.
    ///
    /// # Errors
    ///
    /// This function will return an error if the data could not be fetched.
    pub async fn fetch_text(&self, auth_token: Option<String>) -> Result<String> {
        let response = self.fetch_internal(auth_token).await?;
        let text = response.text().await?;
        Ok(text)
    }

    /// Fetch the data from the URL and return it as a reqwest response.
    async fn fetch_internal(&self, auth_token: Option<String>) -> Result<reqwest::Response> {
        tracing::debug!(
            url = %self.url,
            has_token = auth_token.is_some(),
            "📥 Fetching data"
        );

        let mut headers = HeaderMap::new();

        if let Some(auth_token) = auth_token {
            let value = HeaderValue::from_str(&format!("Bearer {}", auth_token)).map_err(|e| Error::InvalidHeader {
                header: "Authorization".to_string(),
                reason: e.to_string(),
            })?;

            headers.insert(reqwest::header::AUTHORIZATION, value);
        }

        let response = self
            .client
            .get(&self.url)
            .headers(headers)
            .send()
            .await?
            .error_for_status()?;

        Ok(response)
    }

    /// Downloads the asset at the given URL and writes it to the given destination.
    /// This optimized method uses parallel downloads and download resumption.
    ///
    /// # Arguments
    ///
    /// * `destination` - The path where to write the asset.
    ///
    /// # Errors
    ///
    /// This function will return an error if the asset cannot be downloaded or written to the destination.
    pub async fn fetch_asset(&self, destination: impl Into<PathBuf>) -> Result<()> {
        let destination: PathBuf = destination.into();

        // Delegate to range variant when a byte constraint is configured
        if let Some((start, end)) = self.range_constraint {
            return self.fetch_asset_range(destination, start, end).await;
        }

        tracing::debug!(
            url = %self.url,
            destination = ?destination,
            parallel_segments = self.parallel_segments,
            segment_size = self.segment_size,
            "📥 Fetching asset to file"
        );

        // Ensure the destination directory exists
        fs::create_parent_dir(&destination).await?;

        // If the parent directory doesn't exist, create it
        if let Some(parent) = destination.parent()
            && !parent.exists()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check if the file exists and if we can resume the download
        let file_exists = destination.as_path().exists();
        let file_size = if file_exists {
            match tokio::fs::metadata(&destination).await {
                Ok(metadata) => Some(metadata.len()),
                Err(_) => None,
            }
        } else {
            None
        };

        // Probe server capabilities for range downloads
        let (supports_ranges, content_length) = self.probe_range_support().await?;
        if !supports_ranges {
            return self.fetch_asset_simple(destination).await;
        }
        let Some(content_length) = content_length else {
            return self.fetch_asset_simple(destination).await;
        };

        // If the file exists and has the same size, it is already downloaded
        if file_size.is_some_and(|size| size == content_length) {
            tracing::debug!(
                destination = ?destination,
                size = content_length,
                "✅ File already exists with correct size, skipping download"
            );
            return Ok(());
        }

        let file = Arc::new(self.open_download_file(&destination, file_size, content_length).await?);

        let segment_size = self.segment_size as u64;
        let ranges: Vec<(u64, u64)> = (0..content_length.div_ceil(segment_size))
            .map(|i| {
                let start = i * segment_size;
                let end = min(start + segment_size - 1, content_length - 1);
                (start, end)
            })
            .collect();

        self.run_parallel_segments(file, file_exists, ranges, 0, content_length, &destination)
            .await
    }

    /// Downloads only `[byte_start, byte_end]` from the URL and writes them from offset 0
    /// in `destination`.
    ///
    /// Skips the `probe_range_support` call — range support is assumed to be confirmed by
    /// the caller (e.g. `media_seek` already validated it during container parsing).
    /// The file is pre-allocated to `byte_end - byte_start + 1` bytes and segments are
    /// downloaded in parallel using the same machinery as [`fetch_asset`].
    ///
    /// # Arguments
    ///
    /// * `destination` - Path where the sub-range bytes are written (starting at offset 0).
    /// * `byte_start` - First byte to download (URL-absolute, inclusive).
    /// * `byte_end` - Last byte to download (URL-absolute, inclusive).
    ///
    /// # Errors
    ///
    /// Returns an error if a segment download fails after all retry attempts or if the
    /// destination file cannot be created.
    pub(crate) async fn fetch_asset_range(
        &self,
        destination: impl Into<PathBuf>,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<()> {
        let destination: PathBuf = destination.into();
        let range_len = byte_end - byte_start + 1;

        tracing::debug!(
            url = %self.url,
            destination = ?destination,
            byte_start,
            byte_end,
            range_len,
            parallel_segments = self.parallel_segments,
            segment_size = self.segment_size,
            "📥 Fetching asset range to file"
        );

        fs::create_parent_dir(&destination).await?;
        if let Some(parent) = destination.parent()
            && !parent.exists()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check for an existing partial download of this range
        let file_exists = destination.as_path().exists();
        let file_size = if file_exists {
            tokio::fs::metadata(&destination).await.ok().map(|m| m.len())
        } else {
            None
        };

        // If the file already has the exact expected size, the range was already downloaded
        if file_size.is_some_and(|size| size == range_len) {
            tracing::debug!(
                destination = ?destination,
                size = range_len,
                "✅ Range already downloaded with correct size, skipping"
            );
            return Ok(());
        }

        let file = Arc::new(self.open_download_file(&destination, file_size, range_len).await?);

        // Segments use URL-absolute offsets; file writes are remapped via file_offset_base
        let segment_size = self.segment_size as u64;
        let ranges: Vec<(u64, u64)> = (0..range_len.div_ceil(segment_size))
            .map(|i| {
                let seg_start = byte_start + i * segment_size;
                let seg_end = min(seg_start + segment_size - 1, byte_end);
                (seg_start, seg_end)
            })
            .collect();

        self.run_parallel_segments(file, file_exists, ranges, byte_start, range_len, &destination)
            .await?;

        tracing::debug!(
            byte_start,
            byte_end,
            destination = ?destination,
            "✅ Asset range downloaded"
        );

        Ok(())
    }
}
