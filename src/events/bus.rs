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
        #[cfg(feature = "tracing")]
        tracing::debug!(capacity = capacity, "Creating new EventBus");

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
    /// Events are wrapped in Arc for efficient cloning across subscribers.
    /// If no subscribers are listening, the event is silently dropped.
    ///
    /// # Arguments
    /// 
    /// * `event` - The event to emit
    ///
    /// # Returns
    /// 
    /// The number of active receivers that received the event. If 0, no one is listening.
    pub fn emit(&self, event: DownloadEvent) -> usize {
        let event_type = event.event_type();
        let download_id = event.download_id();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            event_type = event_type,
            download_id = download_id,
            subscriber_count = self.subscriber_count(),
            "Emitting event"
        );

        let event = Arc::new(event);
        // send returns Err if there are no receivers, which is fine
        let receiver_count = self.tx.send(event).unwrap_or(0);

        #[cfg(feature = "tracing")]
        tracing::debug!(
            event_type = event_type,
            download_id = download_id,
            receivers_notified = receiver_count,
            "Event emitted"
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

    /// Creates a new subscriber that will receive all future events
    ///
    /// # Returns
    ///
    /// A broadcast receiver that can be used to receive events
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<DownloadEvent>> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            subscriber_count_before = self.subscriber_count(),
            "Creating new subscriber"
        );

        let receiver = self.tx.subscribe();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            subscriber_count_after = self.subscriber_count(),
            "Subscriber created"
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
    ) -> impl Stream<
        Item = Result<Arc<DownloadEvent>, tokio_stream::wrappers::errors::BroadcastStreamRecvError>,
    > {
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
