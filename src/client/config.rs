//! Configuration types for the Youtube client.

use std::time::Duration;

/// Configuration for the Youtube client
#[derive(Debug, Clone)]
pub struct YoutubeConfig {
    /// The arguments to pass to 'yt-dlp'
    pub args: Vec<String>,
    /// The timeout for command execution
    pub timeout: Duration,
}

impl Default for YoutubeConfig {
    fn default() -> Self {
        Self {
            args: Vec::new(),
            timeout: Duration::from_secs(30),
        }
    }
}

impl YoutubeConfig {
    /// Creates a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the arguments to pass to yt-dlp
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Adds a single argument to pass to yt-dlp
    pub fn add_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Sets the timeout for command execution
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
