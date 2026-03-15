//! Parallel segment download machinery for the HTTP fetcher.
//!
//! Contains the parallel download pipeline, segment management, progress tracking,
//! and simple download fallback. These are `impl Fetcher` methods split from
//! `fetcher.rs` for file-size constraints.

use std::cmp::min;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{StreamExt, stream};
use reqwest::header::RANGE;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::fetcher::{Fetcher, PartsGuard};
use crate::download::engine::segment::SegmentContext;
use crate::error::{Error, Result};
use crate::utils::fs;
use crate::utils::retry::is_http_error_retryable;

// Buffer size for checking whether a segment has already been downloaded
const SEGMENT_CHECK_BUFFER_SIZE: usize = 1024;
// Batch write threshold: flush to disk when buffered data reaches this size
const WRITE_BATCH_SIZE: usize = 256 * 1024;

/// Writes `batch` to `file` at the given `offset` using positional I/O inside `spawn_blocking`.
///
/// Uses `write_all_at` on Unix and a `seek_write` loop on Windows so that multiple segments
/// can write concurrently without holding a lock on the file handle.
///
/// # Arguments
///
/// * `file` - Shared file handle opened for writing.
/// * `batch` - Byte buffer to write.
/// * `offset` - File-local byte offset at which to begin writing.
///
/// # Errors
///
/// Returns an I/O error if the write fails or `seek_write` returns zero.
async fn write_batch_to_file(file: Arc<std::fs::File>, batch: Vec<u8>, offset: u64) -> Result<()> {
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
    .await?
    .map_err(Into::into)
}

/// Reads up to `len` bytes from `file` at `offset` and returns whether any non-zero bytes exist.
///
/// Uses positional I/O (`read_at` / `seek_read`) inside `spawn_blocking` so the call is
/// non-blocking and no file lock is needed.
async fn read_bytes_at(file: Arc<std::fs::File>, offset: u64, len: usize) -> Result<bool> {
    Ok(tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
        let mut buf = [0u8; SEGMENT_CHECK_BUFFER_SIZE];
        #[cfg(unix)]
        let n = file.read_at(&mut buf[..len], offset)?;
        #[cfg(windows)]
        let n = file.seek_read(&mut buf[..len], offset)?;
        Ok(n > 0 && buf[..n].iter().any(|&b| b != 0))
    })
    .await??)
}

/// Sends `request`, streams the response body into `context.file`, and tracks byte counts.
///
/// Writes are batched in [`WRITE_BATCH_SIZE`] chunks to minimise `spawn_blocking` calls.
/// `attempt_bytes` is incremented for every byte received; the caller should subtract it
/// from `context.downloaded_bytes` on error to avoid double-counting on retry.
async fn stream_response_chunks(
    request: reqwest::RequestBuilder,
    context: &SegmentContext,
    start: u64,
    attempt_bytes: &mut u64,
) -> Result<()> {
    let response = request.send().await?.error_for_status()?;

    let mut current_offset = start - context.file_offset_base;
    let mut chunk_stream = response.bytes_stream();
    let mut write_buf: Vec<u8> = Vec::with_capacity(WRITE_BATCH_SIZE);
    let mut buf_offset = current_offset;

    while let Some(chunk_result) = chunk_stream.next().await {
        let chunk = chunk_result?;
        let chunk_len = chunk.len() as u64;
        write_buf.extend_from_slice(&chunk);
        current_offset += chunk_len;

        if write_buf.len() >= WRITE_BATCH_SIZE {
            let batch = std::mem::replace(&mut write_buf, Vec::with_capacity(WRITE_BATCH_SIZE));
            write_batch_to_file(Arc::clone(&context.file), batch, buf_offset).await?;
            buf_offset = current_offset;
        }

        *attempt_bytes += chunk_len;
        let new_total = context.downloaded_bytes.fetch_add(chunk_len, Ordering::Relaxed) + chunk_len;
        if let Some(callback) = &context.progress_callback {
            callback(new_total, context.total_bytes);
        }
    }

    if !write_buf.is_empty() {
        write_batch_to_file(Arc::clone(&context.file), write_buf, buf_offset).await?;
    }

    Ok(())
}

impl Fetcher {
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
    pub(super) async fn run_parallel_segments(
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
    pub(super) async fn probe_range_support(&self) -> Result<(bool, Option<u64>)> {
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
    pub(super) async fn open_download_file(
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

        if !read_bytes_at(Arc::clone(&file), local_start, buf_len).await? {
            return Ok(Some(false));
        }

        let end_has_data = if (end - start + 1) > SEGMENT_CHECK_BUFFER_SIZE as u64 {
            let seek_pos = (end - file_offset_base).saturating_sub(SEGMENT_CHECK_BUFFER_SIZE as u64 - 1);
            read_bytes_at(file, seek_pos, SEGMENT_CHECK_BUFFER_SIZE).await?
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
                    // Track bytes downloaded in this attempt locally to avoid
                    // corrupting the global counter when other segments run concurrently
                    let mut attempt_bytes: u64 = 0;

                    let mut req = client.get(&url_clone).header(RANGE, &range_clone);
                    if let Some(ref headers) = self.extra_headers {
                        for (key, value) in headers.iter() {
                            req = req.header(key, value);
                        }
                    }

                    let result = stream_response_chunks(req, context, start, &mut attempt_bytes).await;

                    // Rollback only this attempt's bytes on failure to prevent double-counting on retry
                    if result.is_err() && attempt_bytes > 0 {
                        context.downloaded_bytes.fetch_sub(attempt_bytes, Ordering::Relaxed);
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
    pub(super) async fn fetch_asset_simple(&self, destination: impl Into<PathBuf>) -> Result<()> {
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

        dest.flush().await?;

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
