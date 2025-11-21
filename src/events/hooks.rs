use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{DownloadEvent, EventFilter};

/// Result type for hook execution
pub type HookResult = Result<(), HookError>;

/// Errors that can occur during hook execution
#[derive(Debug, thiserror::Error)]
pub enum HookError {
    /// Hook execution failed
    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    /// Hook timed out
    #[error("Hook execution timed out")]
    Timeout,

    /// Custom error
    #[error("{0}")]
    Custom(String),
}

/// Trait for implementing custom event hooks
///
/// Event hooks are called asynchronously when events occur, allowing
/// custom logic to be executed in response to download lifecycle events.
#[async_trait]
pub trait EventHook: Send + Sync {
    /// Called when an event occurs
    ///
    /// # Arguments
    ///
    /// * `event` - The event that occurred
    ///
    /// # Returns
    ///
    /// Ok(()) if the hook executed successfully, or an error if it failed
    async fn on_event(&self, event: &DownloadEvent) -> HookResult;

    /// Returns a filter for which events this hook should receive
    ///
    /// By default, receives all events
    fn filter(&self) -> EventFilter {
        EventFilter::all()
    }

    /// Returns the name of this hook (for debugging/logging)
    fn name(&self) -> &'static str {
        "unnamed_hook"
    }

    /// Whether this hook should be executed in parallel with other hooks
    ///
    /// If false, hooks will execute sequentially
    fn parallel_execution(&self) -> bool {
        true
    }
}

/// Registry for managing event hooks
pub struct HookRegistry {
    hooks: Arc<RwLock<Vec<Box<dyn EventHook>>>>,
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry")
            .field("hooks_count", &"<async>")
            .finish()
    }
}

impl HookRegistry {
    /// Creates a new hook registry
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Registers a new hook
    ///
    /// # Arguments
    ///
    /// * `hook` - The hook to register
    pub async fn register(&self, hook: impl EventHook + 'static) {
        let mut hooks = self.hooks.write().await;
        hooks.push(Box::new(hook));
    }

    /// Executes all registered hooks for an event
    ///
    /// Hooks are executed in parallel by default, unless they specify sequential execution
    ///
    /// # Arguments
    ///
    /// * `event` - The event to process
    pub async fn execute(&self, event: &DownloadEvent) {
        let hooks = self.hooks.read().await;

        // Separate hooks into parallel and sequential
        let mut parallel_hooks = Vec::new();
        let mut sequential_hooks = Vec::new();

        for hook in hooks.iter() {
            if hook.filter().matches(event) {
                if hook.parallel_execution() {
                    parallel_hooks.push(hook);
                } else {
                    sequential_hooks.push(hook);
                }
            }
        }

        // Execute parallel hooks concurrently
        let parallel_futures: Vec<_> = parallel_hooks
            .into_iter()
            .map(|hook| {
                let event = event.clone();
                async move {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        hook.on_event(&event),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("Hook '{}' failed: {}", hook.name(), e);
                        }
                        Err(_) => {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("Hook '{}' timed out", hook.name());
                        }
                    }
                }
            })
            .collect();

        futures_util::future::join_all(parallel_futures).await;

        // Execute sequential hooks one by one
        for hook in sequential_hooks {
            match tokio::time::timeout(std::time::Duration::from_secs(30), hook.on_event(event))
                .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Hook '{}' failed: {}", hook.name(), e);
                }
                Err(_) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Hook '{}' timed out", hook.name());
                }
            }
        }
    }

    /// Returns the number of registered hooks
    pub async fn count(&self) -> usize {
        let hooks = self.hooks.read().await;
        hooks.len()
    }

    /// Clears all registered hooks
    pub async fn clear(&self) {
        let mut hooks = self.hooks.write().await;
        hooks.clear();
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for HookRegistry {
    fn clone(&self) -> Self {
        Self {
            hooks: self.hooks.clone(),
        }
    }
}

/// Helper macro for creating simple hooks from closures
///
/// # Example
///
/// ```ignore
/// use yt_dlp::events::{DownloadEvent, EventFilter, simple_hook};
///
/// let hook = simple_hook!("my_hook", EventFilter::only_completed(), |event| {
///     println!("Download completed: {:?}", event);
///     Ok(())
/// });
/// ```
#[macro_export]
macro_rules! simple_hook {
    ($name:expr, $filter:expr, $closure:expr) => {{
        struct SimpleHook<F>
        where
            F: Fn(&$crate::events::DownloadEvent) -> $crate::events::HookResult + Send + Sync,
        {
            name: &'static str,
            filter: $crate::events::EventFilter,
            closure: F,
        }

        #[$crate::async_trait::async_trait]
        impl<F> $crate::events::EventHook for SimpleHook<F>
        where
            F: Fn(&$crate::events::DownloadEvent) -> $crate::events::HookResult + Send + Sync,
        {
            async fn on_event(
                &self,
                event: &$crate::events::DownloadEvent,
            ) -> $crate::events::HookResult {
                (self.closure)(event)
            }

            fn filter(&self) -> $crate::events::EventFilter {
                self.filter.clone()
            }

            fn name(&self) -> &'static str {
                self.name
            }
        }

        SimpleHook {
            name: $name,
            filter: $filter,
            closure: $closure,
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestHook {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHook for TestHook {
        async fn on_event(&self, _event: &DownloadEvent) -> HookResult {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn name(&self) -> &'static str {
            "test_hook"
        }
    }

    #[tokio::test]
    async fn test_hook_registry() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let hook = TestHook {
            counter: counter.clone(),
        };

        registry.register(hook).await;

        let event = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: "/tmp/test.mp4".into(),
            duration: std::time::Duration::from_secs(10),
            total_bytes: 1000,
        };

        registry.execute(&event).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_hook_filter() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));

        struct FilteredHook {
            counter: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl EventHook for FilteredHook {
            async fn on_event(&self, _event: &DownloadEvent) -> HookResult {
                self.counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn filter(&self) -> EventFilter {
                EventFilter::only_completed()
            }

            fn name(&self) -> &'static str {
                "filtered_hook"
            }
        }

        registry
            .register(FilteredHook {
                counter: counter.clone(),
            })
            .await;

        // This event should be filtered out
        let started_event = DownloadEvent::DownloadStarted {
            download_id: 1,
            url: "test".to_string(),
            total_bytes: 1000,
            format_id: None,
        };

        registry.execute(&started_event).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // This event should trigger the hook
        let completed_event = DownloadEvent::DownloadCompleted {
            download_id: 1,
            output_path: "/tmp/test.mp4".into(),
            duration: std::time::Duration::from_secs(10),
            total_bytes: 1000,
        };

        registry.execute(&completed_event).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
