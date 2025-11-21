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
    /// * `capacity` - Maximum number of events that can be buffered. If the buffer
    ///   is full and subscribers are slow, the oldest events will be dropped.
    ///   A capacity of 1024 is reasonable for most use cases.
    ///
    /// # Returns
    /// A new EventBus instance
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Creates a new EventBus with default capacity (1024 events)
    pub fn with_default_capacity() -> Self {
        Self::new(1024)
    }

    /// Emits an event to all subscribers
    ///
    /// Events are wrapped in Arc for efficient cloning across subscribers.
    /// If no subscribers are listening, the event is silently dropped.
    ///
    /// # Arguments
    /// * `event` - The event to emit
    ///
    /// # Errors
    /// Returns the number of active receivers. If 0, no one is listening.
    pub fn emit(&self, event: DownloadEvent) -> usize {
        let event = Arc::new(event);
        // send returns Err if there are no receivers, which is fine
        self.tx.send(event).unwrap_or(0)
    }

    /// Emits an event only if there are active subscribers
    ///
    /// This is more efficient than `emit()` if you want to avoid creating
    /// the event when no one is listening.
    ///
    /// # Arguments
    /// * `event` - The event to emit
    ///
    /// # Returns
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
    /// A broadcast receiver that can be used to receive events
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<DownloadEvent>> {
        self.tx.subscribe()
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
    /// A Stream that yields events
    pub fn stream(
        &self,
    ) -> impl Stream<
        Item = Result<Arc<DownloadEvent>, tokio_stream::wrappers::errors::BroadcastStreamRecvError>,
    > {
        BroadcastStream::new(self.subscribe())
    }

    /// Returns the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Checks if there are any active subscribers
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_event_bus_basic() {
        let bus = EventBus::new(10);

        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let event = DownloadEvent::DownloadQueued {
            download_id: 1,
            url: "test".to_string(),
            priority: crate::download::DownloadPriority::Normal,
            output_path: "/tmp/test.mp4".into(),
        };

        let count = bus.emit(event.clone());
        assert_eq!(count, 2);

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert!(matches!(*received1, DownloadEvent::DownloadQueued { .. }));
        assert!(matches!(*received2, DownloadEvent::DownloadQueued { .. }));
    }

    #[tokio::test]
    async fn test_event_stream() {
        let bus = EventBus::new(10);
        let mut stream = bus.stream();

        bus.emit(DownloadEvent::DownloadStarted {
            download_id: 1,
            url: "test".to_string(),
            total_bytes: 1000,
            format_id: None,
        });

        let event = stream.next().await.unwrap().unwrap();
        assert!(matches!(*event, DownloadEvent::DownloadStarted { .. }));
    }

    #[test]
    fn test_subscriber_count() {
        let bus = EventBus::new(10);
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);

        let _rx2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_emit_if_subscribed() {
        let bus = EventBus::new(10);

        let event = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: "/tmp/test.mp4".into(),
            duration: std::time::Duration::from_secs(10),
            total_bytes: 1000,
        };

        // No subscribers
        assert!(!bus.emit_if_subscribed(event.clone()));

        // With subscriber
        let _rx = bus.subscribe();
        assert!(bus.emit_if_subscribed(event));
    }
}
