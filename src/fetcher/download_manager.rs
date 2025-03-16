//! Download manager with priority queue and concurrent downloads limitation.
//!
//! This module provides a download manager that allows:
//! - Limiting the number of concurrent downloads
//! - Managing a download queue with priorities
//! - Resuming interrupted downloads
//! - Optimizing memory usage

use crate::error::Result;
use crate::fetcher::Fetcher;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinHandle;

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
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: 3,
            segment_size: 1024 * 1024 * 5, // 5 MB
            parallel_segments: 4,
            retry_attempts: 3,
            max_buffer_size: 1024 * 1024 * 10, // 10 MB
        }
    }
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
        Self {
            config: config.clone(),
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            next_id: Arc::new(Mutex::new(0)),
            statuses: Arc::new(Mutex::new(HashMap::new())),
            tasks: Arc::new(Mutex::new(HashMap::new())),
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

        false
    }

    /// Wait for a download to complete
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the download to wait for
    ///
    /// # Returns
    ///
    /// The final download status, or None if the ID doesn't exist
    pub async fn wait_for_completion(&self, id: u64) -> Option<DownloadStatus> {
        loop {
            let status = self.get_status(id).await?;

            match status {
                DownloadStatus::Completed
                | DownloadStatus::Failed { .. }
                | DownloadStatus::Canceled => {
                    return Some(status);
                }
                _ => {
                    // Wait a little before checking again
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Process the download queue
    fn process_queue(&self) {
        let queue_clone = self.queue.clone();
        let semaphore_clone = self.semaphore.clone();
        let statuses_clone = self.statuses.clone();
        let tasks_clone = self.tasks.clone();
        let config_clone = self.config.clone();

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
                let mut fetcher = Fetcher::new(&task.url)
                    .with_segment_size(config_clone.segment_size)
                    .with_parallel_segments(config_clone.parallel_segments)
                    .with_retry_attempts(config_clone.retry_attempts);

                // Add progress callback if available
                let task_id = task.id;
                let statuses_for_callback = statuses_clone.clone();

                if let Some(callback) = task.progress_callback {
                    fetcher = fetcher.with_progress_callback(move |downloaded, total| {
                        // Update status with progress
                        let mut statuses = statuses_for_callback.blocking_lock();
                        statuses.insert(
                            task_id,
                            DownloadStatus::Downloading {
                                downloaded_bytes: downloaded,
                                total_bytes: total,
                            },
                        );

                        // Call the original callback
                        callback(downloaded, total);
                    });
                } else {
                    // Default callback that just updates the status
                    let statuses_for_callback = statuses_clone.clone();
                    fetcher = fetcher.with_progress_callback(move |downloaded, total| {
                        let mut statuses = statuses_for_callback.blocking_lock();
                        statuses.insert(
                            task_id,
                            DownloadStatus::Downloading {
                                downloaded_bytes: downloaded,
                                total_bytes: total,
                            },
                        );
                    });
                }

                // Launch the download in a separate task
                let destination = task.destination.clone();
                let statuses_for_task = statuses_clone.clone();
                let tasks_for_task = tasks_clone.clone();

                let handle = tokio::spawn(async move {
                    // The permit will be released automatically when it is drop at the end of this closure
                    let _permit = permit;

                    // Download the file
                    let result = fetcher.fetch_asset(&destination).await;

                    // Update status based on result
                    let mut statuses = statuses_for_task.lock().await;
                    match &result {
                        Ok(_) => {
                            statuses.insert(task_id, DownloadStatus::Completed);
                        }
                        Err(e) => {
                            statuses.insert(
                                task_id,
                                DownloadStatus::Failed {
                                    reason: e.to_string(),
                                },
                            );
                        }
                    }

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
