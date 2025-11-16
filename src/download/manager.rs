//! Download manager with priority queue and concurrent downloads limitation.
//!
//! This module provides a download manager that allows:
//! - Limiting the number of concurrent downloads
//! - Managing a download queue with priorities
//! - Resuming interrupted downloads
//! - Optimizing memory usage

use crate::client::proxy::ProxyConfig;
use crate::download::fetcher::Fetcher;
use crate::download::speed_profile::SpeedProfile;
use crate::error::Result;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore, broadcast};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

// Download manager default configuration constants
const DEFAULT_RETRY_ATTEMPTS: usize = 3;

/// Download priority
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

impl DownloadPriority {
    /// Convertit an integer to priority
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Normal,
            2 => Self::High,
            3 => Self::Critical,
            _ => Self::Normal,
        }
    }
}

/// Download task
struct DownloadTask {
    /// URL to download
    url: String,
    /// Destination path
    destination: PathBuf,
    /// Download priority
    priority: DownloadPriority,
    /// Unique ID of the task
    id: u64,
    /// Progress callback
    #[allow(clippy::type_complexity)]
    progress_callback: Option<Arc<dyn Fn(u64, u64) + Send + Sync>>,
}

impl std::fmt::Debug for DownloadTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadTask")
            .field("url", &self.url)
            .field("destination", &self.destination)
            .field("priority", &self.priority)
            .field("id", &self.id)
            .field(
                "progress_callback",
                &format_args!(
                    "{}",
                    if self.progress_callback.is_some() {
                        "Some(Fn)"
                    } else {
                        "None"
                    }
                ),
            )
            .finish()
    }
}

impl PartialEq for DownloadTask {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for DownloadTask {}

impl PartialOrd for DownloadTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DownloadTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by priority (higher priority = more prioritary)
        let priority_cmp = (other.priority as i32).cmp(&(self.priority as i32));
        if priority_cmp != Ordering::Equal {
            return priority_cmp;
        }

        // Then by ID (smaller ID = older = more prioritary)
        self.id.cmp(&other.id)
    }
}

/// Download manager configuration
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// Maximum number of concurrent downloads
    pub max_concurrent_downloads: usize,
    /// Segment size for parallel download (in bytes)
    pub segment_size: usize,
    /// Number of parallel segments per download
    pub parallel_segments: usize,
    /// Number of download attempts in case of failure
    pub retry_attempts: usize,
    /// Maximum buffer size per download (in bytes)
    pub max_buffer_size: usize,
    /// Optional proxy configuration
    pub proxy: Option<ProxyConfig>,
    /// Speed profile for automatic optimization
    pub speed_profile: SpeedProfile,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self::from_speed_profile(SpeedProfile::default())
    }
}

impl ManagerConfig {
    /// Create a ManagerConfig from a speed profile
    ///
    /// This automatically configures all download parameters based on the profile.
    ///
    /// # Arguments
    ///
    /// * `profile` - The speed profile to use
    pub fn from_speed_profile(profile: SpeedProfile) -> Self {
        Self {
            max_concurrent_downloads: profile.max_concurrent_downloads(),
            segment_size: profile.segment_size(),
            parallel_segments: profile.parallel_segments(),
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            max_buffer_size: profile.max_buffer_size(),
            proxy: None,
            speed_profile: profile,
        }
    }

    /// Set the speed profile and update all related parameters
    ///
    /// # Arguments
    ///
    /// * `profile` - The speed profile to use
    pub fn with_speed_profile(mut self, profile: SpeedProfile) -> Self {
        self.max_concurrent_downloads = profile.max_concurrent_downloads();
        self.segment_size = profile.segment_size();
        self.parallel_segments = profile.parallel_segments();
        self.max_buffer_size = profile.max_buffer_size();
        self.speed_profile = profile;
        self
    }

    /// Set the proxy configuration
    ///
    /// # Arguments
    ///
    /// * `proxy` - The proxy configuration
    pub fn with_proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Set the maximum number of concurrent downloads
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum number of concurrent downloads
    pub fn with_max_concurrent_downloads(mut self, max: usize) -> Self {
        self.max_concurrent_downloads = max;
        self
    }

    /// Set the segment size for parallel downloads
    ///
    /// # Arguments
    ///
    /// * `size` - Segment size in bytes
    pub fn with_segment_size(mut self, size: usize) -> Self {
        self.segment_size = size;
        self
    }

    /// Set the number of parallel segments per download
    ///
    /// # Arguments
    ///
    /// * `segments` - Number of parallel segments
    pub fn with_parallel_segments(mut self, segments: usize) -> Self {
        self.parallel_segments = segments;
        self
    }

    /// Set the number of retry attempts for failed downloads
    ///
    /// # Arguments
    ///
    /// * `attempts` - Number of retry attempts
    pub fn with_retry_attempts(mut self, attempts: usize) -> Self {
        self.retry_attempts = attempts;
        self
    }

    /// Set the maximum buffer size per download
    ///
    /// # Arguments
    ///
    /// * `size` - Maximum buffer size in bytes
    pub fn with_max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }
}

/// Progress update event for streaming API
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressUpdate {
    /// Download ID
    pub download_id: u64,
    /// Downloaded bytes
    pub downloaded_bytes: u64,
    /// Total bytes
    pub total_bytes: u64,
}

/// Download status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Queued
    Queued,
    /// Downloading
    Downloading {
        /// Downloaded bytes
        downloaded_bytes: u64,
        /// Total size in bytes
        total_bytes: u64,
    },
    /// Download completed
    Completed,
    /// Download failed
    Failed {
        /// Reason of failure
        reason: String,
    },
    /// Download canceled
    Canceled,
}

/// Download manager
pub struct DownloadManager {
    /// Download manager configuration
    config: ManagerConfig,
    /// Download queue
    queue: Arc<Mutex<BinaryHeap<DownloadTask>>>,
    /// Semaphore to limit the number of concurrent downloads
    semaphore: Arc<Semaphore>,
    /// Counter to generate unique IDs
    next_id: Arc<Mutex<u64>>,
    /// Download statuses
    statuses: Arc<Mutex<HashMap<u64, DownloadStatus>>>,
    /// Download tasks in progress
    tasks: Arc<Mutex<HashMap<u64, JoinHandle<Result<()>>>>>,
    /// Cancelled task IDs
    cancelled: Arc<Mutex<HashSet<u64>>>,
    /// Broadcast channel for status updates (event-driven completion notifications)
    completion_tx: broadcast::Sender<(u64, DownloadStatus)>,
    /// Broadcast channel for progress updates (stream-based progress API)
    progress_tx: broadcast::Sender<ProgressUpdate>,
}

impl std::fmt::Debug for DownloadManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadManager")
            .field("config", &self.config)
            .field(
                "max_concurrent_downloads",
                &self.config.max_concurrent_downloads,
            )
            .finish_non_exhaustive()
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadManager {
    /// Create a new download manager with default configuration
    pub fn new() -> Self {
        Self::with_config(ManagerConfig::default())
    }

    /// Create a new download manager with custom configuration
    pub fn with_config(config: ManagerConfig) -> Self {
        let (completion_tx, _) = broadcast::channel(100);
        let (progress_tx, _) = broadcast::channel(1000); // Larger buffer for frequent progress updates

        Self {
            config: config.clone(),
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            next_id: Arc::new(Mutex::new(0)),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            completion_tx,
            progress_tx,
        }
    }

    /// Add a download to the queue
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download
    /// * `destination` - The destination path
    /// * `priority` - The download priority (optional, default Normal)
    ///
    /// # Returns
    ///
    /// The ID of the download
    pub async fn enqueue(
        &self,
        url: impl AsRef<str>,
        destination: impl AsRef<Path>,
        priority: Option<DownloadPriority>,
    ) -> u64 {
        let mut id_guard = self.next_id.lock().await;
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        let task = DownloadTask {
            url: url.as_ref().to_string(),
            destination: destination.as_ref().to_path_buf(),
            priority: priority.unwrap_or(DownloadPriority::Normal),
            id,
            progress_callback: None,
        };

        // Add the task to the queue
        {
            let mut queue = self.queue.lock().await;
            queue.push(task);
        }

        // Update status
        {
            let mut statuses = self.statuses.lock().await;
            statuses.insert(id, DownloadStatus::Queued);
        }

        // Start the queue processor
        self.process_queue();

        id
    }

    /// Add a download to the queue with a progress callback
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download
    /// * `destination` - The destination path
    /// * `priority` - The download priority (optional, default Normal)
    /// * `progress_callback` - Function called with downloaded bytes and total size
    ///
    /// # Returns
    ///
    /// The ID of the download
    pub async fn enqueue_with_progress<F>(
        &self,
        url: impl AsRef<str>,
        destination: impl AsRef<Path>,
        priority: Option<DownloadPriority>,
        progress_callback: F,
    ) -> u64
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let mut id_guard = self.next_id.lock().await;
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        let task = DownloadTask {
            url: url.as_ref().to_string(),
            destination: destination.as_ref().to_path_buf(),
            priority: priority.unwrap_or(DownloadPriority::Normal),
            id,
            progress_callback: Some(Arc::new(progress_callback)),
        };

        // Add the task to the queue
        {
            let mut queue = self.queue.lock().await;
            queue.push(task);
        }

        // Update status
        {
            let mut statuses = self.statuses.lock().await;
            statuses.insert(id, DownloadStatus::Queued);
        }

        // Start the queue processor
        self.process_queue();

        id
    }

    /// Get the status of a download
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the download
    ///
    /// # Returns
    ///
    /// The download status, or None if the ID doesn't exist
    pub async fn get_status(&self, id: u64) -> Option<DownloadStatus> {
        let statuses = self.statuses.lock().await;
        statuses.get(&id).cloned()
    }

    /// Clean up completed, failed, and cancelled downloads from internal maps
    ///
    /// This method removes finished downloads from memory to prevent memory leaks.
    /// It should be called periodically or after downloads complete.
    pub async fn cleanup_finished(&self) {
        let mut statuses = self.statuses.lock().await;
        let mut cancelled = self.cancelled.lock().await;

        // Collect IDs to remove
        let ids_to_remove: Vec<u64> = statuses
            .iter()
            .filter_map(|(id, status)| match status {
                DownloadStatus::Completed
                | DownloadStatus::Failed { .. }
                | DownloadStatus::Canceled => Some(*id),
                _ => None,
            })
            .collect();

        // Remove from statuses and cancelled
        for id in ids_to_remove {
            statuses.remove(&id);
            cancelled.remove(&id);
        }
    }

    /// Cancel a download
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the download to cancel
    ///
    /// # Returns
    ///
    /// true if the download was canceled, false if it doesn't exist or is already completed
    pub async fn cancel(&self, id: u64) -> bool {
        // Mark as cancelled first to prevent race conditions
        {
            let mut cancelled = self.cancelled.lock().await;
            cancelled.insert(id);
        }

        // Check if the download is in progress
        let task_handle = {
            let mut tasks = self.tasks.lock().await;
            tasks.remove(&id)
        };

        // If the download is in progress, cancel it
        if let Some(handle) = task_handle {
            handle.abort();

            // Update status
            let mut statuses = self.statuses.lock().await;
            statuses.insert(id, DownloadStatus::Canceled);

            return true;
        }

        // Check if the download is in the queue
        let removed_from_queue = {
            let mut queue = self.queue.lock().await;
            let len_before = queue.len();

            // Create a new queue without the task to cancel
            let mut new_queue = BinaryHeap::new();
            for task in queue.drain() {
                if task.id != id {
                    new_queue.push(task);
                }
            }

            // Replace the queue
            *queue = new_queue;

            len_before > queue.len()
        };

        if removed_from_queue {
            // Update status
            let mut statuses = self.statuses.lock().await;
            statuses.insert(id, DownloadStatus::Canceled);
            return true;
        }

        // Even if not found in queue or tasks, it might be in the brief window
        // between being popped and starting execution, so mark as cancelled
        let mut statuses = self.statuses.lock().await;
        statuses.insert(id, DownloadStatus::Canceled);
        true
    }

    /// Wait for a download to complete using event-driven notifications (no polling).
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the download to wait for
    ///
    /// # Returns
    ///
    /// The final download status, or None if the ID doesn't exist
    pub async fn wait_for_completion(&self, id: u64) -> Option<DownloadStatus> {
        // First check if the download already completed
        if let Some(status) = self.get_status(id).await {
            match status {
                DownloadStatus::Completed
                | DownloadStatus::Failed { .. }
                | DownloadStatus::Canceled => {
                    return Some(status);
                }
                _ => {}
            }
        }

        // Subscribe to completion events
        let mut rx = self.completion_tx.subscribe();

        // Wait for the completion event for this specific download
        loop {
            match rx.recv().await {
                Ok((download_id, status)) if download_id == id => match status {
                    DownloadStatus::Completed
                    | DownloadStatus::Failed { .. }
                    | DownloadStatus::Canceled => {
                        return Some(status);
                    }
                    _ => continue,
                },
                Ok(_) => continue, // Event for a different download
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Channel lagged, check current status
                    if let Some(status) = self.get_status(id).await {
                        match status {
                            DownloadStatus::Completed
                            | DownloadStatus::Failed { .. }
                            | DownloadStatus::Canceled => {
                                return Some(status);
                            }
                            _ => continue,
                        }
                    } else {
                        return None;
                    }
                }
                Err(_) => return None, // Channel closed
            }
        }
    }

    /// Subscribe to progress updates for a specific download as a stream.
    ///
    /// This provides a stream-based API for tracking download progress in real-time.
    /// The stream will emit `ProgressUpdate` events as the download progresses.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the download to track
    ///
    /// # Returns
    ///
    /// A stream of `ProgressUpdate` events filtered for the specified download ID
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use tokio_stream::StreamExt;
    ///
    /// let download_id = manager.enqueue("https://example.com/file", "output", None).await;
    /// let mut progress_stream = manager.progress_stream(download_id);
    ///
    /// while let Some(update) = progress_stream.next().await {
    ///     println!("Downloaded: {}/{} bytes ({:.1}%)",
    ///         update.downloaded_bytes,
    ///         update.total_bytes,
    ///         (update.downloaded_bytes as f64 / update.total_bytes as f64) * 100.0
    ///     );
    /// }
    /// ```
    pub fn progress_stream(&self, id: u64) -> impl Stream<Item = ProgressUpdate> + Send + 'static {
        let rx = self.progress_tx.subscribe();

        // Create a stream that filters events for the specific download ID
        BroadcastStream::new(rx).filter_map(move |result| match result {
            Ok(update) if update.download_id == id => Some(update),
            _ => None,
        })
    }

    /// Subscribe to all progress updates as a stream.
    ///
    /// This provides a stream-based API for tracking all download progress in real-time.
    ///
    /// # Returns
    ///
    /// A stream of `ProgressUpdate` events for all downloads
    pub fn progress_stream_all(&self) -> impl Stream<Item = ProgressUpdate> + Send + 'static {
        let rx = self.progress_tx.subscribe();

        BroadcastStream::new(rx).filter_map(|result| result.ok())
    }

    /// Process the download queue
    fn process_queue(&self) {
        let queue_clone = self.queue.clone();
        let semaphore_clone = self.semaphore.clone();
        let statuses_clone = self.statuses.clone();
        let tasks_clone = self.tasks.clone();
        let config_clone = self.config.clone();
        let cancelled_clone = self.cancelled.clone();
        let completion_tx_clone = self.completion_tx.clone();
        let progress_tx_clone = self.progress_tx.clone();

        tokio::spawn(async move {
            loop {
                // Acquire a permit from the semaphore (blocks if the maximum number of downloads is reached)
                let permit = match semaphore_clone.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => break, // The semaphore has been closed, stop processing
                };

                // Get the next task from the queue
                let task = {
                    let mut queue = queue_clone.lock().await;
                    queue.pop()
                };

                // If the queue is empty, release the permit and stop
                let task = match task {
                    Some(task) => task,
                    None => {
                        drop(permit); // Release the permit
                        break;
                    }
                };

                // Check if the task was cancelled before starting
                {
                    let cancelled = cancelled_clone.lock().await;
                    if cancelled.contains(&task.id) {
                        drop(permit); // Release the permit
                        continue; // Skip this task
                    }
                }

                // Update status
                {
                    let mut statuses = statuses_clone.lock().await;
                    statuses.insert(
                        task.id,
                        DownloadStatus::Downloading {
                            downloaded_bytes: 0,
                            total_bytes: 0,
                        },
                    );
                }

                // Create a fetcher for this task
                let mut fetcher = Fetcher::new(&task.url, config_clone.proxy.as_ref())
                    .with_segment_size(config_clone.segment_size)
                    .with_parallel_segments(config_clone.parallel_segments)
                    .with_retry_attempts(config_clone.retry_attempts)
                    .with_speed_profile(config_clone.speed_profile);

                // Add progress callback if available
                let task_id = task.id;
                let statuses_for_callback = statuses_clone.clone();
                let progress_tx_for_callback = progress_tx_clone.clone();

                if let Some(callback) = task.progress_callback {
                    fetcher = fetcher.with_progress_callback(move |downloaded, total| {
                        let statuses_for_callback = statuses_for_callback.clone();
                        let progress_tx = progress_tx_for_callback.clone();

                        tokio::task::spawn_blocking(move || {
                            // Update status with progress
                            let mut statuses = statuses_for_callback.blocking_lock();
                            statuses.insert(
                                task_id,
                                DownloadStatus::Downloading {
                                    downloaded_bytes: downloaded,
                                    total_bytes: total,
                                },
                            );
                        });

                        // Emit progress event for stream-based API
                        let _ = progress_tx.send(ProgressUpdate {
                            download_id: task_id,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                        });

                        // Call the original callback
                        callback(downloaded, total);
                    });
                } else {
                    // Default callback that just updates the status
                    let statuses_for_callback = statuses_clone.clone();
                    let progress_tx_for_callback = progress_tx_clone.clone();

                    fetcher = fetcher.with_progress_callback(move |downloaded, total| {
                        let statuses_for_callback = statuses_for_callback.clone();
                        let progress_tx = progress_tx_for_callback.clone();

                        tokio::task::spawn_blocking(move || {
                            let mut statuses = statuses_for_callback.blocking_lock();
                            statuses.insert(
                                task_id,
                                DownloadStatus::Downloading {
                                    downloaded_bytes: downloaded,
                                    total_bytes: total,
                                },
                            );
                        });

                        // Emit progress event for stream-based API
                        let _ = progress_tx.send(ProgressUpdate {
                            download_id: task_id,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                        });
                    });
                }

                // Launch the download in a separate task
                let destination = task.destination.clone();
                let statuses_for_task = statuses_clone.clone();
                let tasks_for_task = tasks_clone.clone();
                let completion_tx_for_task = completion_tx_clone.clone();

                let handle = tokio::spawn(async move {
                    // The permit will be released automatically when it is drop at the end of this closure
                    let _permit = permit;

                    // Download the file
                    let result = fetcher.fetch_asset(&destination).await;

                    // Update status based on result and notify completion
                    let final_status = match &result {
                        Ok(_) => DownloadStatus::Completed,
                        Err(e) => DownloadStatus::Failed {
                            reason: e.to_string(),
                        },
                    };

                    {
                        let mut statuses = statuses_for_task.lock().await;
                        statuses.insert(task_id, final_status.clone());
                    }

                    // Notify completion via broadcast channel (event-driven, no polling needed)
                    let _ = completion_tx_for_task.send((task_id, final_status));

                    // Remove the task from the list of tasks in progress
                    let mut tasks = tasks_for_task.lock().await;
                    tasks.remove(&task_id);

                    result
                });

                // Store the task handle
                {
                    let mut tasks = tasks_clone.lock().await;
                    tasks.insert(task_id, handle);
                }

                // Continue processing the queue if there are remaining tasks
                let queue_empty = {
                    let queue = queue_clone.lock().await;
                    queue.is_empty()
                };

                if queue_empty {
                    break;
                }
            }
        });
    }
}
