//! Tools for fetching data from a URL.
//!
//! This module is subdivided into several modules, each responsible for fetching a specific type of data.
//! This module contains structs for fetching video data, dependencies binaries, or HTTP data.
//!
//! The `blocking` module contains blocking functions for fetching data from YouTube.

use crate::error::{Error, Result};
use crate::utils::file_system;
use futures_util::{StreamExt, stream};
use reqwest::header::{HeaderMap, HeaderValue, RANGE, USER_AGENT};
use std::cmp::min;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

pub mod deps;
pub mod download_manager;
pub mod streams;
pub mod thumbnail;

/// Context for segment download operations
struct SegmentContext {
    file: Arc<Mutex<tokio::fs::File>>,
    downloaded_bytes: Arc<std::sync::atomic::AtomicU64>,
    progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
    total_bytes: u64,
}

/// The fetcher is responsible for downloading data from a URL.
/// This optimized implementation uses parallel downloads and download resumption.
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
    /// Callback optional for tracking download progress
    #[allow(clippy::type_complexity)]
    progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
}

impl fmt::Display for Fetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Fetcher(url={}, segments={})",
            self.url, self.parallel_segments
        )
    }
}

impl Fetcher {
    /// Creates a new fetcher for the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL from which to download the data.
    pub fn new(url: impl AsRef<str>) -> Self {
        Self {
            url: url.as_ref().to_string(),
            parallel_segments: 4,          // 4 parallel segments by default
            segment_size: 1024 * 1024 * 5, // 5 MB per segment by default
            retry_attempts: 3,
            progress_callback: None,
        }
    }

    /// Configures the number of parallel segments for downloading.
    ///
    /// # Arguments
    ///
    /// * `segments` - The number of parallel segments to use.
    pub fn with_parallel_segments(mut self, segments: usize) -> Self {
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
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching JSON from {}", self.url);

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("rust-reqwest"));

        if let Some(auth_token) = auth_token {
            let value = HeaderValue::from_str(&format!("Bearer {}", auth_token))
                .map_err(|e| Error::Unknown(e.to_string()))?;

            headers.insert(reqwest::header::AUTHORIZATION, value);
        }

        let client = reqwest::Client::new();
        let response = client
            .get(&self.url)
            .headers(headers)
            .send()
            .await?
            .error_for_status()?;

        let json = response.json().await?;
        Ok(json)
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
    pub async fn fetch_asset(&self, destination: impl AsRef<Path> + std::fmt::Debug) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Fetching asset from {} to {:?}", self.url, destination);

        // Ensure the destination directory exists
        file_system::create_parent_dir(&destination)?;

        // If the parent directory doesn't exist, create it
        if let Some(parent) = destination.as_ref().parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Check if the file exists and if we can resume the download
        let file_exists = destination.as_ref().exists();
        let file_size = if file_exists {
            match tokio::fs::metadata(destination.as_ref()).await {
                Ok(metadata) => Some(metadata.len()),
                Err(_) => None,
            }
        } else {
            None
        };

        // Check if the server supports range requests
        let client = reqwest::Client::new();
        let head_response = client.head(&self.url).send().await?;

        // If the server does not support range requests, use the simple method
        if !head_response.headers().contains_key("accept-ranges") {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Server does not support range requests, falling back to simple download"
            );
            return self.fetch_asset_simple(destination).await;
        }

        // Get the total file size
        let content_length = match head_response.headers().get("content-length") {
            Some(length) => {
                let length_str = length.to_str().map_err(|e| Error::Unknown(e.to_string()))?;
                length_str
                    .parse::<u64>()
                    .map_err(|e| Error::Unknown(e.to_string()))?
            }
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Content-Length header not found, falling back to simple download");
                return self.fetch_asset_simple(destination).await;
            }
        };

        // If the file exists and has the same size, it is already downloaded
        if let Some(size) = file_size {
            if size == content_length {
                #[cfg(feature = "tracing")]
                tracing::debug!("File already exists with correct size, skipping download");
                return Ok(());
            }
        }

        // Create or open the destination file
        let file = if file_exists && file_size.is_some() {
            // Open existing file for resuming download
            #[cfg(feature = "tracing")]
            tracing::debug!("Resuming download of existing file");

            let file = tokio::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(destination.as_ref())
                .await?;

            // Ensure the file is the correct size
            file.set_len(content_length).await?;
            file
        } else {
            // Create a new file
            #[cfg(feature = "tracing")]
            tracing::debug!("Creating new file for download");

            file_system::create_parent_dir(&destination)?;
            let file = file_system::create_file(&destination).await?;
            // Resize the file to the total size
            file.set_len(content_length).await?;
            file
        };

        // Create a mutex to share the file between tasks
        let file = Arc::new(Mutex::new(file));

        // Calculate the optimal number of parallel segments based on file size
        let optimal_segments = self.calculate_optimal_segments(content_length);
        let parallel_segments = min(self.parallel_segments, optimal_segments);

        #[cfg(feature = "tracing")]
        tracing::debug!("Using {} parallel segments for download", parallel_segments);

        // Calculate ranges for each segment
        let segment_size = self.segment_size as u64;
        let mut ranges = Vec::new();

        for i in 0..content_length.div_ceil(segment_size) {
            let start = i * segment_size;
            let end = min(start + segment_size - 1, content_length - 1);
            ranges.push((start, end));
        }

        // Create a temporary file to track downloaded segments
        let temp_file_path = format!("{}.parts", destination.as_ref().display());
        let downloaded_segments = if file_exists && std::path::Path::new(&temp_file_path).exists() {
            // Read the downloaded segments from the temporary file
            match tokio::fs::read_to_string(&temp_file_path).await {
                Ok(content) => {
                    let mut downloaded = vec![false; ranges.len()];
                    for line in content.lines() {
                        if let Ok(index) = line.parse::<usize>() {
                            if index < downloaded.len() {
                                downloaded[index] = true;
                            }
                        }
                    }
                    downloaded
                }
                Err(_) => vec![false; ranges.len()],
            }
        } else {
            vec![false; ranges.len()]
        };

        // Filter out already downloaded segments
        let ranges_to_download: Vec<(usize, (u64, u64))> = ranges
            .iter()
            .enumerate()
            .filter(|&(i, _)| !downloaded_segments[i])
            .map(|(i, &range)| (i, range))
            .collect();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Resuming download: {} of {} segments already downloaded",
            downloaded_segments.iter().filter(|&&x| x).count(),
            ranges.len()
        );

        // Limit the number of parallel tasks
        let parallel_count = min(parallel_segments, ranges_to_download.len());

        // Create an atomic counter to track progress
        let downloaded_bytes = Arc::new(AtomicU64::new(
            // Start with the sum of already downloaded segments
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
        let total_bytes = content_length;

        // Create a temporary file to track downloaded segments
        let temp_file_path_clone = temp_file_path.clone();
        let downloaded_segments = Arc::new(Mutex::new(downloaded_segments));

        // Create a stream of futures to download each segment
        let results = stream::iter(ranges_to_download)
            .map(|(segment_index, (start, end))| {
                let url = self.url.clone();
                let file_clone = Arc::clone(&file);
                let downloaded_bytes_clone = Arc::clone(&downloaded_bytes);
                let progress_callback = self.progress_callback.as_ref().map(Arc::clone);
                let downloaded_segments_clone = Arc::clone(&downloaded_segments);
                let temp_file_path = temp_file_path_clone.clone();

                async move {
                    for attempt in 0..self.retry_attempts {
                        match self
                            .download_segment(
                                &url,
                                start,
                                end,
                                &SegmentContext {
                                    file: Arc::clone(&file_clone),
                                    downloaded_bytes: Arc::clone(&downloaded_bytes_clone),
                                    progress_callback: progress_callback.clone(),
                                    total_bytes,
                                },
                            )
                            .await
                        {
                            Ok(_) => {
                                // Mark the segment as downloaded
                                let mut segments = downloaded_segments_clone.lock().await;
                                segments[segment_index] = true;

                                // Update the temporary file
                                if let Ok(mut file) = tokio::fs::OpenOptions::new()
                                    .create(true)
                                    .write(true)
                                    .append(true)
                                    .open(&temp_file_path)
                                    .await
                                {
                                    let _ = file
                                        .write_all(format!("{}\n", segment_index).as_bytes())
                                        .await;
                                }

                                return Ok(());
                            }
                            Err(error) if attempt < self.retry_attempts - 1 => {
                                #[cfg(feature = "tracing")]
                                tracing::warn!(
                                    "Segment download failed (attempt {}): {}",
                                    attempt + 1,
                                    error
                                );
                                // Consume the error
                                let _ = error;

                                // Wait a bit before retrying (exponential backoff)
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    250 * 2u64.pow(attempt as u32),
                                ))
                                .await;
                            }
                            Err(error) => return Err(error),
                        }
                    }

                    Err(Error::Unknown(format!(
                        "Failed to download segment after {} attempts",
                        self.retry_attempts
                    )))
                }
            })
            .buffer_unordered(parallel_count)
            .collect::<Vec<Result<()>>>();

        // Wait for all downloads to complete
        let results = results.await;

        // Check if there were any errors
        for result in results {
            result?;
        }

        // Call the callback one last time to indicate that the download is complete
        if let Some(callback) = &self.progress_callback {
            callback(total_bytes, total_bytes);
        }

        // Remove the temporary file
        let _ = tokio::fs::remove_file(temp_file_path).await;

        Ok(())
    }

    /// Calculate the optimal number of parallel segments based on file size
    fn calculate_optimal_segments(&self, file_size: u64) -> usize {
        // Dynamic adjustment of the number of segments based on file size
        // and segment size
        let segment_size = self.segment_size as u64;

        // Calculate the total number of segments needed
        let total_segments = file_size.div_ceil(segment_size);

        // Limit the number of segments based on file size
        let file_size_mb = file_size / (1024 * 1024);

        // Determine the maximum number of parallel segments based on file size
        let max_parallel_segments = match file_size_mb {
            size if size < 10 => 1,    // Less than 10 MB
            size if size < 50 => 2,    // Less than 50 MB
            size if size < 100 => 4,   // Less than 100 MB
            size if size < 500 => 8,   // Less than 500 MB
            size if size < 1000 => 12, // Less than 1 GB
            size if size < 2000 => 16, // Less than 2 GB
            _ => 24,                   // More than 2 GB
        };

        // Take the minimum between total segments and maximum parallel segments
        std::cmp::min(total_segments as usize, max_parallel_segments)
    }

    /// Downloads a specific segment of the file.
    async fn download_segment(
        &self,
        url: &str,
        start: u64,
        end: u64,
        context: &SegmentContext,
    ) -> Result<()> {
        let client = reqwest::Client::new();

        // Check if the segment is already downloaded by reading the file
        let mut file_guard = context.file.lock().await;
        file_guard.seek(std::io::SeekFrom::Start(start)).await?;

        // Read a small sample to check if the segment is already downloaded
        // This is a heuristic and not 100% reliable, but it's fast
        let mut buffer = vec![0; 1024.min((end - start + 1) as usize)];
        let bytes_read = file_guard.read(&mut buffer).await?;

        // If we read some data and it's not all zeros, assume the segment is already downloaded
        let is_segment_empty = bytes_read == 0 || buffer.iter().all(|&b| b == 0);

        // Release the file lock before making HTTP request
        drop(file_guard);

        if !is_segment_empty {
            // We don't update the downloaded_bytes counter here because it was already
            // initialized with the sum of already downloaded segments
            #[cfg(feature = "tracing")]
            tracing::debug!("Segment {}-{} already downloaded, skipping", start, end);

            return Ok(());
        }

        // Create the Range header
        let range_header = format!("bytes={}-{}", start, end);

        // Make the request with the Range header
        let response = client
            .get(url)
            .header(RANGE, range_header)
            .send()
            .await?
            .error_for_status()?;

        // Read the data
        let data = response.bytes().await?;

        // Acquire the mutex and write the data at the correct position
        let mut file_guard = context.file.lock().await;
        file_guard.seek(std::io::SeekFrom::Start(start)).await?;
        file_guard.write_all(&data).await?;

        // Update the progress counter
        let segment_size = data.len() as u64;
        let new_total = context
            .downloaded_bytes
            .fetch_add(segment_size, Ordering::SeqCst)
            + segment_size;

        // Call the progress callback if available
        if let Some(callback) = &context.progress_callback {
            callback(new_total, context.total_bytes);
        }

        Ok(())
    }

    /// Simple download method without parallel optimizations.
    async fn fetch_asset_simple(&self, destination: impl AsRef<Path>) -> Result<()> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Using simple download for {}", self.url);

        // Ensure the destination directory exists
        file_system::create_parent_dir(&destination)?;

        // If the parent directory doesn't exist, create it
        if let Some(parent) = destination.as_ref().parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Check if the file exists and get its size
        let file_exists = destination.as_ref().exists();
        let file_size = if file_exists {
            match tokio::fs::metadata(destination.as_ref()).await {
                Ok(metadata) => Some(metadata.len()),
                Err(_) => None,
            }
        } else {
            None
        };

        // Create a client with a longer timeout
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()?;

        // If the file exists, try to resume the download
        let mut request = client.get(&self.url);

        // Add Range header if the file exists and has some content
        if let Some(size) = file_size {
            if size > 0 {
                #[cfg(feature = "tracing")]
                tracing::debug!("Resuming download from byte {}", size);

                request = request.header(RANGE, format!("bytes={}-", size));
            }
        }

        // Add User-Agent header
        request = request.header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36");

        // Send the request
        let response = request.send().await?;

        // Check if the server accepted our range request
        let status = response.status();
        let is_partial_content = status == reqwest::StatusCode::PARTIAL_CONTENT;
        let is_ok = status == reqwest::StatusCode::OK;

        // Ensure the response is valid
        if !is_partial_content && !is_ok {
            return Err(Error::Unknown(format!(
                "Unexpected status code: {}",
                status
            )));
        }

        // Ensure the response is successful
        let response = response.error_for_status()?;

        // Get content length before checking if we need to resume
        let content_length = response.content_length();

        // If we got a 200 OK instead of 206 Partial Content, the server doesn't support range requests
        // In this case, we need to start the download from the beginning
        let append_mode = is_partial_content && file_size.is_some() && file_size.unwrap() > 0;

        // Open the file in the appropriate mode
        let mut dest = if append_mode {
            tokio::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&destination)
                .await?
        } else {
            file_system::create_file(&destination).await?
        };

        let mut stream = response.bytes_stream();

        // Use a larger buffer to improve performance
        let mut buffer = Vec::with_capacity(1024 * 1024); // 1 MB buffer

        // Track progress for callback
        let mut downloaded_bytes = if append_mode {
            file_size.unwrap_or(0)
        } else {
            0
        };

        // Get total size if available
        let total_bytes = if let Some(length) = content_length {
            if append_mode {
                length + file_size.unwrap_or(0)
            } else {
                length
            }
        } else {
            0 // Unknown size
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
}
