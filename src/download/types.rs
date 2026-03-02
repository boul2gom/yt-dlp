//! Data types and configuration for the download manager.
//!
//! Contains the priority enum, task struct, configuration, progress tracking,
//! and status types used by the download queue.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use typed_builder::TypedBuilder;

use crate::client::proxy::ProxyConfig;
use crate::download::config::speed_profile::SpeedProfile;

/// Shared progress callback: receives `(downloaded_bytes, total_bytes)` on each chunk.
pub type ProgressCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// Per-task byte counters used by the progress callback (downloaded, total).
pub(crate) type ProgressCounters = Arc<std::sync::Mutex<HashMap<u64, (Arc<AtomicU64>, Arc<AtomicU64>)>>>;

/// Number of download attempts in case of failure.
pub(super) const DEFAULT_RETRY_ATTEMPTS: usize = 3;
/// Threshold for automatic cleanup of finished downloads.
pub(super) const DEFAULT_CLEANUP_THRESHOLD: usize = 1000;

/// Download priority
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DownloadPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    #[default]
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

impl std::fmt::Display for DownloadPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => f.write_str("Low"),
            Self::Normal => f.write_str("Normal"),
            Self::High => f.write_str("High"),
            Self::Critical => f.write_str("Critical"),
        }
    }
}

/// Download task
pub(crate) struct DownloadTask {
    /// URL to download
    pub(crate) url: String,
    /// Destination path
    pub(crate) destination: PathBuf,
    /// Download priority
    pub(crate) priority: DownloadPriority,
    /// Unique ID of the task
    pub(crate) id: u64,
    /// Progress callback
    pub(crate) progress_callback: Option<ProgressCallback>,
    /// Optional HTTP headers from yt-dlp to use for the download
    pub(crate) http_headers: Option<crate::model::format::HttpHeaders>,
    /// Optional byte sub-range to download: only `[start, end]` bytes are fetched and
    /// written from offset 0 in the destination file.
    pub(crate) range_constraint: Option<(u64, u64)>,
}

impl std::fmt::Debug for DownloadTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadTask")
            .field("url", &self.url)
            .field("destination", &self.destination)
            .field("priority", &self.priority)
            .field("id", &self.id)
            .field("range_constraint", &self.range_constraint)
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

        // Then by ID (smaller ID = older = more prioritary in a max-heap)
        other.id.cmp(&self.id)
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

impl ManagerConfig {
    /// Creates a configuration from a speed profile with appropriate defaults.
    ///
    /// # Arguments
    ///
    /// * `profile` - The speed profile to base configuration on
    ///
    /// # Returns
    ///
    /// A `ManagerConfig` with settings derived from the speed profile.
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

    /// Sets the speed profile and updates related settings.
    pub fn with_speed_profile(mut self, profile: SpeedProfile) -> Self {
        self.max_concurrent_downloads = profile.max_concurrent_downloads();
        self.segment_size = profile.segment_size();
        self.parallel_segments = profile.parallel_segments();
        self.max_buffer_size = profile.max_buffer_size();
        self.speed_profile = profile;
        self
    }
}

impl Default for ManagerConfig {
    fn default() -> Self {
        let profile = SpeedProfile::default();
        Self::from_speed_profile(profile)
    }
}

impl std::fmt::Display for ManagerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ManagerConfig(profile={}, concurrent={}, segments={}x{}, retries={})",
            self.speed_profile,
            self.max_concurrent_downloads,
            self.parallel_segments,
            self.segment_size,
            self.retry_attempts
        )
    }
}

/// Progress update for a download
#[derive(Clone)]
pub struct ProgressUpdate {
    /// Download ID
    pub download_id: u64,
    /// Downloaded bytes
    pub downloaded_bytes: u64,
    /// Total bytes
    pub total_bytes: u64,
}

impl std::fmt::Display for ProgressUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProgressUpdate(id={}, downloaded={}, total={})",
            self.download_id, self.downloaded_bytes, self.total_bytes
        )
    }
}

/// Status of a download
#[derive(Debug, Clone)]
pub enum DownloadStatus {
    /// Download is pending in queue
    Queued,
    /// Download is in progress
    Downloading {
        /// Downloaded bytes
        downloaded_bytes: u64,
        /// Total bytes (0 if unknown)
        total_bytes: u64,
    },
    /// Download was successful
    Completed,
    /// Download failed
    Failed {
        /// Error message
        reason: String,
    },
    /// Download was cancelled
    Canceled,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => f.write_str("Queued"),
            Self::Downloading {
                downloaded_bytes,
                total_bytes,
            } => write!(f, "Downloading({}/{})", downloaded_bytes, total_bytes),
            Self::Completed => f.write_str("Completed"),
            Self::Failed { reason } => write!(f, "Failed(reason={})", reason),
            Self::Canceled => f.write_str("Canceled"),
        }
    }
}
