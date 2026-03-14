use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinHandle;

use super::config::TrackerConfig;
use super::inner::{CompletedDownload, DownloadOutcome, InProgressDownload, StatsInner};
use super::snapshot::{
    ActiveDownloadSnapshot, DownloadOutcomeSnapshot, DownloadSnapshot, DownloadStats, FetchStats, GlobalSnapshot,
    PlaylistStats, PostProcessStats,
};
use crate::download::DownloadPriority;
use crate::events::{DownloadEvent, EventBus};

/// Subscribes to the event bus and maintains running statistics about all download
/// and metadata fetch operations.
///
/// The tracker runs a background task that processes events from the [`EventBus`] and
/// updates its internal counters. Use [`snapshot`](StatisticsTracker::snapshot) to
/// retrieve a point-in-time view of the collected data.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::Downloader;
/// # use yt_dlp::client::deps::Libraries;
/// # use std::path::PathBuf;
/// # #[tokio::main]
/// # async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
/// let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
/// let downloader = Downloader::builder(libraries, "output").build().await?;
///
/// // ... perform downloads ...
///
/// let snapshot = downloader.statistics().snapshot().await;
/// println!("Completed: {}", snapshot.downloads.completed);
/// println!("Total bytes: {}", snapshot.downloads.total_bytes);
/// # Ok(())
/// # }
/// ```
pub struct StatisticsTracker {
    inner: Arc<RwLock<StatsInner>>,
    // Kept alive to ensure the background task runs for as long as the tracker lives.
    _task: JoinHandle<()>,
}

impl StatisticsTracker {
    /// Creates a tracker with default configuration and subscribes to `bus`.
    ///
    /// # Arguments
    ///
    /// * `bus` - The event bus to subscribe to.
    ///
    /// # Returns
    ///
    /// A new [`StatisticsTracker`] that is already running its background event loop.
    pub fn new(bus: &EventBus) -> Self {
        Self::with_config(bus, TrackerConfig::default())
    }

    /// Creates a tracker with custom configuration and subscribes to `bus`.
    ///
    /// # Arguments
    ///
    /// * `bus` - The event bus to subscribe to.
    /// * `config` - History-size bounds and other settings.
    ///
    /// # Returns
    ///
    /// A new [`StatisticsTracker`] that is already running its background event loop.
    pub fn with_config(bus: &EventBus, config: TrackerConfig) -> Self {
        tracing::debug!(
            max_history = config.max_download_history,
            "📊 Creating statistics tracker"
        );

        let inner = Arc::new(RwLock::new(StatsInner::new(config)));
        let rx = bus.subscribe();
        let inner_clone = inner.clone();

        let task = tokio::spawn(run_event_loop(inner_clone, rx));

        Self { inner, _task: task }
    }

    /// Returns a point-in-time snapshot of all collected statistics.
    ///
    /// Acquires a read lock on the internal state, computes derived metrics (averages,
    /// rates), and returns a fully-owned [`GlobalSnapshot`]. Subsequent mutations to the
    /// tracker are not reflected in an already-obtained snapshot.
    ///
    /// # Returns
    ///
    /// A [`GlobalSnapshot`] containing aggregate statistics for downloads, fetches,
    /// post-processing, and playlists at the time of the call.
    pub async fn snapshot(&self) -> GlobalSnapshot {
        let inner = self.inner.read().await;
        build_snapshot(&inner)
    }

    /// Returns the number of downloads currently in progress.
    ///
    /// # Returns
    ///
    /// The count of downloads that have been started but have not yet reached a terminal
    /// state (completed, failed, or canceled).
    pub async fn active_count(&self) -> usize {
        self.inner.read().await.in_progress.len()
    }

    /// Returns the total number of successfully completed downloads.
    ///
    /// # Returns
    ///
    /// Cumulative count of downloads that ended with a [`DownloadCompleted`](crate::events::DownloadEvent::DownloadCompleted) event.
    pub async fn completed_count(&self) -> u64 {
        self.inner.read().await.completed
    }

    /// Returns the total number of bytes transferred across all completed downloads.
    ///
    /// # Returns
    ///
    /// Sum of `total_bytes` from every [`DownloadCompleted`](crate::events::DownloadEvent::DownloadCompleted) event received.
    pub async fn total_bytes(&self) -> u64 {
        self.inner.read().await.total_bytes
    }

    /// Resets all counters and history to their initial state, preserving the configuration.
    ///
    /// This is useful for implementing rolling windows or clearing statistics between
    /// logical phases of an application. The tracker continues running and will start
    /// collecting fresh data immediately after the reset.
    pub async fn reset(&self) {
        tracing::debug!("📊 Resetting statistics tracker");

        let mut inner = self.inner.write().await;
        let config = inner.config;
        *inner = StatsInner::new(config);
    }
}

impl std::fmt::Debug for StatisticsTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatisticsTracker").finish_non_exhaustive()
    }
}

impl std::fmt::Display for StatisticsTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StatisticsTracker")
    }
}

/// Background loop: receives events from the broadcast channel and updates `inner`.
async fn run_event_loop(inner: Arc<RwLock<StatsInner>>, mut rx: tokio::sync::broadcast::Receiver<Arc<DownloadEvent>>) {
    loop {
        match rx.recv().await {
            Ok(event) => {
                let mut state = inner.write().await;
                handle_event(&mut state, &event);
            }
            Err(RecvError::Lagged(missed)) => {
                tracing::warn!(missed = missed, "Statistics tracker lagged, some events were missed");
            }
            Err(RecvError::Closed) => break,
        }
    }

    tracing::debug!("📊 Statistics tracker event loop terminated");
}

/// Resolved fields extracted from an in-progress download record.
///
/// Produced by [`resolve_in_progress_record`] to avoid repeating the same
/// `match record { Some(r) => ..., None => ... }` pattern in every terminal arm.
struct ResolvedRecord {
    url: String,
    priority: DownloadPriority,
    queue_wait: Option<Duration>,
    peak_speed: f64,
    elapsed: Option<Duration>,
}

/// Extracts timing and identity fields from an optional in-progress record.
///
/// Returns a fully-populated [`ResolvedRecord`] with zero-valued defaults when
/// `record` is `None` (i.e. the download was not tracked — e.g. it was queued
/// before the tracker started).
///
/// # Arguments
///
/// * `record` - The removed in-progress entry, or `None` if not found.
///
/// # Returns
///
/// A [`ResolvedRecord`] with `url`, `priority`, `queue_wait`, `peak_speed`,
/// and `elapsed` populated from the record, or defaulted to empty/`None`/`0.0`.
fn resolve_in_progress_record(record: Option<InProgressDownload>) -> ResolvedRecord {
    match record {
        Some(r) => ResolvedRecord {
            queue_wait: r.started_at.map(|s| s.duration_since(r.queued_at)),
            elapsed: r.started_at.map(|s| s.elapsed()),
            peak_speed: r.peak_speed,
            url: r.url,
            priority: r.priority,
        },
        None => ResolvedRecord {
            url: String::new(),
            priority: DownloadPriority::Normal,
            queue_wait: None,
            peak_speed: 0.0,
            elapsed: None,
        },
    }
}

/// Applies a single event to the mutable state. No I/O or `.await` inside.
fn handle_event(state: &mut StatsInner, event: &DownloadEvent) {
    match event {
        DownloadEvent::DownloadQueued {
            download_id,
            url,
            priority,
            ..
        } => {
            state.attempted += 1;
            state.queued += 1;
            state.in_progress.insert(
                *download_id,
                InProgressDownload {
                    url: url.clone(),
                    priority: *priority,
                    queued_at: Instant::now(),
                    started_at: None,
                    peak_speed: 0.0,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                },
            );

            tracing::debug!(download_id = download_id, url = url, "📊 Download queued");
        }

        DownloadEvent::DownloadStarted {
            download_id,
            total_bytes,
            ..
        } => {
            state.queued = state.queued.saturating_sub(1);

            if let Some(entry) = state.in_progress.get_mut(download_id) {
                entry.started_at = Some(Instant::now());
                entry.total_bytes = *total_bytes;
            }

            tracing::debug!(
                download_id = download_id,
                total_bytes = total_bytes,
                "📊 Download started"
            );
        }

        DownloadEvent::DownloadProgress {
            download_id,
            downloaded_bytes,
            total_bytes,
            speed_bytes_per_sec,
            ..
        } => {
            if let Some(entry) = state.in_progress.get_mut(download_id) {
                entry.downloaded_bytes = *downloaded_bytes;
                entry.total_bytes = *total_bytes;
                if *speed_bytes_per_sec > entry.peak_speed {
                    entry.peak_speed = *speed_bytes_per_sec;
                }
            }
        }

        DownloadEvent::DownloadCompleted {
            download_id,
            duration,
            total_bytes,
            ..
        } => {
            state.completed += 1;
            state.total_bytes += total_bytes;
            state.total_download_duration += *duration;

            let rec = resolve_in_progress_record(state.in_progress.remove(download_id));

            state.push_history(CompletedDownload {
                download_id: *download_id,
                url: rec.url,
                priority: rec.priority,
                outcome: DownloadOutcome::Completed,
                bytes: *total_bytes,
                duration: Some(*duration),
                queue_wait: rec.queue_wait,
                peak_speed: rec.peak_speed,
                retry_count: 0,
            });

            tracing::debug!(
                download_id = download_id,
                total_bytes = total_bytes,
                duration = ?duration,
                "📊 Download completed"
            );
        }

        DownloadEvent::DownloadFailed {
            download_id,
            retry_count,
            ..
        } => {
            state.failed += 1;
            state.total_retries += *retry_count as u64;

            let rec = resolve_in_progress_record(state.in_progress.remove(download_id));

            state.push_history(CompletedDownload {
                download_id: *download_id,
                url: rec.url,
                priority: rec.priority,
                outcome: DownloadOutcome::Failed,
                bytes: 0,
                duration: rec.elapsed,
                queue_wait: rec.queue_wait,
                peak_speed: rec.peak_speed,
                retry_count: *retry_count,
            });

            tracing::debug!(
                download_id = download_id,
                retry_count = retry_count,
                "📊 Download failed"
            );
        }

        DownloadEvent::DownloadCanceled { download_id, .. } => {
            state.canceled += 1;
            if state.queued > 0 {
                state.queued -= 1;
            }

            let rec = resolve_in_progress_record(state.in_progress.remove(download_id));

            state.push_history(CompletedDownload {
                download_id: *download_id,
                url: rec.url,
                priority: rec.priority,
                outcome: DownloadOutcome::Canceled,
                bytes: 0,
                duration: None,
                queue_wait: rec.queue_wait,
                peak_speed: 0.0,
                retry_count: 0,
            });

            tracing::debug!(download_id = download_id, "📊 Download canceled");
        }

        DownloadEvent::VideoFetched { url, duration, .. } => {
            state.fetch_attempted += 1;
            state.fetch_succeeded += 1;
            state.total_fetch_duration += *duration;

            tracing::debug!(
                url = url,
                duration = ?duration,
                "📊 Video fetched"
            );
        }

        DownloadEvent::VideoFetchFailed { url, duration, .. } => {
            state.fetch_attempted += 1;
            state.fetch_failed += 1;

            tracing::debug!(
                url = url,
                duration = ?duration,
                "📊 Video fetch failed"
            );
        }

        DownloadEvent::PlaylistFetched {
            url,
            duration,
            playlist,
        } => {
            state.fetch_attempted += 1;
            state.fetch_succeeded += 1;
            state.total_fetch_duration += *duration;
            state.playlists_fetched += 1;

            tracing::debug!(
                url = url,
                duration = ?duration,
                playlist_id = %playlist.id,
                "📊 Playlist fetched"
            );
        }

        DownloadEvent::PlaylistFetchFailed { url, duration, .. } => {
            state.fetch_attempted += 1;
            state.fetch_failed += 1;
            state.playlist_fetch_failed += 1;

            tracing::debug!(
                url = url,
                duration = ?duration,
                "📊 Playlist fetch failed"
            );
        }

        DownloadEvent::PlaylistCompleted { successful, failed, .. } => {
            state.playlist_items_successful += *successful as u64;
            state.playlist_items_failed += *failed as u64;

            tracing::debug!(successful = successful, failed = failed, "📊 Playlist completed");
        }

        DownloadEvent::PostProcessStarted { operation, .. } => {
            state.postprocess_attempted += 1;

            tracing::debug!(
                operation = ?operation,
                "📊 Post-process started"
            );
        }

        DownloadEvent::PostProcessCompleted {
            operation, duration, ..
        } => {
            state.postprocess_succeeded += 1;
            state.total_postprocess_duration += *duration;

            tracing::debug!(
                operation = ?operation,
                duration = ?duration,
                "📊 Post-process completed"
            );
        }

        DownloadEvent::PostProcessFailed { operation, error, .. } => {
            state.postprocess_failed += 1;

            tracing::debug!(
                operation = ?operation,
                error = error,
                "📊 Post-process failed"
            );
        }

        DownloadEvent::SegmentStarted { .. }
        | DownloadEvent::SegmentCompleted { .. }
        | DownloadEvent::FormatSelected { .. }
        | DownloadEvent::MetadataApplied { .. }
        | DownloadEvent::ChaptersEmbedded { .. }
        | DownloadEvent::DownloadPaused { .. }
        | DownloadEvent::DownloadResumed { .. }
        | DownloadEvent::PlaylistItemStarted { .. }
        | DownloadEvent::PlaylistItemCompleted { .. }
        | DownloadEvent::PlaylistItemFailed { .. } => {
            tracing::debug!(event = ?event, "📊 Untracked event, ignoring");
        }

        #[cfg(feature = "live-recording")]
        DownloadEvent::LiveRecordingStarted { .. }
        | DownloadEvent::LiveRecordingProgress { .. }
        | DownloadEvent::LiveRecordingStopped { .. }
        | DownloadEvent::LiveRecordingFailed { .. } => {
            tracing::debug!(event = ?event, "📊 Live recording event, ignoring in stats");
        }
        #[cfg(feature = "live-streaming")]
        DownloadEvent::LiveStreamStarted { .. }
        | DownloadEvent::LiveStreamProgress { .. }
        | DownloadEvent::LiveStreamStopped { .. }
        | DownloadEvent::LiveStreamFailed { .. } => {
            tracing::debug!(event = ?event, "📊 Live stream event, ignoring in stats");
        }
    }
}

/// Constructs a [`GlobalSnapshot`] from the current state. Called under a read lock.
fn build_snapshot(state: &StatsInner) -> GlobalSnapshot {
    let now = Instant::now();
    let terminal = state.completed + state.failed + state.canceled;
    let download_success_rate = if terminal > 0 {
        Some(state.completed as f64 / terminal as f64)
    } else {
        None
    };

    let fetch_success_rate = if state.fetch_attempted > 0 {
        Some(state.fetch_succeeded as f64 / state.fetch_attempted as f64)
    } else {
        None
    };

    let postprocess_success_rate = if state.postprocess_attempted > 0 {
        Some(state.postprocess_succeeded as f64 / state.postprocess_attempted as f64)
    } else {
        None
    };

    let postprocess_avg_duration = if state.postprocess_succeeded > 0 {
        Some(Duration::from_secs_f64(
            state.total_postprocess_duration.as_secs_f64() / state.postprocess_succeeded as f64,
        ))
    } else {
        None
    };

    let playlist_items_total = state.playlist_items_successful + state.playlist_items_failed;
    let item_success_rate = if playlist_items_total > 0 {
        Some(state.playlist_items_successful as f64 / playlist_items_total as f64)
    } else {
        None
    };

    let mut active_downloads: Vec<ActiveDownloadSnapshot> = state
        .in_progress
        .iter()
        .map(|(id, r)| {
            let progress = if r.total_bytes > 0 {
                Some(r.downloaded_bytes as f64 / r.total_bytes as f64)
            } else {
                None
            };
            ActiveDownloadSnapshot {
                download_id: *id,
                url: r.url.clone(),
                priority: r.priority,
                downloaded_bytes: r.downloaded_bytes,
                total_bytes: r.total_bytes,
                progress,
                peak_speed_bytes_per_sec: r.peak_speed,
                elapsed: r.started_at.map(|s| now.duration_since(s)),
                time_since_queued: now.duration_since(r.queued_at),
            }
        })
        .collect();
    active_downloads.sort_by_key(|e| e.download_id);

    let recent_downloads: Vec<DownloadSnapshot> = state
        .history
        .iter()
        .map(|r| DownloadSnapshot {
            download_id: r.download_id,
            url: r.url.clone(),
            priority: r.priority,
            outcome: match r.outcome {
                DownloadOutcome::Completed => DownloadOutcomeSnapshot::Completed,
                DownloadOutcome::Failed => DownloadOutcomeSnapshot::Failed,
                DownloadOutcome::Canceled => DownloadOutcomeSnapshot::Canceled,
            },
            bytes: r.bytes,
            duration: r.duration,
            queue_wait: r.queue_wait,
            peak_speed_bytes_per_sec: r.peak_speed,
            retry_count: r.retry_count,
        })
        .collect();

    GlobalSnapshot {
        downloads: DownloadStats {
            attempted: state.attempted,
            completed: state.completed,
            failed: state.failed,
            canceled: state.canceled,
            queued: state.queued,
            total_bytes: state.total_bytes,
            total_retries: state.total_retries,
            total_duration: state.total_download_duration,
            avg_duration: state.avg_download_duration(),
            avg_speed_bytes_per_sec: state.avg_speed_bytes_per_sec(),
            peak_speed_bytes_per_sec: state.peak_speed_bytes_per_sec(),
            success_rate: download_success_rate,
        },
        fetches: FetchStats {
            attempted: state.fetch_attempted,
            succeeded: state.fetch_succeeded,
            failed: state.fetch_failed,
            avg_duration: state.avg_fetch_duration(),
            success_rate: fetch_success_rate,
        },
        post_processing: PostProcessStats {
            attempted: state.postprocess_attempted,
            succeeded: state.postprocess_succeeded,
            failed: state.postprocess_failed,
            avg_duration: postprocess_avg_duration,
            success_rate: postprocess_success_rate,
        },
        playlists: PlaylistStats {
            playlists_fetched: state.playlists_fetched,
            playlists_fetch_failed: state.playlist_fetch_failed,
            items_successful: state.playlist_items_successful,
            items_failed: state.playlist_items_failed,
            item_success_rate,
        },
        active_count: active_downloads.len(),
        active_downloads,
        recent_downloads,
    }
}
