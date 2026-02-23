//! Retry logic with exponential backoff for handling transient failures.
//!
//! This module provides robust retry mechanisms for async operations that may fail
//! due to network issues or temporary unavailability.

use std::time::Duration;
use tokio::time::sleep;
use typed_builder::TypedBuilder;

/// Configuration for retry behavior with exponential backoff.
///
/// # Examples
///
/// ```rust
/// use yt_dlp::utils::retry::RetryPolicy;
/// use std::time::Duration;
///
/// let policy = RetryPolicy::builder()
///     .max_attempts(5)
///     .initial_delay(Duration::from_millis(100))
///     .max_delay(Duration::from_secs(30))
///     .build();
/// ```
#[derive(Debug, Clone, TypedBuilder)]
pub struct RetryPolicy {
    #[builder(default = 3)]
    max_attempts: u32,
    #[builder(default = Duration::from_millis(500))]
    initial_delay: Duration,
    #[builder(default = Duration::from_secs(60))]
    max_delay: Duration,
    #[builder(default = 2.0)]
    backoff_factor: f64,
    #[builder(default = true)]
    jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Create a new retry policy with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the delay for a specific retry attempt.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The retry attempt number (0-based)
    fn calculate_delay(&self, attempt: u32) -> Duration {
        // Calculate exponential backoff: initial_delay * (backoff_factor ^ attempt)
        let base_delay =
            self.initial_delay.as_millis() as f64 * self.backoff_factor.powi(attempt as i32);

        // Cap at max_delay
        let delay_ms = base_delay.min(self.max_delay.as_millis() as f64);

        // Add jitter if enabled (random value between 0% and 20% of delay)
        let final_delay_ms = if self.jitter {
            use rand::prelude::*;
            let mut rng = rand::rng();
            let jitter_factor: f64 = rng.random_range(0.8..=1.0);
            delay_ms * jitter_factor
        } else {
            delay_ms
        };

        Duration::from_millis(final_delay_ms as u64)
    }

    /// Execute an async operation with retry logic.
    ///
    /// # Arguments
    ///
    /// * `operation` - The async operation to retry
    ///
    /// # Returns
    ///
    /// The result of the operation, or the last error if all retries fail.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::utils::retry::RetryPolicy;
    /// # use yt_dlp::error::Result;
    /// # async fn example() -> Result<String> {
    /// let policy = RetryPolicy::default();
    /// let result = policy.execute(|| async {
    ///     // Your async operation here
    ///     Ok::<_, yt_dlp::error::Error>("success".to_string())
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// ```
    pub async fn execute<F, Fut, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut last_error = None;

        for attempt in 0..self.max_attempts {
            if attempt > 0 {
                tracing::debug!("Retry attempt {}/{}", attempt + 1, self.max_attempts);
            }

            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        tracing::info!("Operation succeeded after {} retry attempts", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    tracing::warn!(
                        "Operation failed (attempt {}/{}): {}",
                        attempt + 1,
                        self.max_attempts,
                        e
                    );

                    last_error = Some(e);

                    // Don't sleep after the last attempt
                    if attempt + 1 < self.max_attempts {
                        let delay = self.calculate_delay(attempt);

                        tracing::debug!("Waiting {:?} before retry", delay);

                        sleep(delay).await;
                    }
                }
            }
        }

        // All retries failed, return the last error
        Err(last_error
            .unwrap_or_else(|| unreachable!("retry loop exited without recording an error")))
    }

    /// Execute an async operation with retry logic and a condition for retryable errors.
    ///
    /// # Arguments
    ///
    /// * `operation` - The async operation to retry
    /// * `is_retryable` - Function to determine if an error should trigger a retry
    ///
    /// # Returns
    ///
    /// The result of the operation, or the last error if all retries fail or a non-retryable error occurs.
    pub async fn execute_with_condition<F, Fut, T, E, P>(
        &self,
        mut operation: F,
        is_retryable: P,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
        P: Fn(&E) -> bool,
    {
        let mut last_error = None;

        for attempt in 0..self.max_attempts {
            if attempt > 0 {
                tracing::debug!("Retry attempt {}/{}", attempt + 1, self.max_attempts);
            }

            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        tracing::info!("Operation succeeded after {} retry attempts", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    // Check if the error is retryable
                    if !is_retryable(&e) {
                        tracing::warn!("Non-retryable error encountered: {}", e);
                        return Err(e);
                    }

                    tracing::warn!(
                        "Operation failed (attempt {}/{}): {}",
                        attempt + 1,
                        self.max_attempts,
                        e
                    );

                    last_error = Some(e);

                    // Don't sleep after the last attempt
                    if attempt + 1 < self.max_attempts {
                        let delay = self.calculate_delay(attempt);

                        tracing::debug!("Waiting {:?} before retry", delay);

                        sleep(delay).await;
                    }
                }
            }
        }

        // All retries failed, return the last error
        Err(last_error
            .unwrap_or_else(|| unreachable!("retry loop exited without recording an error")))
    }

    /// Get the maximum number of attempts.
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Get the initial delay.
    pub fn initial_delay(&self) -> Duration {
        self.initial_delay
    }

    /// Get the maximum delay.
    pub fn max_delay(&self) -> Duration {
        self.max_delay
    }

    /// Get the backoff factor.
    pub fn backoff_factor(&self) -> f64 {
        self.backoff_factor
    }

    /// Check if jitter is enabled.
    pub fn has_jitter(&self) -> bool {
        self.jitter
    }
}

/// Check if an HTTP error is retryable (transient network error or server error).
///
/// # Arguments
///
/// * `error` - The reqwest error to check
///
/// # Returns
///
/// True if the error is likely transient and worth retrying.
pub fn is_http_error_retryable(error: &reqwest::Error) -> bool {
    // Retry on network/connection errors
    if error.is_timeout() || error.is_connect() || error.is_request() {
        return true;
    }

    // Retry on specific HTTP status codes
    if let Some(status) = error.status() {
        let code = status.as_u16();
        // Retry on server errors (5xx) and specific client errors
        return matches!(
            code,
            408 | // Request Timeout
            429 | // Too Many Requests
            500..=599 // Server errors
        );
    }

    false
}

/// Check if a database error is retryable.
///
/// # Arguments
///
/// * `error` - The sqlx error to check
///
/// # Returns
///
/// True if the error is likely transient and worth retrying.
#[cfg(feature = "cache-sqlite")]
pub fn is_database_error_retryable(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_)
    )
}
