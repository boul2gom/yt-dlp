use std::time::Duration;

/// Strategy for retrying failed webhook deliveries
#[derive(Debug, Clone, Copy)]
pub struct RetryStrategy {
    /// Maximum number of retry attempts
    pub max_attempts: usize,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl RetryStrategy {
    /// Creates a new retry strategy with exponential backoff
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of retry attempts (0 means no retries)
    /// * `initial_delay` - Initial delay before first retry
    /// * `max_delay` - Maximum delay between retries
    ///
    /// # Returns
    ///
    /// A RetryStrategy with exponential backoff (2x multiplier)
    pub fn exponential(max_attempts: usize, initial_delay: Duration, max_delay: Duration) -> Self {
        tracing::debug!(
            max_attempts = max_attempts,
            initial_delay_ms = initial_delay.as_millis(),
            max_delay_ms = max_delay.as_millis(),
            "⚙️ Creating exponential retry strategy"
        );

        Self {
            max_attempts,
            initial_delay,
            max_delay,
            backoff_multiplier: 2.0,
        }
    }

    /// Creates a strategy with linear backoff
    ///
    /// # Arguments
    ///
    /// * `max_attempts` - Maximum number of retry attempts
    /// * `delay` - Fixed delay between retries
    ///
    /// # Returns
    ///
    /// A RetryStrategy with linear backoff (constant delay)
    pub fn linear(max_attempts: usize, delay: Duration) -> Self {
        tracing::debug!(
            max_attempts = max_attempts,
            delay_ms = delay.as_millis(),
            "⚙️ Creating linear retry strategy"
        );

        Self {
            max_attempts,
            initial_delay: delay,
            max_delay: delay,
            backoff_multiplier: 1.0,
        }
    }

    /// Creates a strategy with no retries
    ///
    /// # Returns
    ///
    /// A RetryStrategy that never retries
    pub fn none() -> Self {
        tracing::debug!("⚙️ Creating no-retry strategy");

        Self {
            max_attempts: 0,
            initial_delay: Duration::from_secs(0),
            max_delay: Duration::from_secs(0),
            backoff_multiplier: 1.0,
        }
    }

    /// Calculates the delay for a specific attempt
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (0-indexed)
    ///
    /// # Returns
    ///
    /// The delay duration for this attempt
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        if attempt >= self.max_attempts {
            return Duration::from_secs(0);
        }

        let delay_secs = self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);

        let result = Duration::from_secs_f64(delay_secs.min(self.max_delay.as_secs_f64()));

        tracing::debug!(
            attempt = attempt,
            delay_ms = result.as_millis(),
            "🔄 Calculated retry delay"
        );

        result
    }

    /// Returns true if retries should continue
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (0-indexed)
    ///
    /// # Returns
    ///
    /// true if more retries are allowed, false otherwise
    pub fn should_retry(&self, attempt: usize) -> bool {
        let should_retry = attempt < self.max_attempts;

        tracing::debug!(
            attempt = attempt,
            max_attempts = self.max_attempts,
            should_retry = should_retry,
            "🔄 Checking retry status"
        );

        should_retry
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        // Default: 3 retries with exponential backoff starting at 1 second, max 30 seconds
        Self::exponential(3, Duration::from_secs(1), Duration::from_secs(30))
    }
}

impl std::fmt::Display for RetryStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RetryStrategy(max_attempts={}, initial_delay={}ms, backoff={}x)",
            self.max_attempts,
            self.initial_delay.as_millis(),
            self.backoff_multiplier
        )
    }
}
