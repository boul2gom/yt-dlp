use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::config::TrackerConfig;
use crate::download::DownloadPriority;

/// Outcome of a completed download.
#[derive(Debug, Clone)]
pub(super) enum DownloadOutcome {
    Completed,
    Failed,
    Canceled,
}

/// State for a download that is still in progress.
#[derive(Debug)]
pub(super) struct InProgressDownload {
    pub url: String,
    pub priority: DownloadPriority,
    pub queued_at: Instant,
    pub started_at: Option<Instant>,
    pub peak_speed: f64,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

/// Record of a completed (terminal) download kept in history.
#[derive(Debug, Clone)]
pub(super) struct CompletedDownload {
    pub download_id: u64,
    pub url: String,
    pub priority: DownloadPriority,
    pub outcome: DownloadOutcome,
    pub bytes: u64,
    pub duration: Option<Duration>,
    pub queue_wait: Option<Duration>,
    pub peak_speed: f64,
    pub retry_count: u32,
}

/// All mutable state maintained by the tracker, held behind an `RwLock`.
#[derive(Debug)]
pub(super) struct StatsInner {
    pub config: TrackerConfig,

    // download counters
    pub attempted: u64,
    pub completed: u64,
    pub failed: u64,
    pub canceled: u64,
    pub queued: u64,
    pub total_bytes: u64,
    pub total_retries: u64,
    pub total_download_duration: Duration,

    // per-download live state
    pub in_progress: HashMap<u64, InProgressDownload>,
    pub history: VecDeque<CompletedDownload>,

    // fetch counters
    pub fetch_attempted: u64,
    pub fetch_succeeded: u64,
    pub fetch_failed: u64,
    pub total_fetch_duration: Duration,

    // post-processing counters
    pub postprocess_attempted: u64,
    pub postprocess_succeeded: u64,
    pub postprocess_failed: u64,
    pub total_postprocess_duration: Duration,

    // playlist counters
    pub playlists_fetched: u64,
    pub playlist_fetch_failed: u64,
    pub playlist_items_successful: u64,
    pub playlist_items_failed: u64,
}

impl StatsInner {
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            config,
            attempted: 0,
            completed: 0,
            failed: 0,
            canceled: 0,
            queued: 0,
            total_bytes: 0,
            total_retries: 0,
            total_download_duration: Duration::ZERO,
            in_progress: HashMap::new(),
            history: VecDeque::new(),
            fetch_attempted: 0,
            fetch_succeeded: 0,
            fetch_failed: 0,
            total_fetch_duration: Duration::ZERO,
            postprocess_attempted: 0,
            postprocess_succeeded: 0,
            postprocess_failed: 0,
            total_postprocess_duration: Duration::ZERO,
            playlists_fetched: 0,
            playlist_fetch_failed: 0,
            playlist_items_successful: 0,
            playlist_items_failed: 0,
        }
    }

    /// Append a completed download to history, evicting the oldest entry if at capacity.
    pub fn push_history(&mut self, record: CompletedDownload) {
        if self.history.len() >= self.config.max_download_history {
            self.history.pop_front();
        }
        self.history.push_back(record);
    }

    /// Average duration across all completed downloads, or `None` if no downloads finished.
    pub fn avg_download_duration(&self) -> Option<Duration> {
        if self.completed == 0 {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.total_download_duration.as_secs_f64() / self.completed as f64,
            ))
        }
    }

    /// Average duration across all successful fetches, or `None` if none succeeded.
    pub fn avg_fetch_duration(&self) -> Option<Duration> {
        if self.fetch_succeeded == 0 {
            None
        } else {
            Some(Duration::from_secs_f64(
                self.total_fetch_duration.as_secs_f64() / self.fetch_succeeded as f64,
            ))
        }
    }

    /// Average download speed in bytes per second across all completed downloads.
    pub fn avg_speed_bytes_per_sec(&self) -> Option<f64> {
        let secs = self.total_download_duration.as_secs_f64();
        if secs == 0.0 || self.total_bytes == 0 {
            None
        } else {
            Some(self.total_bytes as f64 / secs)
        }
    }

    /// Peak speed observed across all download history records.
    pub fn peak_speed_bytes_per_sec(&self) -> f64 {
        self.history.iter().map(|r| r.peak_speed).fold(0.0_f64, f64::max)
    }
}
