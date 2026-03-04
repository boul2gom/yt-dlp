use std::sync::Arc;

use async_trait::async_trait;
use dyn_clone::DynClone;
use tokio::sync::RwLock;

use crate::events::{DownloadEvent, EventFilter};

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
pub trait EventHook: DynClone + Send + Sync {
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

dyn_clone::clone_trait_object!(EventHook);

/// Registry for managing event hooks
pub struct HookRegistry {
    hooks: Arc<RwLock<Vec<Box<dyn EventHook>>>>,
    timeout: std::time::Duration,
}

impl HookRegistry {
    /// Creates a new hook registry
    ///
    /// # Returns
    ///
    /// A new empty HookRegistry
    pub fn new() -> Self {
        tracing::debug!("⚙️ Creating new HookRegistry");

        Self {
            hooks: Arc::new(RwLock::new(Vec::new())),
            timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Sets the timeout for hook execution.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum duration to wait for a single hook to complete
    ///
    /// # Returns
    ///
    /// `self` with the updated timeout
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Registers a new hook
    ///
    /// # Arguments
    ///
    /// * `hook` - The hook to register
    pub async fn register(&self, hook: impl EventHook + 'static) {
        let hook_name = hook.name();

        tracing::debug!(
            hook_name = hook_name,
            parallel_execution = hook.parallel_execution(),
            "🔔 Registering new hook"
        );

        let mut hooks = self.hooks.write().await;
        hooks.push(Box::new(hook));

        tracing::debug!(hook_name = hook_name, total_hooks = hooks.len(), "✅ Hook registered");
    }

    /// Executes all registered hooks for an event
    ///
    /// Hooks are executed in parallel by default, unless they specify sequential execution
    ///
    /// # Arguments
    ///
    /// * `event` - The event to process
    pub async fn execute(&self, event: &DownloadEvent) {
        tracing::debug!(
            event_type = event.event_type(),
            download_id = event.download_id(),
            "🔔 Executing hooks for event"
        );

        // Clone hooks under the lock then release it to avoid deadlock
        // if any hook calls register()/clear()
        let hooks: Vec<_> = self.hooks.read().await.clone();

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

        let parallel_count = parallel_hooks.len();
        let sequential_count = sequential_hooks.len();

        tracing::debug!(
            event_type = event.event_type(),
            total_hooks = hooks.len(),
            parallel_hooks = parallel_count,
            sequential_hooks = sequential_count,
            "⚙️ Hooks separated by execution mode"
        );

        let timeout = self.timeout;

        // Wrap in Arc once so parallel hooks share the same allocation
        let event_arc = Arc::new(event.clone());

        // Execute parallel hooks concurrently
        let parallel_futures: Vec<_> = parallel_hooks
            .into_iter()
            .map(|hook| {
                let event = Arc::clone(&event_arc);
                async move {
                    match tokio::time::timeout(timeout, hook.on_event(&event)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            tracing::warn!(hook = hook.name(), error = %e, "Hook execution failed");
                        }
                        Err(_) => {
                            tracing::warn!(hook = hook.name(), "Hook execution timed out");
                        }
                    }
                }
            })
            .collect();

        futures_util::future::join_all(parallel_futures).await;

        tracing::debug!(
            event_type = event.event_type(),
            parallel_hooks_completed = parallel_count,
            "✅ Parallel hooks completed"
        );

        // Execute sequential hooks one by one
        for hook in sequential_hooks {
            match tokio::time::timeout(timeout, hook.on_event(event)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::warn!(hook = hook.name(), error = %e, "Hook execution failed");
                }
                Err(_) => {
                    tracing::warn!(hook = hook.name(), "Hook execution timed out");
                }
            }
        }

        tracing::debug!(
            event_type = event.event_type(),
            sequential_hooks_completed = sequential_count,
            "✅ Sequential hooks completed"
        );
    }

    /// Returns the number of registered hooks
    ///
    /// # Returns
    ///
    /// The total number of registered hooks
    pub async fn count(&self) -> usize {
        let hooks = self.hooks.read().await;
        hooks.len()
    }

    /// Clears all registered hooks
    pub async fn clear(&self) {
        tracing::debug!("⚙️ Clearing all hooks");

        let mut hooks = self.hooks.write().await;
        let count = hooks.len();
        hooks.clear();

        tracing::debug!(hooks_cleared = count, "✅ All hooks cleared");
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
            timeout: self.timeout,
        }
    }
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry").field("hooks_count", &"<async>").finish()
    }
}

impl std::fmt::Display for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HookRegistry(timeout={}s)", self.timeout.as_secs())
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
        #[derive(Clone)]
        struct SimpleHook<F>
        where
            F: Fn(&$crate::events::DownloadEvent) -> $crate::events::HookResult + Clone + Send + Sync,
        {
            name: &'static str,
            filter: $crate::events::EventFilter,
            closure: F,
        }

        #[$crate::async_trait::async_trait]
        impl<F> $crate::events::EventHook for SimpleHook<F>
        where
            F: Fn(&$crate::events::DownloadEvent) -> $crate::events::HookResult + Clone + Send + Sync,
        {
            async fn on_event(&self, event: &$crate::events::DownloadEvent) -> $crate::events::HookResult {
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
