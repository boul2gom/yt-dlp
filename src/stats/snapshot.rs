use std::time::Duration;

use crate::download::DownloadPriority;

/// Aggregate snapshot of all statistics collected by the [`super::StatisticsTracker`].
///
/// Obtained by calling [`super::StatisticsTracker::snapshot`]. All fields are
/// computed at the moment of the call; subsequent mutations to the tracker are not
/// reflected in an already-obtained snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GlobalSnapshot {
    /// Download-level aggregate statistics.
    pub downloads: DownloadStats,
    /// Metadata fetch (video/playlist) aggregate statistics.
    pub fetches: FetchStats,
    /// Post-processing aggregate statistics.
    pub post_processing: PostProcessStats,
    /// Playlist-level aggregate statistics.
    pub playlists: PlaylistStats,
    /// Number of downloads currently in progress. Equivalent to `active_downloads.len()`.
    pub active_count: usize,
    /// Live state of every download currently in progress, ordered by download ID.
    pub active_downloads: Vec<ActiveDownloadSnapshot>,
    /// Bounded window of the most recently completed downloads.
    pub recent_downloads: Vec<DownloadSnapshot>,
}

/// Aggregate counters and derived metrics for all download operations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadStats {
    /// Total number of downloads that were enqueued.
    pub attempted: u64,
    /// Completed downloads.
    pub completed: u64,
    /// Downloads that ended with an error.
    pub failed: u64,
    /// Downloads that were canceled.
    pub canceled: u64,
    /// Downloads currently waiting in the queue.
    pub queued: u64,
    /// Sum of bytes transferred across all completed downloads.
    pub total_bytes: u64,
    /// Total number of retry attempts across all downloads.
    pub total_retries: u64,
    /// Cumulative wall-clock time spent downloading (completed downloads only).
    pub total_duration: Duration,
    /// Average duration per completed download, or `None` if no downloads finished yet.
    pub avg_duration: Option<Duration>,
    /// Average throughput in bytes per second, or `None` if no data transferred.
    pub avg_speed_bytes_per_sec: Option<f64>,
    /// Highest per-download peak speed observed, in bytes per second.
    pub peak_speed_bytes_per_sec: f64,
    /// Ratio of completed to terminal downloads, or `None` if no terminal downloads.
    pub success_rate: Option<f64>,
}

/// Live state of a single download that is currently in progress.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveDownloadSnapshot {
    /// Internal download identifier.
    pub download_id: u64,
    /// URL being downloaded.
    pub url: String,
    /// Priority at which the download was queued.
    pub priority: DownloadPriority,
    /// Number of bytes received so far.
    pub downloaded_bytes: u64,
    /// Expected total size in bytes. `0` means the size is not yet known.
    pub total_bytes: u64,
    /// Download progress as a fraction in `[0.0, 1.0]`, or `None` if `total_bytes` is 0.
    pub progress: Option<f64>,
    /// Peak speed observed so far during this download, in bytes per second.
    pub peak_speed_bytes_per_sec: f64,
    /// Time elapsed since the download started transferring data.
    /// `None` if the download is still waiting in the queue.
    pub elapsed: Option<Duration>,
    /// Total time elapsed since the download was enqueued (queue wait + transfer time).
    pub time_since_queued: Duration,
}

/// Snapshot of a single completed (terminal) download.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadSnapshot {
    /// Internal download identifier.
    pub download_id: u64,
    /// Original URL that was downloaded.
    pub url: String,
    /// Priority at which the download was queued.
    pub priority: DownloadPriority,
    /// Terminal outcome of this download.
    pub outcome: DownloadOutcomeSnapshot,
    /// Bytes transferred.
    pub bytes: u64,
    /// Wall-clock download duration, or `None` if it was canceled before starting.
    pub duration: Option<Duration>,
    /// Time spent waiting in the queue before the download started.
    pub queue_wait: Option<Duration>,
    /// Peak speed observed during this download, in bytes per second.
    pub peak_speed_bytes_per_sec: f64,
    /// Number of retry attempts for this download.
    pub retry_count: u32,
}

/// Terminal outcome of a single completed download.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, serde::Serialize)]
pub enum DownloadOutcomeSnapshot {
    /// Download finished successfully.
    Completed,
    /// Download ended with an error.
    Failed,
    /// Download was canceled by the user.
    Canceled,
}

/// Aggregate statistics for metadata fetch operations (video and playlist).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FetchStats {
    /// Total number of fetch calls made.
    pub attempted: u64,
    /// Fetches that returned a result.
    pub succeeded: u64,
    /// Fetches that returned an error.
    pub failed: u64,
    /// Average duration of successful fetches, or `None` if none succeeded.
    pub avg_duration: Option<Duration>,
    /// Ratio of successful to total fetches, or `None` if no fetches attempted.
    pub success_rate: Option<f64>,
}

/// Aggregate statistics for post-processing operations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PostProcessStats {
    /// Number of post-processing operations started.
    pub attempted: u64,
    /// Operations that completed successfully.
    pub succeeded: u64,
    /// Operations that failed.
    pub failed: u64,
    /// Average duration of successful operations, or `None` if none succeeded.
    pub avg_duration: Option<Duration>,
    /// Ratio of succeeded to attempted, or `None` if no operations ran.
    pub success_rate: Option<f64>,
}

/// Aggregate statistics for playlist-level operations.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistStats {
    /// Number of playlists whose metadata was successfully fetched.
    pub playlists_fetched: u64,
    /// Number of playlist metadata fetch failures.
    pub playlists_fetch_failed: u64,
    /// Number of individual playlist items that downloaded successfully.
    pub items_successful: u64,
    /// Number of individual playlist items that failed.
    pub items_failed: u64,
    /// Ratio of successful items to total items, or `None` if no items attempted.
    pub item_success_rate: Option<f64>,
}

impl std::fmt::Display for GlobalSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GlobalSnapshot(active={}, downloads={}, fetches={}, playlists={})",
            self.active_count, self.downloads, self.fetches, self.playlists
        )
    }
}

impl std::fmt::Display for DownloadStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DownloadStats(attempted={}, completed={}, failed={})",
            self.attempted, self.completed, self.failed
        )
    }
}

impl std::fmt::Display for ActiveDownloadSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ActiveDownloadSnapshot(id={}, downloaded={}, total={})",
            self.download_id, self.downloaded_bytes, self.total_bytes
        )
    }
}

impl std::fmt::Display for DownloadSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DownloadSnapshot(id={}, outcome={}, bytes={})",
            self.download_id, self.outcome, self.bytes
        )
    }
}

impl std::fmt::Display for DownloadOutcomeSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Completed => f.write_str("Completed"),
            Self::Failed => f.write_str("Failed"),
            Self::Canceled => f.write_str("Canceled"),
        }
    }
}

impl std::fmt::Display for FetchStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FetchStats(attempted={}, succeeded={}, failed={})",
            self.attempted, self.succeeded, self.failed
        )
    }
}

impl std::fmt::Display for PostProcessStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PostProcessStats(attempted={}, succeeded={}, failed={})",
            self.attempted, self.succeeded, self.failed
        )
    }
}

impl std::fmt::Display for PlaylistStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PlaylistStats(fetched={}, items_ok={}, items_failed={})",
            self.playlists_fetched, self.items_successful, self.items_failed
        )
    }
}
