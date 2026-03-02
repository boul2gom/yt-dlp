//! HTTP fetcher for downloading files with parallel segment support.
//!
//! This module provides the core HTTP fetching functionality with:
//! - Parallel segment downloads
//! - Connection pooling
//! - Retry logic with exponential backoff
//! - Progress tracking

use std::cmp::min;
use std::fmt;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{StreamExt, stream};
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::client::proxy::ProxyConfig;
use crate::download::segment::SegmentContext;
use crate::download::speed_profile::SpeedProfile;
use crate::error::{Error, Result};
use crate::model::format::HttpHeaders;
use crate::utils::fs;
use crate::utils::retry::{RetryPolicy, is_http_error_retryable};

// Download configuration constants
const DEFAULT_PARALLEL_SEGMENTS: usize = 4;
const DEFAULT_SEGMENT_SIZE: usize = 5 * 1024 * 1024; // 5 MB
const DEFAULT_RETRY_ATTEMPTS: usize = 3;
const SEGMENT_CHECK_BUFFER_SIZE: usize = 1024; // 1 KB buffer for checking empty segments

/// The fetcher is responsible for downloading data from a URL.
/// This optimized implementation uses parallel downloads, download resumption,
/// and connection pooling for optimal performance.
pub struct Fetcher {
    /// The URL from which to download the data.
    url: String,
    /// The number of parallel segments to use for downloading.
    /// A higher value can improve performance but consumes more resources.
    parallel_segments: usize,
    /// The size of each segment in bytes.
    segment_size: usize,
    /// The number of download attempts in case of failure.
    retry_attempts: usize,
    /// Retry policy with exponential backoff for HTTP requests.
    retry_policy: RetryPolicy,
    /// Shared HTTP client with connection pooling for efficient request handling.
    client: Arc<reqwest::Client>,
    /// Per-request headers applied on top of the shared client's defaults.
    extra_headers: Option<reqwest::header::HeaderMap>,
    /// Callback optional for tracking download progress
    #[allow(clippy::type_complexity)]
    progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
    /// Speed profile for optimizing download parameters
    speed_profile: SpeedProfile,
    /// Optional byte-range constraint: only download `[start, end]` from the URL.
    ///
    /// When set, [`fetch_asset`] delegates to [`fetch_asset_range`] and writes the
    /// sub-range from offset 0 in the destination file.
    range_constraint: Option<(u64, u64)>,
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
struct PartsGuard {
    path: PathBuf,
    keep: bool,
}

impl PartsGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, keep: false }
    }

    fn commit(&mut self) {
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

    /// Runs the shared parallel segment download pipeline.
    ///
    /// Handles `.parts` progress tracking, resume detection, segment filtering,
    /// concurrent downloading, progress callback, and cleanup. Both [`fetch_asset`]
    /// and [`fetch_asset_range`] delegate to this method after computing their
    /// respective byte ranges and file handles.
    ///
    /// # Arguments
    ///
    /// * `file` - Pre-allocated destination file.
    /// * `file_exists` - Whether the file existed before this download (enables resume detection).
    /// * `ranges` - Byte ranges to download (URL-absolute, inclusive `[start, end]` pairs).
    /// * `file_offset_base` - Subtracted from each range's start to get the file-local write offset.
    /// * `total_bytes` - Total byte count expected for the download (used for progress callbacks).
    /// * `destination` - Destination path (used to derive the `.parts` tracking file path).
    ///
    /// # Errors
    ///
    /// Returns an error if any segment fails after all retry attempts.
    async fn run_parallel_segments(
        &self,
        file: Arc<std::fs::File>,
        file_exists: bool,
        ranges: Vec<(u64, u64)>,
        file_offset_base: u64,
        total_bytes: u64,
        destination: &Path,
    ) -> Result<()> {
        let optimal_segments = self.calculate_optimal_segments(total_bytes);
        let parallel_segments = min(self.parallel_segments, optimal_segments);

        tracing::debug!(
            parallel_segments,
            segment_size = self.segment_size,
            total_bytes,
            optimal_segments,
            "⚙️ Calculated parallel download segments"
        );

        let temp_file_path = format!("{}.parts", destination.display());
        let mut parts_guard = PartsGuard::new(PathBuf::from(&temp_file_path));
        let downloaded_segments = if file_exists && Path::new(&temp_file_path).exists() {
            Self::load_segment_progress(&temp_file_path, ranges.len()).await
        } else {
            vec![false; ranges.len()]
        };

        let ranges_to_download: Vec<(usize, (u64, u64))> = ranges
            .iter()
            .enumerate()
            .filter(|&(i, _)| !downloaded_segments[i])
            .map(|(i, &range)| (i, range))
            .collect();

        tracing::debug!(
            completed = downloaded_segments.iter().filter(|&&x| x).count(),
            total = ranges.len(),
            "🔄 Resuming download"
        );

        let parallel_count = min(parallel_segments, ranges_to_download.len());

        let downloaded_bytes = Arc::new(AtomicU64::new(
            downloaded_segments
                .iter()
                .enumerate()
                .filter(|&(_, &downloaded)| downloaded)
                .map(|(i, _)| {
                    let (start, end) = ranges[i];
                    end - start + 1
                })
                .sum(),
        ));

        let temp_file_path_clone = temp_file_path.clone();
        let downloaded_segments = Arc::new(Mutex::new(downloaded_segments));

        let results = stream::iter(ranges_to_download)
            .map(|(segment_index, (start, end))| {
                let context = SegmentContext {
                    file: Arc::clone(&file),
                    downloaded_bytes: Arc::clone(&downloaded_bytes),
                    progress_callback: self.progress_callback.as_ref().map(Arc::clone),
                    total_bytes,
                    file_offset_base,
                    is_resuming: file_exists,
                };
                let downloaded_segments = Arc::clone(&downloaded_segments);
                let temp_file_path = temp_file_path_clone.clone();

                async move {
                    self.download_and_track_segment(
                        segment_index,
                        start,
                        end,
                        &context,
                        &downloaded_segments,
                        &temp_file_path,
                    )
                    .await
                }
            })
            .buffer_unordered(parallel_count)
            .collect::<Vec<Result<()>>>()
            .await;

        for result in results {
            result?;
        }

        if let Some(callback) = &self.progress_callback {
            callback(total_bytes, total_bytes);
        }

        parts_guard.commit();
        fs::remove_temp_file(temp_file_path).await;

        Ok(())
    }

    /// Probes the server for range request support and content length.
    ///
    /// Uses `GET Range: bytes=0-0` instead of HEAD for better CDN compatibility.
    /// The Content-Range header reveals the total file size.
    async fn probe_range_support(&self) -> Result<(bool, Option<u64>)> {
        let url = self.url.clone();
        let client = Arc::clone(&self.client);

        let response = self
            .retry_policy
            .execute_with_condition(
                || async {
                    let mut req = client.get(&url).header(RANGE, "bytes=0-0");
                    if let Some(ref headers) = self.extra_headers {
                        for (key, value) in headers.iter() {
                            req = req.header(key, value);
                        }
                    }
                    req.send().await
                },
                is_http_error_retryable,
            )
            .await?;

        // 206 Partial Content confirms range support; extract total from Content-Range
        let supports_ranges = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if !supports_ranges {
            tracing::debug!(url = %self.url, "⚙️ Server does not support range requests");
        }

        // Parse total size from Content-Range: bytes 0-0/<total_size>
        let content_length = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .filter(|s| *s != "*")
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| {
                // Fallback to Content-Length header (for non-range responses)
                response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
            });

        if content_length.is_none() && supports_ranges {
            tracing::debug!(url = %self.url, "⚙️ Content-Length header not found");
        }

        Ok((supports_ranges, content_length))
    }

    /// Opens an existing file for resume or creates a new one, pre-allocated to the target size.
    ///
    /// Returns a `std::fs::File` so that parallel segments can perform lock-free positional
    /// writes via `write_all_at` (Unix) / `seek_write` (Windows) without holding a Mutex.
    async fn open_download_file(
        &self,
        destination: &Path,
        file_size: Option<u64>,
        content_length: u64,
    ) -> Result<std::fs::File> {
        if let Some(existing_size) = file_size {
            tracing::debug!(
                destination = ?destination,
                existing_size = existing_size,
                total_size = content_length,
                "🔄 Resuming download of existing file"
            );

            let file = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(destination)
                .await?;

            file.set_len(content_length).await?;
            Ok(file.into_std().await)
        } else {
            tracing::debug!(
                destination = ?destination,
                total_size = content_length,
                "📥 Creating new file for download"
            );

            fs::create_parent_dir(destination).await?;
            let file = fs::create_file(destination).await?;
            file.set_len(content_length).await?;
            Ok(file.into_std().await)
        }
    }

    /// Loads segment progress from a .parts tracking file.
    async fn load_segment_progress(temp_file_path: &str, ranges_count: usize) -> Vec<bool> {
        let Ok(content) = tokio::fs::read_to_string(temp_file_path).await else {
            return vec![false; ranges_count];
        };

        let mut downloaded = vec![false; ranges_count];
        for line in content.lines() {
            if let Ok(index) = line.parse::<usize>()
                && index < downloaded.len()
            {
                downloaded[index] = true;
            }
        }
        downloaded
    }

    /// Downloads a single segment and tracks progress in the .parts file.
    /// Retry logic is handled internally by `download_segment` via `retry_policy`.
    async fn download_and_track_segment(
        &self,
        segment_index: usize,
        start: u64,
        end: u64,
        context: &SegmentContext,
        downloaded_segments: &Mutex<Vec<bool>>,
        temp_file_path: &str,
    ) -> Result<()> {
        self.download_segment(&self.url, start, end, context).await?;

        let mut segments = downloaded_segments.lock().await;
        segments[segment_index] = true;

        if let Ok(mut file) = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(temp_file_path)
            .await
        {
            let _ = file.write_all(format!("{}\n", segment_index).as_bytes()).await;
        }

        Ok(())
    }

    /// Calculate the optimal number of parallel segments based on file size and speed profile
    fn calculate_optimal_segments(&self, file_size: u64) -> usize {
        self.speed_profile
            .calculate_optimal_segments(file_size, self.segment_size as u64)
    }

    /// Checks whether a segment range has already been downloaded by probing start and end bytes.
    ///
    /// Uses positional reads (`read_at` / `seek_read`) via `spawn_blocking` so no lock is held.
    /// Returns `None` when the start has data but the end does not (partial segment — re-download).
    async fn is_segment_downloaded(
        file: Arc<std::fs::File>,
        start: u64,
        end: u64,
        file_offset_base: u64,
    ) -> Result<Option<bool>> {
        let local_start = start - file_offset_base;
        let buf_len = SEGMENT_CHECK_BUFFER_SIZE.min((end - start + 1) as usize);

        let file_a = Arc::clone(&file);
        let start_has_data = tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
            let mut buf = [0u8; SEGMENT_CHECK_BUFFER_SIZE];
            #[cfg(unix)]
            let n = file_a.read_at(&mut buf[..buf_len], local_start)?;
            #[cfg(windows)]
            let n = file_a.seek_read(&mut buf[..buf_len], local_start)?;
            Ok(n > 0 && buf[..n].iter().any(|&b| b != 0))
        })
        .await??;

        if !start_has_data {
            return Ok(Some(false));
        }

        let end_has_data = if (end - start + 1) > SEGMENT_CHECK_BUFFER_SIZE as u64 {
            let seek_pos = (end - file_offset_base).saturating_sub(SEGMENT_CHECK_BUFFER_SIZE as u64 - 1);
            tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
                let mut buf = [0u8; SEGMENT_CHECK_BUFFER_SIZE];
                #[cfg(unix)]
                let n = file.read_at(&mut buf, seek_pos)?;
                #[cfg(windows)]
                let n = file.seek_read(&mut buf, seek_pos)?;
                Ok(n > 0 && buf[..n].iter().any(|&b| b != 0))
            })
            .await??
        } else {
            true
        };

        // None = partial data (start ok, end missing), needs re-download
        Ok(if end_has_data { Some(true) } else { None })
    }

    /// Downloads a specific segment of the file.
    ///
    /// File write position is derived from `start - context.file_offset_base`. For regular
    /// downloads `file_offset_base` is `0`; for range downloads it is the first byte of the
    /// range so the segment is always written from byte 0 in the destination file.
    ///
    /// Writes are performed via positional I/O (`write_all_at` / `seek_write`) inside
    /// `spawn_blocking`, so multiple segments can write concurrently without holding a lock.
    async fn download_segment(&self, url: &str, start: u64, end: u64, context: &SegmentContext) -> Result<()> {
        let client = Arc::clone(&self.client);

        // Only check for existing data when resuming a partial download
        if context.is_resuming {
            match Self::is_segment_downloaded(Arc::clone(&context.file), start, end, context.file_offset_base).await? {
                Some(true) => {
                    tracing::debug!(
                        segment_start = start,
                        segment_end = end,
                        "✅ Segment already downloaded (verified), skipping"
                    );
                    return Ok(());
                }
                None => {
                    tracing::warn!(
                        segment_start = start,
                        segment_end = end,
                        "🔄 Segment has data at start but not at end, re-downloading"
                    );
                }
                Some(false) => {}
            }
        }

        let range_header = format!("bytes={}-{}", start, end);

        let url_clone = url.to_string();
        let range_clone = range_header.clone();

        self.retry_policy
            .execute_with_condition(
                || async {
                    // Snapshot progress before this attempt to rollback on failure
                    let attempt_start = context.downloaded_bytes.load(Ordering::Relaxed);

                    let result: std::result::Result<(), Error> = async {
                    let mut req = client.get(&url_clone).header(RANGE, &range_clone);
                    if let Some(ref headers) = self.extra_headers {
                        for (key, value) in headers.iter() {
                            req = req.header(key, value);
                        }
                    }
                    let response = req.send().await?.error_for_status()?;

                    // file_offset_base translates the URL-absolute offset to a file-local offset
                    let mut current_offset = start - context.file_offset_base;
                    let mut chunk_stream = response.bytes_stream();

                    // Batch chunks before writing to reduce spawn_blocking calls
                    const WRITE_BATCH_SIZE: usize = 256 * 1024; // 256 KB
                    let mut write_buf: Vec<u8> = Vec::with_capacity(WRITE_BATCH_SIZE);
                    let mut buf_offset = current_offset;

                    while let Some(chunk_result) = chunk_stream.next().await {
                        let chunk = chunk_result?;
                        let chunk_len = chunk.len() as u64;
                        write_buf.extend_from_slice(&chunk);
                        current_offset += chunk_len;

                        if write_buf.len() >= WRITE_BATCH_SIZE {
                            let batch = std::mem::replace(&mut write_buf, Vec::with_capacity(WRITE_BATCH_SIZE));
                            let offset = buf_offset;
                            let file = Arc::clone(&context.file);

                            tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                                #[cfg(unix)]
                                file.write_all_at(&batch, offset)?;
                                #[cfg(windows)]
                                {
                                    let mut written = 0usize;
                                    while written < batch.len() {
                                        let n = file.seek_write(&batch[written..], offset + written as u64)?;
                                        if n == 0 {
                                            return Err(std::io::Error::new(
                                                std::io::ErrorKind::WriteZero,
                                                "seek_write returned 0",
                                            ));
                                        }
                                        written += n;
                                    }
                                }
                                Ok(())
                            })
                            .await??;

                            buf_offset = current_offset;
                        }

                        let new_total = context.downloaded_bytes.fetch_add(chunk_len, Ordering::Relaxed) + chunk_len;

                        if let Some(callback) = &context.progress_callback {
                            callback(new_total, context.total_bytes);
                        }
                    }

                    // Flush remaining buffered data
                    if !write_buf.is_empty() {
                        let batch = write_buf;
                        let offset = buf_offset;
                        let file = Arc::clone(&context.file);

                        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                            #[cfg(unix)]
                            file.write_all_at(&batch, offset)?;
                            #[cfg(windows)]
                            {
                                let mut written = 0usize;
                                while written < batch.len() {
                                    let n = file.seek_write(&batch[written..], offset + written as u64)?;
                                    if n == 0 {
                                        return Err(std::io::Error::new(
                                            std::io::ErrorKind::WriteZero,
                                            "seek_write returned 0",
                                        ));
                                    }
                                    written += n;
                                }
                            }
                            Ok(())
                        })
                        .await??;
                    }

                    Ok(())
                    }.await;

                    // Rollback progress on failure to prevent double-counting on retry
                    if result.is_err() {
                        let current = context.downloaded_bytes.load(Ordering::Relaxed);
                        let added = current.saturating_sub(attempt_start);
                        if added > 0 {
                            context.downloaded_bytes.fetch_sub(added, Ordering::Relaxed);
                        }
                    }

                    result
                },
                |err: &Error| {
                    if let Error::Http { source, .. } = err {
                        is_http_error_retryable(source)
                    } else {
                        false
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Simple download method without parallel optimizations.
    async fn fetch_asset_simple(&self, destination: impl Into<PathBuf>) -> Result<()> {
        let destination: PathBuf = destination.into();

        tracing::debug!(
            url = %self.url,
            destination = ?destination,
            "📥 Using simple download (no parallel segments)"
        );

        // Ensure the destination directory exists
        fs::create_parent_dir(&destination).await?;

        // If the parent directory doesn't exist, create it
        if let Some(parent) = destination.parent()
            && !parent.exists()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        let file_size = tokio::fs::metadata(&destination).await.ok().map(|m| m.len());
        let response = self.execute_simple_request(file_size).await?;
        let (append_mode, response) = Self::validate_simple_response(response, file_size, &self.url)?;

        let content_length = response.content_length();
        let mut dest = self.open_simple_destination(&destination, append_mode).await?;
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::with_capacity(1024 * 1024);
        let mut downloaded_bytes = if append_mode { file_size.unwrap_or(0) } else { 0 };
        let total_bytes = match content_length {
            Some(length) if append_mode => length + file_size.unwrap_or(0),
            Some(length) => length,
            None => 0,
        };

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.extend_from_slice(&chunk);

            // Update progress
            downloaded_bytes += chunk.len() as u64;

            // Call progress callback if available
            if let Some(callback) = &self.progress_callback {
                callback(downloaded_bytes, total_bytes);
            }

            // Write the buffer when it reaches a certain size
            if buffer.len() >= 1024 * 1024 {
                dest.write_all(&buffer).await?;
                buffer.clear();
            }
        }

        // Write remaining data
        if !buffer.is_empty() {
            dest.write_all(&buffer).await?;
        }

        Ok(())
    }

    /// Executes the simple download HTTP request with optional resume via Range header.
    async fn execute_simple_request(&self, file_size: Option<u64>) -> Result<reqwest::Response> {
        let url = self.url.clone();
        let range_header = file_size.filter(|&s| s > 0).map(|s| format!("bytes={}-", s));
        let client = Arc::clone(&self.client);

        self.retry_policy
            .execute_with_condition(
                || async {
                    let mut req = client.get(&url);
                    if let Some(ref range) = range_header {
                        req = req.header(RANGE, range);
                    }
                    if let Some(ref headers) = self.extra_headers {
                        for (key, value) in headers.iter() {
                            req = req.header(key, value);
                        }
                    }
                    req.send().await
                },
                is_http_error_retryable,
            )
            .await
            .map_err(Into::into)
    }

    /// Validates the response status and determines whether to append or overwrite.
    fn validate_simple_response(
        response: reqwest::Response,
        file_size: Option<u64>,
        url: &str,
    ) -> Result<(bool, reqwest::Response)> {
        let status = response.status();
        let is_partial = status == reqwest::StatusCode::PARTIAL_CONTENT;

        if !is_partial && status != reqwest::StatusCode::OK {
            return Err(Error::UnexpectedStatus {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }

        let response = response.error_for_status()?;
        let append_mode = is_partial && file_size.is_some_and(|sz| sz > 0);
        Ok((append_mode, response))
    }

    /// Opens the destination file in append or create mode.
    async fn open_simple_destination(&self, destination: &Path, append_mode: bool) -> Result<tokio::fs::File> {
        if append_mode {
            Ok(tokio::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(destination)
                .await?)
        } else {
            Ok(fs::create_file(destination).await?)
        }
    }
}
