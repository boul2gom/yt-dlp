//! Download manager with priority queue and concurrent downloads limitation.
//!
//! This module provides a download manager that allows:
//! - Limiting the number of concurrent downloads
//! - Managing a download queue with priorities
//! - Resuming interrupted downloads
//! - Optimizing memory usage

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore, broadcast};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use crate::download::config::speed_profile::SpeedProfile;
// Re-export types for backward compatibility with `use crate::download::manager::*`
pub use crate::download::types::{DownloadPriority, DownloadStatus, ManagerConfig, ProgressUpdate};
use crate::download::types::{DownloadTask, ProgressCallback, ProgressCounters};
use crate::download::worker::{
    WorkerContext, build_progress_callback, emit_bus_event, prepare_task_fetcher, run_download_task,
};
use crate::error::Result;
use crate::model::format::HttpHeaders;

/// Download manager
pub struct DownloadManager {
    /// Download manager configuration
    config: ManagerConfig,
    /// Shared HTTP client with connection pooling (avoids per-download client creation)
    client: Arc<reqwest::Client>,
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
    /// Token to gracefully shut down the worker loop
    shutdown_token: CancellationToken,
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

    /// Returns the speed profile configured for this download manager.
    pub fn speed_profile(&self) -> SpeedProfile {
        self.config.speed_profile
    }

    /// Create a new download manager with default configuration
    pub fn new() -> Self {
        tracing::debug!("⚙️ Creating download manager with default config");
        Self::with_config(ManagerConfig::default())
    }

    /// Create a new download manager with custom configuration
    pub fn with_config(config: ManagerConfig) -> Self {
        tracing::debug!(config = %config, "⚙️ Creating download manager with config");
        Self::with_config_and_event_bus(config, None)
    }

    /// Create a new download manager with custom configuration and event bus
    ///
    /// # Arguments
    ///
    /// * `config` - The download manager configuration
    /// * `event_bus` - Optional event bus for emitting download events
    pub fn with_config_and_event_bus(config: ManagerConfig, event_bus: Option<crate::events::EventBus>) -> Self {
        tracing::debug!(config = %config, has_event_bus = event_bus.is_some(), "⚙️ Initializing download manager");
        let (completion_tx, _) = broadcast::channel(100);
        let (progress_tx, _) = broadcast::channel(1000); // Larger buffer for frequent progress updates

        // Build a shared HTTP client from config so all downloads reuse
        // connection pools, TLS sessions, and DNS caches
        let client = Self::build_shared_client(&config);

        Self {
            config: config.clone(),
            client,
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
            shutdown_token: CancellationToken::new(),
        }
    }

    /// Returns the shared HTTP client used by all downloads.
    ///
    /// This client is configured with connection pooling, TLS session caching,
    /// and any proxy settings from the manager configuration. Passing it to
    /// [`Fetcher::with_client`] avoids the cost of rebuilding these resources
    /// per download.
    pub fn client(&self) -> &Arc<reqwest::Client> {
        &self.client
    }

    /// Builds the base HTTP client from the manager config.
    fn build_shared_client(config: &ManagerConfig) -> Arc<reqwest::Client> {
        let default_headers = config
            .user_agent
            .as_ref()
            .map(|ua| crate::model::format::HttpHeaders::browser_defaults(ua.clone()).to_header_map());

        let http_config = crate::utils::http::HttpClientConfig {
            proxy: config.proxy.as_ref(),
            user_agent: config.user_agent.clone(),
            default_headers,
            http2_adaptive_window: true,
            ..Default::default()
        };

        crate::utils::http::build_http_client(http_config).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to build configured HTTP client, falling back to default");
            Arc::new(reqwest::Client::new())
        })
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
    /// let id = manager
    ///     .enqueue("https://example.com", "output.mp4", None)
    ///     .await;
    /// # }
    /// ```
    pub async fn enqueue(
        &self,
        url: impl AsRef<str>,
        destination: impl Into<PathBuf>,
        priority: Option<DownloadPriority>,
    ) -> u64 {
        self.enqueue_internal(
            url.as_ref().to_string(),
            destination.into(),
            priority.unwrap_or(DownloadPriority::Normal),
            None,
            None,
            None,
        )
        .await
    }

    /// Add a download to the queue with specific HTTP headers from yt-dlp
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download
    /// * `destination` - The destination path
    /// * `priority` - The download priority (optional, default Normal)
    /// * `http_headers` - The headers to use
    ///
    /// # Returns
    ///
    /// The ID of the download
    pub async fn enqueue_with_headers(
        &self,
        url: impl AsRef<str>,
        destination: impl Into<PathBuf>,
        priority: Option<DownloadPriority>,
        http_headers: Option<crate::model::format::HttpHeaders>,
    ) -> u64 {
        self.enqueue_internal(
            url.as_ref().to_string(),
            destination.into(),
            priority.unwrap_or(DownloadPriority::Normal),
            None,
            http_headers,
            None,
        )
        .await
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
        self.enqueue_internal(
            url.as_ref().to_string(),
            destination.into(),
            priority.unwrap_or(DownloadPriority::Normal),
            Some(Arc::new(progress_callback)),
            None,
            None,
        )
        .await
    }

    /// Add a download to the queue with a progress callback and specific HTTP headers
    pub async fn enqueue_with_progress_and_headers<F>(
        &self,
        url: impl AsRef<str>,
        destination: impl Into<PathBuf>,
        priority: Option<DownloadPriority>,
        progress_callback: F,
        http_headers: Option<HttpHeaders>,
    ) -> u64
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        self.enqueue_internal(
            url.as_ref().to_string(),
            destination.into(),
            priority.unwrap_or(DownloadPriority::Normal),
            Some(Arc::new(progress_callback)),
            http_headers,
            None,
        )
        .await
    }

    /// Enqueues a partial download covering only `[byte_start, byte_end]` of the URL.
    ///
    /// The destination file will contain exactly `byte_end - byte_start + 1` bytes,
    /// downloaded in parallel segments using the manager's speed profile and retry policy.
    /// Progress events are emitted on the broadcast channel and the event bus as usual.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to download from.
    /// * `destination` - Output file path.
    /// * `byte_start` - First byte to download (URL-absolute, inclusive).
    /// * `byte_end` - Last byte to download (URL-absolute, inclusive).
    /// * `priority` - Queue priority (default: Normal).
    /// * `http_headers` - Optional format-specific HTTP headers.
    ///
    /// # Returns
    ///
    /// The download ID, usable with [`wait_for_completion`] and [`progress_stream`].
    pub async fn enqueue_range(
        &self,
        url: impl AsRef<str>,
        destination: impl Into<PathBuf>,
        byte_start: u64,
        byte_end: u64,
        priority: Option<DownloadPriority>,
        http_headers: Option<HttpHeaders>,
    ) -> u64 {
        self.enqueue_internal(
            url.as_ref().to_string(),
            destination.into(),
            priority.unwrap_or(DownloadPriority::Normal),
            None,
            http_headers,
            Some((byte_start, byte_end)),
        )
        .await
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
        tracing::debug!(download_id = id, "⚙️ Getting download status");
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
        tracing::debug!("⚙️ Cleaning up finished downloads");
        let mut statuses = self.statuses.lock().await;
        let mut cancelled = self.cancelled.lock().await;

        // Collect IDs to remove
        let ids_to_remove: Vec<u64> = statuses
            .iter()
            .filter_map(|(id, status)| match status {
                DownloadStatus::Completed | DownloadStatus::Failed { .. } | DownloadStatus::Canceled => Some(*id),
                _ => None,
            })
            .collect();

        // Remove from statuses and cancelled
        for id in &ids_to_remove {
            statuses.remove(id);
            cancelled.remove(id);
        }
        drop(statuses);
        drop(cancelled);

        // Also clean task handles and progress counters for finished downloads
        let mut tasks = self.tasks.lock().await;
        let mut counters = self.progress_counters.lock().unwrap_or_else(|e| e.into_inner());
        for id in &ids_to_remove {
            tasks.remove(id);
            counters.remove(id);
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
    /// let id = manager
    ///     .enqueue("https://example.com", "out.mp4", None)
    ///     .await;
    /// let cancelled = manager.cancel(id).await;
    /// assert!(cancelled);
    /// # }
    /// ```
    pub async fn cancel(&self, id: u64) -> bool {
        tracing::debug!(download_id = id, "📥 Cancelling download");

        {
            self.cancelled.lock().await.insert(id);
        }

        // Clean up progress counters to prevent leaks
        {
            self.progress_counters.lock().unwrap().remove(&id);
        }

        // Check if the download is in progress
        let task_handle = { self.tasks.lock().await.remove(&id) };
        if let Some(handle) = task_handle {
            handle.abort();
            self.mark_cancelled_and_emit(id, "Cancelled by user").await;
            return true;
        }

        // Check if the download is in the queue
        if self.remove_from_queue(id).await {
            self.mark_cancelled_and_emit(id, "Cancelled before download started")
                .await;
            return true;
        }

        // Not found — the ID was never enqueued or already completed
        false
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
    /// let id = manager
    ///     .enqueue("https://example.com", "out.mp4", None)
    ///     .await;
    /// if let Some(status) = manager.wait_for_completion(id).await {
    ///     println!("Download finished with status: {:?}", status);
    /// }
    /// # }
    /// ```
    pub async fn wait_for_completion(&self, id: u64) -> Option<DownloadStatus> {
        tracing::debug!(download_id = id, "📥 Waiting for download completion");

        // Subscribe BEFORE checking status to avoid TOCTOU race:
        // if the download completes between the check and the subscribe,
        // the broadcast would be missed and the caller would block forever.
        let mut rx = self.completion_tx.subscribe();

        // Now check if the download already completed
        if let Some(status) = self.get_status(id).await
            && is_terminal_status(&status)
        {
            return Some(status);
        }

        // Wait for the completion event for this specific download
        loop {
            match rx.recv().await {
                Ok((download_id, status)) if download_id == id => {
                    if is_terminal_status(&status) {
                        return Some(status);
                    }
                    continue;
                }
                Ok(_) => continue, // Event for a different download
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Channel lagged, check current status
                    if let Some(status) = self.get_status(id).await {
                        if is_terminal_status(&status) {
                            return Some(status);
                        }
                        continue;
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
    /// use yt_dlp::download::manager::{DownloadManager, ManagerConfig};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let manager = DownloadManager::with_config_and_event_bus(ManagerConfig::default(), None);
    ///
    ///     let download_id = manager
    ///         .enqueue("https://example.com/file", "output", None)
    ///         .await;
    ///     let mut progress_stream = manager.progress_stream(download_id);
    ///
    ///     while let Some(update) = progress_stream.next().await {
    ///         println!(
    ///             "Downloaded: {}/{} bytes ({:.1}%)",
    ///             update.downloaded_bytes,
    ///             update.total_bytes,
    ///             (update.downloaded_bytes as f64 / update.total_bytes as f64) * 100.0
    ///         );
    ///     }
    /// }
    /// ```
    pub fn progress_stream(&self, id: u64) -> impl Stream<Item = ProgressUpdate> + Send + 'static {
        tracing::debug!(download_id = id, "📥 Subscribing to progress stream");
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
        tracing::debug!("📥 Subscribing to all progress streams");
        let rx = self.progress_tx.subscribe();

        BroadcastStream::new(rx).filter_map(|result| result.ok())
    }

    /// Emits an event if an event bus is configured
    fn emit_event(&self, event: crate::events::DownloadEvent) {
        tracing::trace!(event = ?event, "🔔 Emitting download event");
        if let Some(ref bus) = self.event_bus {
            bus.emit(event);
        }
    }

    async fn mark_cancelled_and_emit(&self, id: u64, reason: &str) {
        let mut statuses = self.statuses.lock().await;
        statuses.insert(id, DownloadStatus::Canceled);
        drop(statuses);

        self.emit_event(crate::events::DownloadEvent::DownloadCanceled {
            download_id: id,
            reason: reason.to_string(),
        });
    }

    async fn remove_from_queue(&self, id: u64) -> bool {
        let mut queue = self.queue.lock().await;
        let len_before = queue.len();

        let mut new_queue = BinaryHeap::new();
        for task in queue.drain() {
            if task.id != id {
                new_queue.push(task);
            }
        }
        *queue = new_queue;

        len_before > queue.len()
    }

    async fn enqueue_internal(
        &self,
        url: String,
        destination: PathBuf,
        priority: DownloadPriority,
        progress_callback: Option<ProgressCallback>,
        http_headers: Option<crate::model::format::HttpHeaders>,
        range_constraint: Option<(u64, u64)>,
    ) -> u64 {
        let mut id_guard = self.next_id.lock().await;
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        let task = DownloadTask {
            url: url.clone(),
            destination: destination.clone(),
            priority,
            id,
            progress_callback,
            http_headers,
            range_constraint,
        };

        tracing::debug!(id = id, url = url, destination = ?destination, priority = ?priority, "📥 Enqueuing download");

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
            url,
            priority,
            output_path: destination,
        });

        // Wake the single worker
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
            "⚙️ Starting download queue worker"
        );

        let ctx = WorkerLoopCtx {
            queue: self.queue.clone(),
            semaphore: self.semaphore.clone(),
            statuses: self.statuses.clone(),
            tasks: self.tasks.clone(),
            config: self.config.clone(),
            cancelled: self.cancelled.clone(),
            completion_tx: self.completion_tx.clone(),
            progress_tx: self.progress_tx.clone(),
            event_bus: self.event_bus.clone(),
            notify: self.worker_notify.clone(),
            progress_counters: self.progress_counters.clone(),
            shared_client: Arc::clone(&self.client),
            shutdown: self.shutdown_token.clone(),
        };
        tokio::spawn(run_worker_loop(ctx));
    }

    /// Shuts down the worker task gracefully.
    ///
    /// # Returns
    ///
    /// Nothing. After calling this, no new tasks will be processed.
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }
}

impl std::fmt::Debug for DownloadManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadManager")
            .field("config", &self.config)
            .field("max_concurrent_downloads", &self.config.max_concurrent_downloads)
            .finish_non_exhaustive()
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DownloadManager {
    fn drop(&mut self) {
        self.shutdown_token.cancel();

        // Abort all in-flight download tasks to prevent resource leaks
        if let Ok(tasks) = self.tasks.try_lock() {
            for (_, handle) in tasks.iter() {
                handle.abort();
            }
        }
    }
}

/// Returns `true` when `status` is a terminal state (no further transitions possible).
fn is_terminal_status(status: &DownloadStatus) -> bool {
    matches!(
        status,
        DownloadStatus::Completed | DownloadStatus::Failed { .. } | DownloadStatus::Canceled
    )
}

/// All state captured by the background worker task.
#[derive(Debug)]
struct WorkerLoopCtx {
    queue: Arc<Mutex<BinaryHeap<DownloadTask>>>,
    semaphore: Arc<Semaphore>,
    statuses: Arc<Mutex<HashMap<u64, DownloadStatus>>>,
    tasks: Arc<Mutex<HashMap<u64, JoinHandle<Result<()>>>>>,
    config: ManagerConfig,
    cancelled: Arc<Mutex<HashSet<u64>>>,
    completion_tx: broadcast::Sender<(u64, DownloadStatus)>,
    progress_tx: broadcast::Sender<ProgressUpdate>,
    event_bus: Option<crate::events::EventBus>,
    notify: Arc<tokio::sync::Notify>,
    progress_counters: ProgressCounters,
    shared_client: Arc<reqwest::Client>,
    shutdown: CancellationToken,
}

/// Runs the worker drain loop: processes queued tasks until empty, then sleeps.
async fn run_worker_loop(ctx: WorkerLoopCtx) {
    loop {
        if ctx.shutdown.is_cancelled() {
            tracing::debug!("🛑 Worker shutting down");
            return;
        }

        // --- Drain phase: process tasks until the queue is empty ---
        loop {
            let permit = match ctx.semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => return, // Semaphore closed; shut down
            };

            let Some(task) = ctx.queue.lock().await.pop() else {
                drop(permit);
                break; // Queue empty — exit drain loop
            };

            tracing::debug!(
                task_id = task.id,
                url = %task.url,
                destination = ?task.destination,
                priority = ?task.priority,
                "⚙️ Popped task from download queue"
            );

            let worker_ctx = WorkerContext {
                statuses: ctx.statuses.clone(),
                tasks: ctx.tasks.clone(),
                cancelled: ctx.cancelled.clone(),
                completion_tx: ctx.completion_tx.clone(),
                event_bus: ctx.event_bus.clone(),
                progress_counters: ctx.progress_counters.clone(),
            };
            process_queued_task(
                task,
                &ctx.config,
                &ctx.shared_client,
                permit,
                ctx.progress_tx.clone(),
                worker_ctx,
            )
            .await;
        }

        // Queue drained — wait for the next enqueue signal or shutdown
        tokio::select! {
            _ = ctx.notify.notified() => {}
            _ = ctx.shutdown.cancelled() => {
                tracing::debug!("🛑 Worker shutting down");
                return;
            }
        }
    }
}

/// Processes a single task that has been popped from the download queue.
///
/// Handles the full lifecycle of one queued download: cancelled-check, status
/// update, event emission, fetcher construction, progress callback wiring, and
/// task spawning. Accepts an already-acquired semaphore permit that is forwarded
/// to the spawned `run_download_task` and released when that task completes.
///
/// # Arguments
///
/// * `task` - The popped [`DownloadTask`] to process.
/// * `config` - The download manager configuration.
/// * `client` - The shared HTTP client.
/// * `permit` - An acquired semaphore permit forwarded to the download task.
/// * `progress_tx` - Sender for progress broadcast updates.
/// * `ctx` - Shared worker state (statuses, tasks, cancelled, bus, etc.).
async fn process_queued_task(
    task: DownloadTask,
    config: &ManagerConfig,
    client: &Arc<reqwest::Client>,
    permit: OwnedSemaphorePermit,
    progress_tx: broadcast::Sender<ProgressUpdate>,
    ctx: WorkerContext,
) {
    if ctx.cancelled.lock().await.contains(&task.id) {
        return; // permit drops automatically
    }

    ctx.statuses.lock().await.insert(
        task.id,
        DownloadStatus::Downloading {
            downloaded_bytes: 0,
            total_bytes: 0,
        },
    );

    emit_bus_event(
        &ctx.event_bus,
        crate::events::DownloadEvent::DownloadStarted {
            download_id: task.id,
            url: task.url.clone(),
            total_bytes: 0,
            format_id: None,
        },
    );

    let fetcher = match prepare_task_fetcher(&task, config, client) {
        Ok(f) => f,
        Err(e) => {
            let reason = e.to_string();
            ctx.statuses
                .lock()
                .await
                .insert(task.id, DownloadStatus::Failed { reason: reason.clone() });
            emit_bus_event(
                &ctx.event_bus,
                crate::events::DownloadEvent::DownloadFailed {
                    download_id: task.id,
                    url: task.url.clone(),
                    error: reason.clone(),
                    retry_count: 0,
                },
            );
            let _ = ctx.completion_tx.send((task.id, DownloadStatus::Failed { reason }));
            return;
        }
    };

    let fetcher = fetcher.with_progress_callback(build_progress_callback(
        task.id,
        &ctx.progress_counters,
        progress_tx,
        ctx.event_bus.clone(),
        task.progress_callback.clone(),
    ));

    let task_id = task.id;
    let tasks = ctx.tasks.clone(); // clone before ctx is moved into run_download_task
    let handle = tokio::spawn(run_download_task(
        task_id,
        task.url.clone(),
        task.destination.clone(),
        fetcher,
        permit,
        ctx,
    ));
    tasks.lock().await.insert(task_id, handle);
}
