use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;

use super::types::DownloadEvent;

/// Central event bus for distributing download events to listeners
///
/// The EventBus uses broadcast channels to allow multiple subscribers to receive
/// all events. Events are cloned for each subscriber.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Arc<DownloadEvent>>,
}

impl EventBus {
    /// Creates a new EventBus with the specified channel capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of events that can be buffered. If the buffer
    ///   is full and subscribers are slow, the oldest events will be dropped.
    ///   A capacity of 1024 is reasonable for most use cases.
    ///
    /// # Returns
    ///
    /// A new EventBus instance
    pub fn new(capacity: usize) -> Self {
        tracing::debug!(capacity = capacity, "⚙️ Creating new EventBus");

        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Creates a new EventBus with default capacity (1024 events)
    ///
    /// # Returns
    ///
    /// A new EventBus instance with default capacity
    pub fn with_default_capacity() -> Self {
        Self::new(1024)
    }

    /// Emits an event to all subscribers
    ///
    /// Accepts either a bare `DownloadEvent` (wrapped in `Arc` internally) or an existing
    /// `Arc<DownloadEvent>` to avoid a redundant allocation when the caller already holds one.
    /// If no subscribers are listening, the event is silently dropped.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to emit (any type that converts into `Arc<DownloadEvent>`)
    ///
    /// # Returns
    ///
    /// The number of active receivers that received the event. If 0, no one is listening.
    pub fn emit(&self, event: impl Into<Arc<DownloadEvent>>) -> usize {
        let event = event.into();
        let event_type = event.event_type();
        let download_id = event.download_id();

        tracing::debug!(
            event_type = event_type,
            download_id = download_id,
            subscriber_count = self.subscriber_count(),
            "🔔 Emitting event"
        );

        // send returns Err if there are no receivers, which is fine
        let receiver_count = self.tx.send(event).unwrap_or(0);

        tracing::debug!(
            event_type = event_type,
            download_id = download_id,
            receivers_notified = receiver_count,
            "✅ Event emitted"
        );

        receiver_count
    }

    /// Emits an event only if there are active subscribers
    ///
    /// This is more efficient than `emit()` if you want to avoid creating
    /// the event when no one is listening.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to emit
    ///
    /// # Returns
    ///
    /// true if the event was sent to at least one subscriber
    pub fn emit_if_subscribed(&self, event: DownloadEvent) -> bool {
        if self.tx.receiver_count() > 0 {
            self.emit(event) > 0
        } else {
            false
        }
    }

    /// Creates a new subscriber that will receive all future events.
    ///
    /// # Lag behaviour
    ///
    /// The underlying channel has a fixed capacity (set at construction time, default 1024).
    /// If this receiver falls behind and the buffer fills up, the *oldest* buffered events
    /// are silently dropped. The next `recv()` call will return
    /// [`RecvError::Lagged(n)`](tokio::sync::broadcast::error::RecvError::Lagged) where `n`
    /// is the number of missed events. Callers that need reliable delivery should either
    /// consume events promptly or use a dedicated, rate-limited downstream queue.
    ///
    /// # Returns
    ///
    /// A broadcast receiver that can be used to receive events
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<DownloadEvent>> {
        tracing::debug!(
            subscriber_count_before = self.subscriber_count(),
            "🔔 Creating new subscriber"
        );

        let receiver = self.tx.subscribe();

        tracing::debug!(
            subscriber_count_after = self.subscriber_count(),
            "✅ Subscriber created"
        );

        receiver
    }

    /// Creates a stream of events for async iteration
    ///
    /// # Example
    /// ```ignore
    /// let mut stream = event_bus.stream();
    /// while let Some(Ok(event)) = stream.next().await {
    ///     println!("Event: {:?}", event);
    /// }
    /// ```
    ///
    /// # Returns
    ///
    /// A Stream that yields events
    pub fn stream(
        &self,
    ) -> impl Stream<Item = Result<Arc<DownloadEvent>, tokio_stream::wrappers::errors::BroadcastStreamRecvError>> {
        BroadcastStream::new(self.subscribe())
    }

    /// Returns the number of active subscribers
    ///
    /// # Returns
    ///
    /// The current number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Checks if there are any active subscribers
    ///
    /// # Returns
    ///
    /// true if there is at least one active subscriber, false otherwise
    pub fn has_subscribers(&self) -> bool {
        self.tx.receiver_count() > 0
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("subscriber_count", &self.subscriber_count())
            .finish()
    }
}

impl std::fmt::Display for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventBus(subscribers={})", self.subscriber_count())
    }
}
