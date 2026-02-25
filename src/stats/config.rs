/// Configuration for the [`super::StatisticsTracker`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackerConfig {
    /// Maximum number of completed download records retained in history.
    /// Oldest records are evicted when this limit is reached. Default: 1000.
    pub max_download_history: usize,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            max_download_history: 1000,
        }
    }
}

impl std::fmt::Display for TrackerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrackerConfig(max_history={})", self.max_download_history)
    }
}
