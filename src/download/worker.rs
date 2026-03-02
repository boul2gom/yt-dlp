//! Worker functions for the download manager.
//!
//! Contains the free functions spawned by `ensure_worker`: task preparation,
//! throttled progress callback construction, download execution and event emission.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;

use crate::download::engine::fetcher::Fetcher;
use crate::download::types::{
    DownloadStatus, DownloadTask, ManagerConfig, ProgressCallback, ProgressCounters, ProgressUpdate,
};
use crate::error::Result;
use crate::model::format::HttpHeaders;

/// Minimum interval between progress event emissions (50 ms).
const PROGRESS_THROTTLE_NANOS: u64 = 50_000_000;

/// Shared mutable state passed from the download manager to each worker task.
pub(super) struct WorkerContext {
    pub(super) statuses: Arc<Mutex<HashMap<u64, DownloadStatus>>>,
    pub(super) tasks: Arc<Mutex<HashMap<u64, JoinHandle<Result<()>>>>>,
    pub(super) cancelled: Arc<Mutex<HashSet<u64>>>,
    pub(super) completion_tx: broadcast::Sender<(u64, DownloadStatus)>,
    pub(super) event_bus: Option<crate::events::EventBus>,
    pub(super) progress_counters: ProgressCounters,
}

pub(super) fn emit_bus_event(event_bus: &Option<crate::events::EventBus>, event: crate::events::DownloadEvent) {
    if let Some(bus) = event_bus {
        bus.emit(event);
    }
}

pub(super) fn prepare_task_fetcher(
    task: &DownloadTask,
    config: &ManagerConfig,
    shared_client: &Arc<reqwest::Client>,
) -> Result<Fetcher> {
    let headers = task
        .http_headers
        .clone()
        .or_else(|| config.user_agent.clone().map(HttpHeaders::browser_defaults));

    let fetcher = match headers {
        None => Fetcher::with_client(&task.url, Arc::clone(shared_client)),
        Some(h) => Fetcher::with_client_and_headers(&task.url, Arc::clone(shared_client), h),
    };

    let mut fetcher = fetcher
        .with_segment_size(config.segment_size)
        .with_parallel_segments(config.parallel_segments)
        .with_retry_attempts(config.retry_attempts)
        .with_speed_profile(config.speed_profile);

    if let Some((start, end)) = task.range_constraint {
        fetcher = fetcher.with_range(start, end);
    }

    Ok(fetcher)
}

pub(super) fn build_progress_callback(
    task_id: u64,
    progress_counters: &ProgressCounters,
    progress_tx: broadcast::Sender<ProgressUpdate>,
    event_bus: Option<crate::events::EventBus>,
    user_callback: Option<ProgressCallback>,
) -> impl Fn(u64, u64) + Send + Sync + 'static {
    let dl_counter = Arc::new(AtomicU64::new(0));
    let total_counter = Arc::new(AtomicU64::new(0));

    {
        let mut counters = progress_counters.lock().unwrap();
        counters.insert(task_id, (dl_counter.clone(), total_counter.clone()));
    }

    let speed_start_nanos = Arc::new(AtomicU64::new(0));
    let last_emit_nanos = Arc::new(AtomicU64::new(0));

    move |downloaded, total| {
        // Lock-free counter update is always performed
        dl_counter.store(downloaded, AtomicOrdering::Relaxed);
        total_counter.store(total, AtomicOrdering::Relaxed);

        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        // Throttle event emission: skip if less than PROGRESS_THROTTLE_NANOS since last emit
        // Always emit for the final update (downloaded == total)
        let prev_emit = last_emit_nanos.load(AtomicOrdering::Relaxed);
        if downloaded != total && now_nanos.saturating_sub(prev_emit) < PROGRESS_THROTTLE_NANOS {
            return;
        }
        last_emit_nanos.store(now_nanos, AtomicOrdering::Relaxed);

        let start_nanos = speed_start_nanos
            .compare_exchange(0, now_nanos, AtomicOrdering::Relaxed, AtomicOrdering::Relaxed)
            .unwrap_or_else(|current| current);
        let elapsed_nanos = now_nanos.saturating_sub(start_nanos);
        let speed = if elapsed_nanos > 0 {
            downloaded as f64 / (elapsed_nanos as f64 / 1_000_000_000.0)
        } else {
            0.0
        };

        let _ = progress_tx.send(ProgressUpdate {
            download_id: task_id,
            downloaded_bytes: downloaded,
            total_bytes: total,
        });

        emit_bus_event(
            &event_bus,
            crate::events::DownloadEvent::DownloadProgress {
                download_id: task_id,
                downloaded_bytes: downloaded,
                total_bytes: total,
                speed_bytes_per_sec: speed,
                eta_seconds: None,
            },
        );

        if let Some(ref callback) = user_callback {
            callback(downloaded, total);
        }
    }
}

pub(super) async fn run_download_task(
    task_id: u64,
    task_url: String,
    destination: PathBuf,
    fetcher: Fetcher,
    permit: tokio::sync::OwnedSemaphorePermit,
    ctx: WorkerContext,
) -> Result<()> {
    let _permit = permit;
    let start_time = std::time::Instant::now();

    tracing::debug!(
        task_id = task_id,
        url = %task_url,
        destination = ?destination,
        "📥 Starting download attempt"
    );

    let result = fetcher.fetch_asset(&destination).await;
    let duration = start_time.elapsed();

    match &result {
        Ok(_) => tracing::info!(task_id = task_id, url = %task_url, ?duration, "✅ Download completed successfully"),
        Err(e) => tracing::warn!(task_id = task_id, url = %task_url, error = %e, ?duration, "Download failed"),
    }

    let final_status = match &result {
        Ok(_) => DownloadStatus::Completed,
        Err(e) => DownloadStatus::Failed { reason: e.to_string() },
    };

    {
        ctx.statuses.lock().await.insert(task_id, final_status.clone());
    }
    {
        ctx.progress_counters.lock().unwrap().remove(&task_id);
    }

    emit_download_result(
        &ctx.event_bus,
        &final_status,
        task_id,
        &task_url,
        &destination,
        duration,
    )
    .await;

    let _ = ctx.completion_tx.send((task_id, final_status));
    {
        ctx.tasks.lock().await.remove(&task_id);
    }
    {
        // Keep final status in the map for post-hoc querying
        ctx.cancelled.lock().await.remove(&task_id);
    }

    result
}

async fn emit_download_result(
    event_bus: &Option<crate::events::EventBus>,
    status: &DownloadStatus,
    task_id: u64,
    url: &str,
    destination: &std::path::Path,
    duration: std::time::Duration,
) {
    let Some(bus) = event_bus else { return };

    match status {
        DownloadStatus::Completed => {
            let total_bytes = tokio::fs::metadata(destination).await.map(|m| m.len()).unwrap_or(0);
            bus.emit(crate::events::DownloadEvent::DownloadCompleted {
                download_id: task_id,
                url: url.to_string(),
                output_path: destination.to_path_buf(),
                duration,
                total_bytes,
            });
        }
        DownloadStatus::Failed { reason } => {
            bus.emit(crate::events::DownloadEvent::DownloadFailed {
                download_id: task_id,
                url: url.to_string(),
                error: reason.clone(),
                retry_count: 0,
            });
        }
        _ => {}
    }
}
