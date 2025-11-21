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
    pub fn exponential(max_attempts: usize, initial_delay: Duration, max_delay: Duration) -> Self {
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
    pub fn linear(max_attempts: usize, delay: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay: delay,
            max_delay: delay,
            backoff_multiplier: 1.0,
        }
    }

    /// Creates a strategy with no retries
    pub fn none() -> Self {
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

        let delay_secs =
            self.initial_delay.as_secs_f64() * self.backoff_multiplier.powi(attempt as i32);

        Duration::from_secs_f64(delay_secs.min(self.max_delay.as_secs_f64()))
    }

    /// Returns true if retries should continue
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (0-indexed)
    pub fn should_retry(&self, attempt: usize) -> bool {
        attempt < self.max_attempts
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        // Default: 3 retries with exponential backoff starting at 1 second, max 30 seconds
        Self::exponential(3, Duration::from_secs(1), Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let strategy =
            RetryStrategy::exponential(3, Duration::from_secs(1), Duration::from_secs(10));

        // First retry: 1 second
        assert_eq!(strategy.delay_for_attempt(0).as_secs(), 1);

        // Second retry: 2 seconds
        assert_eq!(strategy.delay_for_attempt(1).as_secs(), 2);

        // Third retry: 4 seconds
        assert_eq!(strategy.delay_for_attempt(2).as_secs(), 4);

        // Fourth retry would be 8 seconds, but we only have 3 max attempts
        assert!(!strategy.should_retry(3));
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = RetryStrategy::linear(3, Duration::from_secs(5));

        // All retries should have the same delay
        assert_eq!(strategy.delay_for_attempt(0).as_secs(), 5);
        assert_eq!(strategy.delay_for_attempt(1).as_secs(), 5);
        assert_eq!(strategy.delay_for_attempt(2).as_secs(), 5);
    }

    #[test]
    fn test_max_delay() {
        let strategy =
            RetryStrategy::exponential(10, Duration::from_secs(1), Duration::from_secs(10));

        // With exponential backoff of 2^10 = 1024 seconds, it should cap at 10 seconds
        assert_eq!(strategy.delay_for_attempt(10).as_secs(), 0); // Exceeds max_attempts
        assert_eq!(strategy.delay_for_attempt(9).as_secs(), 10); // Capped at max_delay
    }

    #[test]
    fn test_no_retry() {
        let strategy = RetryStrategy::none();

        assert_eq!(strategy.max_attempts, 0);
        assert!(!strategy.should_retry(0));
    }
}
