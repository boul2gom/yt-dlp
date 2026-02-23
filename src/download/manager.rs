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
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, Semaphore, broadcast};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use typed_builder::TypedBuilder;

/// Per-task byte counters used by the progress callback (downloaded, total).
type ProgressCounters = Arc<std::sync::Mutex<HashMap<u64, (Arc<AtomicU64>, Arc<AtomicU64>)>>>;

// Download manager default configuration constants
const DEFAULT_RETRY_ATTEMPTS: usize = 3;
const DEFAULT_CLEANUP_THRESHOLD: usize = 1000; // Cleanup after 1000 entries

/// Download priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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
    /// Converts an integer to priority
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
#[derive(Debug, Clone, TypedBuilder)]
pub struct ManagerConfig {
    /// Maximum number of concurrent downloads
    #[builder(default = SpeedProfile::default().max_concurrent_downloads())]
    pub max_concurrent_downloads: usize,
    /// Segment size for parallel download (in bytes)
    #[builder(default = SpeedProfile::default().segment_size())]
    pub segment_size: usize,
    /// Number of parallel segments per download
    #[builder(default = SpeedProfile::default().parallel_segments())]
    pub parallel_segments: usize,
    /// Number of download attempts in case of failure
    #[builder(default = DEFAULT_RETRY_ATTEMPTS)]
    pub retry_attempts: usize,
    /// Maximum buffer size per download (in bytes)
    #[builder(default = SpeedProfile::default().max_buffer_size())]
    pub max_buffer_size: usize,
    /// Optional proxy configuration
    #[builder(default)]
    pub proxy: Option<ProxyConfig>,
    /// Speed profile for automatic optimization
    #[builder(default)]
    pub speed_profile: SpeedProfile,
    /// Threshold for automatic cleanup of finished downloads
    #[builder(default = DEFAULT_CLEANUP_THRESHOLD)]
    pub cleanup_threshold: usize,
    /// Optional User-Agent string
    #[builder(default)]
    pub user_agent: Option<String>,
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
            cleanup_threshold: DEFAULT_CLEANUP_THRESHOLD,
            user_agent: None,
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
    /// Optional event bus for emitting download events
    event_bus: Option<crate::events::EventBus>,
    /// Per-task atomic byte counters: (downloaded, total); avoids locking on every chunk
    progress_counters: ProgressCounters,
    /// Signals the single worker task that new items were enqueued
    worker_notify: Arc<tokio::sync::Notify>,
    /// Guards against spawning more than one worker task at a time
    worker_started: Arc<AtomicBool>,
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
    /// Returns the number of parallel segments configured for downloads.
    pub fn parallel_segments(&self) -> usize {
        self.config.parallel_segments
    }

    /// Returns the segment size (in bytes) configured for downloads.
    pub fn segment_size(&self) -> usize {
        self.config.segment_size
    }

    /// Returns the number of retry attempts configured for downloads.
    pub fn retry_attempts(&self) -> usize {
        self.config.retry_attempts
    }

    /// Create a new download manager with default configuration
    pub fn new() -> Self {
        Self::with_config(ManagerConfig::default())
    }

    /// Create a new download manager with custom configuration
    pub fn with_config(config: ManagerConfig) -> Self {
        Self::with_config_and_event_bus(config, None)
    }

    /// Create a new download manager with custom configuration and event bus
    ///
    /// # Arguments
    ///
    /// * `config` - The download manager configuration
    /// * `event_bus` - Optional event bus for emitting download events
    pub fn with_config_and_event_bus(
        config: ManagerConfig,
        event_bus: Option<crate::events::EventBus>,
    ) -> Self {
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
            event_bus,
            progress_counters: Arc::new(std::sync::Mutex::new(HashMap::new())),
            worker_notify: Arc::new(tokio::sync::Notify::new()),
            worker_started: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Emits an event if an event bus is configured
    fn emit_event(&self, event: crate::events::DownloadEvent) {
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::download::manager::{DownloadManager, ManagerConfig};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let manager = DownloadManager::new();
    /// let id = manager.enqueue("https://example.com", "output.mp4", None).await;
    /// # }
    /// ```
    pub async fn enqueue(
        &self,
        url: impl AsRef<str>,
        destination: impl Into<PathBuf>,
        priority: Option<DownloadPriority>,
    ) -> u64 {
        let mut id_guard = self.next_id.lock().await;
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        let url_str = url.as_ref().to_string();
        let destination_path = destination.into();
        let task_priority = priority.unwrap_or(DownloadPriority::Normal);

        let task = DownloadTask {
            url: url_str.clone(),
            destination: destination_path.clone(),
            priority: task_priority,
            id,
            progress_callback: None,
        };

        tracing::debug!(
            "Enqueuing download {} for {} -> {:?} (priority: {:?})",
            id,
            url_str,
            destination_path,
            task_priority
        );

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

        // Emit DownloadQueued event
        self.emit_event(crate::events::DownloadEvent::DownloadQueued {
            download_id: id,
            url: url_str,
            priority: task_priority,
            output_path: destination_path,
        });

        // Wake the single worker (M1 fix: no new spawn per enqueue)
        self.worker_notify.notify_one();
        self.ensure_worker();

        // Auto-cleanup if needed
        if id % 100 == 0 {
            // Check every 100 downloads to avoid locking too often
            let status_count = {
                let statuses = self.statuses.lock().await;
                statuses.len()
            };

            if status_count > self.config.cleanup_threshold {
                self.cleanup_finished().await;
            }
        }

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
        destination: impl Into<PathBuf>,
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
            destination: destination.into(),
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

        // Wake the single worker (M1 fix: no new spawn per enqueue)
        self.worker_notify.notify_one();
        self.ensure_worker();

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
        let status = statuses.get(&id)?;

        // For active downloads, read live byte counts from the atomic counters (M2 fix)
        if matches!(status, DownloadStatus::Downloading { .. }) {
            let counters = self.progress_counters.lock().unwrap();
            if let Some((dl, total)) = counters.get(&id) {
                return Some(DownloadStatus::Downloading {
                    downloaded_bytes: dl.load(AtomicOrdering::Relaxed),
                    total_bytes: total.load(AtomicOrdering::Relaxed),
                });
            }
        }

        Some(status.clone())
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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::download::manager::{DownloadManager, ManagerConfig};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let manager = DownloadManager::new();
    /// let id = manager.enqueue("https://example.com", "out.mp4", None).await;
    /// let cancelled = manager.cancel(id).await;
    /// assert!(cancelled);
    /// # }
    /// ```
    pub async fn cancel(&self, id: u64) -> bool {
        tracing::debug!(download_id = id, "Cancelling download");

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

            // Emit DownloadCanceled event
            self.emit_event(crate::events::DownloadEvent::DownloadCanceled {
                download_id: id,
                reason: "Cancelled by user".to_string(),
            });

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

            // Emit DownloadCanceled event
            self.emit_event(crate::events::DownloadEvent::DownloadCanceled {
                download_id: id,
                reason: "Cancelled before download started".to_string(),
            });

            return true;
        }

        // Even if not found in queue or tasks, it might be in the brief window
        // between being popped and starting execution, so mark as cancelled
        let mut statuses = self.statuses.lock().await;
        statuses.insert(id, DownloadStatus::Canceled);

        // Emit DownloadCanceled event
        self.emit_event(crate::events::DownloadEvent::DownloadCanceled {
            download_id: id,
            reason: "Cancelled during initialization".to_string(),
        });

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
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::download::manager::{DownloadManager, ManagerConfig};
    /// # #[tokio::main]
    /// # async fn main() {
    /// let manager = DownloadManager::new();
    /// let id = manager.enqueue("https://example.com", "out.mp4", None).await;
    /// if let Some(status) = manager.wait_for_completion(id).await {
    ///     println!("Download finished with status: {:?}", status);
    /// }
    /// # }
    /// ```
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
    /// use yt_dlp::download::manager::{DownloadManager, ManagerConfig};
    /// use tokio_stream::StreamExt;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let manager = DownloadManager::with_config_and_event_bus(ManagerConfig::default(), None);
    ///
    ///     let download_id = manager.enqueue("https://example.com/file", "output", None).await;
    ///     let mut progress_stream = manager.progress_stream(download_id);
    ///
    ///     while let Some(update) = progress_stream.next().await {
    ///         println!("Downloaded: {}/{} bytes ({:.1}%)",
    ///             update.downloaded_bytes,
    ///             update.total_bytes,
    ///             (update.downloaded_bytes as f64 / update.total_bytes as f64) * 100.0
    ///         );
    ///     }
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

    /// Ensures the single background worker task is running.
    ///
    /// Uses a compare-exchange on `worker_started` so that at most one worker
    /// is ever live. The worker loops forever: it drains the queue until empty,
    /// then sleeps on `worker_notify` waiting for the next `enqueue` signal.
    fn ensure_worker(&self) {
        // Only one worker at a time — if already running, the notify_one() above is enough
        if self
            .worker_started
            .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
            .is_err()
        {
            return;
        }

        tracing::debug!(
            max_concurrent = self.config.max_concurrent_downloads,
            "Starting download queue worker"
        );

        let queue = self.queue.clone();
        let semaphore = self.semaphore.clone();
        let statuses = self.statuses.clone();
        let tasks = self.tasks.clone();
        let config = self.config.clone();
        let cancelled = self.cancelled.clone();
        let completion_tx = self.completion_tx.clone();
        let progress_tx = self.progress_tx.clone();
        let event_bus = self.event_bus.clone();
        let notify = self.worker_notify.clone();
        let progress_counters = self.progress_counters.clone();

        tokio::spawn(async move {
            loop {
                // --- Drain phase: process tasks until the queue is empty ---
                loop {
                    // Block until a download slot is free
                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => return, // Semaphore closed; shut down
                    };

                    let task = { queue.lock().await.pop() };

                    let task = match task {
                        Some(t) => t,
                        None => {
                            drop(permit);
                            break; // Queue empty — exit drain loop
                        }
                    };

                    tracing::debug!(
                        task_id = task.id,
                        url = %task.url,
                        destination = ?task.destination,
                        priority = ?task.priority,
                        "Popped task from download queue"
                    );

                    // Skip tasks that were cancelled before they started
                    {
                        let cancelled = cancelled.lock().await;
                        if cancelled.contains(&task.id) {
                            drop(permit);
                            continue;
                        }
                    }

                    // Transition to Downloading state
                    {
                        let mut statuses = statuses.lock().await;
                        statuses.insert(
                            task.id,
                            DownloadStatus::Downloading {
                                downloaded_bytes: 0,
                                total_bytes: 0,
                            },
                        );
                    }

                    // Emit DownloadStarted event
                    if let Some(ref bus) = event_bus {
                        bus.emit(crate::events::DownloadEvent::DownloadStarted {
                            download_id: task.id,
                            url: task.url.clone(),
                            total_bytes: 0,
                            format_id: None,
                        });
                    }

                    // Build the fetcher
                    let fetcher_result = Fetcher::new(
                        &task.url,
                        config.proxy.as_ref(),
                        config.user_agent.clone(),
                    );

                    let mut fetcher = match fetcher_result {
                        Ok(f) => f,
                        Err(e) => {
                            let reason = e.to_string();
                            {
                                let mut statuses = statuses.lock().await;
                                statuses.insert(
                                    task.id,
                                    DownloadStatus::Failed {
                                        reason: reason.clone(),
                                    },
                                );
                            }
                            if let Some(ref bus) = event_bus {
                                bus.emit(crate::events::DownloadEvent::DownloadFailed {
                                    download_id: task.id,
                                    url: task.url.clone(),
                                    error: reason.clone(),
                                    retry_count: 0,
                                });
                            }
                            let _ = completion_tx
                                .send((task.id, DownloadStatus::Failed { reason }));
                            continue;
                        }
                    };

                    fetcher = fetcher
                        .with_segment_size(config.segment_size)
                        .with_parallel_segments(config.parallel_segments)
                        .with_retry_attempts(config.retry_attempts)
                        .with_speed_profile(config.speed_profile);

                    // --- M2 fix: per-task AtomicU64 counters replace spawn_blocking ---
                    let task_id = task.id;
                    let dl_counter = Arc::new(AtomicU64::new(0));
                    let total_counter = Arc::new(AtomicU64::new(0));

                    {
                        let mut counters = progress_counters.lock().unwrap();
                        counters.insert(task_id, (dl_counter.clone(), total_counter.clone()));
                    }

                    let dl_for_cb = dl_counter.clone();
                    let total_for_cb = total_counter.clone();
                    let progress_tx_for_cb = progress_tx.clone();
                    let event_bus_for_cb = event_bus.clone();
                    let user_callback = task.progress_callback.clone();
                    let speed_start_nanos = Arc::new(AtomicU64::new(0));

                    fetcher = fetcher.with_progress_callback(move |downloaded, total| {
                        // Lock-free update — no spawn_blocking needed
                        dl_for_cb.store(downloaded, AtomicOrdering::Relaxed);
                        total_for_cb.store(total, AtomicOrdering::Relaxed);

                        // Compute speed using a start-time recorded on the first callback
                        let now_nanos = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64;
                        let start_nanos = speed_start_nanos.compare_exchange(
                            0,
                            now_nanos,
                            AtomicOrdering::Relaxed,
                            AtomicOrdering::Relaxed,
                        ).unwrap_or_else(|current| current);
                        let elapsed_nanos = now_nanos.saturating_sub(start_nanos);
                        let speed = if elapsed_nanos > 0 {
                            downloaded as f64 / (elapsed_nanos as f64 / 1_000_000_000.0)
                        } else {
                            0.0
                        };

                        let _ = progress_tx_for_cb.send(ProgressUpdate {
                            download_id: task_id,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                        });

                        if let Some(ref bus) = event_bus_for_cb {
                            bus.emit(crate::events::DownloadEvent::DownloadProgress {
                                download_id: task_id,
                                downloaded_bytes: downloaded,
                                total_bytes: total,
                                speed_bytes_per_sec: speed,
                                eta_seconds: None,
                            });
                        }

                        if let Some(ref callback) = user_callback {
                            callback(downloaded, total);
                        }
                    });

                    // Spawn the actual download task
                    let destination = task.destination.clone();
                    let task_url = task.url.clone();
                    let statuses_for_task = statuses.clone();
                    let tasks_for_task = tasks.clone();
                    let cancelled_for_task = cancelled.clone();
                    let completion_tx_for_task = completion_tx.clone();
                    let event_bus_for_task = event_bus.clone();
                    let progress_counters_for_task = progress_counters.clone();

                    let handle = tokio::spawn(async move {
                        // Permit is released automatically when this task ends
                        let _permit = permit;

                        let start_time = std::time::Instant::now();

                        tracing::debug!(
                            task_id = task_id,
                            url = %task_url,
                            destination = ?destination,
                            "Starting download attempt"
                        );

                        let result = fetcher.fetch_asset(&destination).await;
                        let duration = start_time.elapsed();

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

                        // Remove per-task counters once the download is finished
                        {
                            let mut counters = progress_counters_for_task.lock().unwrap();
                            counters.remove(&task_id);
                        }

                        if let Some(ref bus) = event_bus_for_task {
                            match &final_status {
                                DownloadStatus::Completed => {
                                    let total_bytes = tokio::fs::metadata(&destination)
                                        .await
                                        .map(|m| m.len())
                                        .unwrap_or(0);

                                    bus.emit(crate::events::DownloadEvent::DownloadCompleted {
                                        download_id: task_id,
                                        url: task_url.clone(),
                                        output_path: destination.clone(),
                                        duration,
                                        total_bytes,
                                    });
                                }
                                DownloadStatus::Failed { reason } => {
                                    bus.emit(crate::events::DownloadEvent::DownloadFailed {
                                        download_id: task_id,
                                        url: task_url.clone(),
                                        error: reason.clone(),
                                        retry_count: 0,
                                    });
                                }
                                _ => {}
                            }
                        }

                        let _ = completion_tx_for_task.send((task_id, final_status));

                        {
                            let mut tasks = tasks_for_task.lock().await;
                            tasks.remove(&task_id);
                        }

                        // Remove from statuses and cancelled now that all waiters have been notified
                        {
                            let mut statuses = statuses_for_task.lock().await;
                            statuses.remove(&task_id);
                        }
                        {
                            let mut cancelled = cancelled_for_task.lock().await;
                            cancelled.remove(&task_id);
                        }

                        result
                    });

                    {
                        let mut tasks = tasks.lock().await;
                        tasks.insert(task_id, handle);
                    }
                }

                // Queue drained — wait for the next enqueue signal before looping
                notify.notified().await;
            }
        });
    }
}
