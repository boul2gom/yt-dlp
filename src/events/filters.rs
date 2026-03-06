use std::sync::Arc;

use super::types::DownloadEvent;

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
    ///
    /// # Returns
    ///
    /// An EventFilter that matches all events
    pub fn all() -> Self {
        tracing::debug!("⚙️ Creating EventFilter that accepts all events");

        Self { predicates: Vec::new() }
    }

    /// Creates a filter that only accepts events with the specified download ID
    ///
    /// # Arguments
    ///
    /// * `id` - The download ID to filter by
    ///
    /// # Returns
    ///
    /// An EventFilter that only matches events with the given download ID
    pub fn download_id(id: u64) -> Self {
        tracing::debug!(download_id = id, "⚙️ Creating EventFilter for download ID");

        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(move |event| event.download_id() == Some(id)));
        filter
    }

    /// Creates a filter that only accepts terminal events (completed, failed, canceled)
    ///
    /// # Returns
    ///
    /// An EventFilter that only matches terminal events
    pub fn only_terminal() -> Self {
        tracing::debug!("⚙️ Creating EventFilter for terminal events");

        let mut filter = Self::all();
        filter.predicates.push(Arc::new(|event| event.is_terminal()));
        filter
    }

    /// Creates a filter that only accepts completed downloads
    ///
    /// # Returns
    ///
    /// An EventFilter that only matches completed download events
    pub fn only_completed() -> Self {
        tracing::debug!("⚙️ Creating EventFilter for completed downloads");

        let mut filter = Self::all();
        filter.predicates.push(Arc::new(|event| {
            matches!(event, DownloadEvent::DownloadCompleted { .. })
        }));
        filter
    }

    /// Creates a filter that only accepts failed downloads
    ///
    /// # Returns
    ///
    /// An EventFilter that only matches failed download events
    pub fn only_failed() -> Self {
        tracing::debug!("⚙️ Creating EventFilter for failed downloads");

        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(|event| matches!(event, DownloadEvent::DownloadFailed { .. })));
        filter
    }

    /// Creates a filter that only accepts progress events
    ///
    /// # Returns
    ///
    /// An EventFilter that only matches progress events
    pub fn only_progress() -> Self {
        tracing::debug!("⚙️ Creating EventFilter for progress events");

        let mut filter = Self::all();
        filter.predicates.push(Arc::new(|event| event.is_progress()));
        filter
    }

    /// Creates a filter for specific event types
    pub fn event_types(types: Vec<&'static str>) -> Self {
        tracing::debug!(
            type_count = types.len(),
            "⚙️ Creating EventFilter for specific event types"
        );

        let mut filter = Self::all();
        filter
            .predicates
            .push(Arc::new(move |event| types.contains(&event.event_type())));
        filter
    }

    /// Adds a custom predicate to the filter
    ///
    /// # Arguments
    ///
    /// * `predicate` - A function that returns true if the event should be accepted
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn and_then<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&DownloadEvent) -> bool + Send + Sync + 'static,
    {
        self.predicates.push(Arc::new(predicate));
        self
    }

    /// Tests if an event matches all predicates
    ///
    /// # Arguments
    ///
    /// * `event` - The event to test
    ///
    /// # Returns
    ///
    /// true if the event matches all predicates, false otherwise
    pub fn matches(&self, event: &DownloadEvent) -> bool {
        let result = self.predicates.iter().all(|predicate| predicate(event));

        tracing::trace!(
            event_type = event.event_type(),
            download_id = event.download_id(),
            matches = result,
            predicate_count = self.predicates.len(),
            "🔔 Event filter match test"
        );

        result
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
        Self::any_of(&["post_process_started", "post_process_completed", "post_process_failed"])
    }

    /// Creates a filter for live recording events.
    #[cfg(feature = "live-recording")]
    pub fn only_live_recording() -> Self {
        Self::any_of(&[
            "live_recording_started",
            "live_recording_progress",
            "live_recording_stopped",
            "live_recording_failed",
        ])
    }

    /// Creates a filter for live recording events matching a specific video ID.
    #[cfg(feature = "live-recording")]
    pub fn live_recording(video_id: impl Into<String>) -> Self {
        let id = video_id.into();
        Self::only_live_recording().and_then(move |event| match event {
            DownloadEvent::LiveRecordingStarted { video_id, .. }
            | DownloadEvent::LiveRecordingProgress { video_id, .. }
            | DownloadEvent::LiveRecordingStopped { video_id, .. }
            | DownloadEvent::LiveRecordingFailed { video_id, .. } => video_id == &id,
            _ => false,
        })
    }

    /// Creates a filter for live streaming events.
    #[cfg(feature = "live-streaming")]
    pub fn only_live_streaming() -> Self {
        Self::any_of(&[
            "live_stream_started",
            "live_stream_progress",
            "live_stream_stopped",
            "live_stream_failed",
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

impl std::fmt::Display for EventFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventFilter(predicates={})", self.predicates.len())
    }
}
