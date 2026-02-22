/// Configuration for the [`super::StatisticsTracker`].
#[derive(Debug, Clone)]
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
