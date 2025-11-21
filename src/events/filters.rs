use super::types::DownloadEvent;
use std::sync::Arc;

/// Type alias for event filter predicates
type FilterPredicate = Arc<dyn Fn(&DownloadEvent) -> bool + Send + Sync>;

/// Filter for selecting which events to process
///
/// Filters can be combined using builder pattern methods
#[derive(Clone)]
pub struct EventFilter {
    predicates: Vec<FilterPredicate>,
}

impl EventFilter {
    /// Creates a new filter that accepts all events
    pub fn all() -> Self {
        Self {
            predicates: Vec::new(),
        }
    }

    /// Creates a filter that only accepts events with the specified download ID
    pub fn download_id(id: u64) -> Self {
        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(move |event| event.download_id() == Some(id)));
        filter
    }

    /// Creates a filter that only accepts terminal events (completed, failed, canceled)
    pub fn only_terminal() -> Self {
        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(|event| event.is_terminal()));
        filter
    }

    /// Creates a filter that only accepts completed downloads
    pub fn only_completed() -> Self {
        let mut filter = Self::all();
        filter.predicates.push(Arc::new(|event| {
            matches!(event, DownloadEvent::DownloadCompleted { .. })
        }));
        filter
    }

    /// Creates a filter that only accepts failed downloads
    pub fn only_failed() -> Self {
        let mut filter = Self::all();
        filter.predicates.push(Arc::new(|event| {
            matches!(event, DownloadEvent::DownloadFailed { .. })
        }));
        filter
    }

    /// Creates a filter that only accepts progress events
    pub fn only_progress() -> Self {
        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(|event| event.is_progress()));
        filter
    }

    /// Creates a filter for specific event types
    pub fn event_types(types: Vec<&'static str>) -> Self {
        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(move |event| types.contains(&event.event_type())));
        filter
    }

    /// Adds a custom predicate to the filter
    pub fn and_then<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&DownloadEvent) -> bool + Send + Sync + 'static,
    {
        self.predicates.push(Arc::new(predicate));
        self
    }

    /// Tests if an event matches all predicates
    pub fn matches(&self, event: &DownloadEvent) -> bool {
        self.predicates.iter().all(|predicate| predicate(event))
    }

    /// Excludes progress events (useful to reduce noise)
    pub fn exclude_progress(self) -> Self {
        self.and_then(|event| !event.is_progress())
    }

    /// Only includes events for downloads (excludes metadata, playlists, etc.)
    pub fn only_downloads(self) -> Self {
        self.and_then(|event| event.download_id().is_some())
    }

    /// Creates a filter that accepts events matching any of the given event types
    pub fn any_of(types: &[&'static str]) -> Self {
        let types_vec: Vec<&'static str> = types.to_vec();
        Self::all().and_then(move |event| types_vec.contains(&event.event_type()))
    }

    /// Creates a filter for playlist-related events
    pub fn only_playlist() -> Self {
        Self::any_of(&[
            "playlist_fetched",
            "playlist_item_started",
            "playlist_item_completed",
            "playlist_item_failed",
            "playlist_completed",
        ])
    }

    /// Creates a filter for metadata-related events
    pub fn only_metadata() -> Self {
        Self::any_of(&["metadata_applied", "chapters_embedded"])
    }

    /// Creates a filter for post-processing events
    pub fn only_post_process() -> Self {
        Self::any_of(&[
            "post_process_started",
            "post_process_completed",
            "post_process_failed",
        ])
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::all()
    }
}

impl std::fmt::Debug for EventFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventFilter")
            .field("predicate_count", &self.predicates.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_filter_all() {
        let filter = EventFilter::all();
        let event = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: PathBuf::from("/tmp/test.mp4"),
            duration: Duration::from_secs(10),
            total_bytes: 1000,
        };
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_filter_download_id() {
        let filter = EventFilter::download_id(1);

        let event1 = DownloadEvent::DownloadStarted {
            download_id: 1,
            url: "test".to_string(),
            total_bytes: 1000,
            format_id: None,
        };

        let event2 = DownloadEvent::DownloadStarted {
            download_id: 2,
            url: "test".to_string(),
            total_bytes: 1000,
            format_id: None,
        };

        assert!(filter.matches(&event1));
        assert!(!filter.matches(&event2));
    }

    #[test]
    fn test_filter_terminal() {
        let filter = EventFilter::only_terminal();

        let completed = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: PathBuf::from("/tmp/test.mp4"),
            duration: Duration::from_secs(10),
            total_bytes: 1000,
        };

        let progress = DownloadEvent::DownloadProgress {
            download_id: 1,
            downloaded_bytes: 500,
            total_bytes: 1000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: Some(5),
        };

        assert!(filter.matches(&completed));
        assert!(!filter.matches(&progress));
    }

    #[test]
    fn test_filter_and_then() {
        let filter = EventFilter::download_id(1).and_then(|event| event.is_terminal());

        let completed = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: PathBuf::from("/tmp/test.mp4"),
            duration: Duration::from_secs(10),
            total_bytes: 1000,
        };

        let progress = DownloadEvent::DownloadProgress {
            download_id: 1,
            downloaded_bytes: 500,
            total_bytes: 1000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: Some(5),
        };

        assert!(filter.matches(&completed));
        assert!(!filter.matches(&progress));
    }

    #[test]
    fn test_filter_exclude_progress() {
        let filter = EventFilter::all().exclude_progress();

        let completed = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: PathBuf::from("/tmp/test.mp4"),
            duration: Duration::from_secs(10),
            total_bytes: 1000,
        };

        let progress = DownloadEvent::DownloadProgress {
            download_id: 1,
            downloaded_bytes: 500,
            total_bytes: 1000,
            speed_bytes_per_sec: 100.0,
            eta_seconds: Some(5),
        };

        assert!(filter.matches(&completed));
        assert!(!filter.matches(&progress));
    }
}
