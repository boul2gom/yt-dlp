//! Command execution module.
//!
//! This module provides tools for executing commands with timeout support.

pub mod process;

pub use process::{ProcessOutput, execute_command};

use crate::error::Result;
use std::path::PathBuf;
use std::time::Duration;

/// Represents a command executor.
///
/// # Example
///
/// ```rust,no_run
/// # use yt_dlp::utils;
/// # use std::path::PathBuf;
/// # use std::time::Duration;
/// # use yt_dlp::executor::Executor;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let args = vec!["--update"];
///
/// let executor = Executor::new(
///     PathBuf::from("yt-dlp"),
///     utils::to_owned(args),
///     Duration::from_secs(30),
/// );
///
/// let output = executor.execute().await?;
/// println!("Output: {}", output.stdout);
///
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Executor {
    /// The path to the command executable.
    executable_path: PathBuf,
    /// The timeout for the process.
    timeout: Duration,
    /// The arguments to pass to the command.
    args: Vec<String>,
}

impl Executor {
    /// Creates a new Executor.
    ///
    /// # Arguments
    ///
    /// * `executable_path` - Path to the executable
    /// * `args` - Arguments to pass to the command
    /// * `timeout` - Timeout for the command
    pub fn new(executable_path: PathBuf, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            executable_path,
            args,
            timeout,
        }
    }

    /// Returns the executable path.
    pub fn executable_path(&self) -> &PathBuf {
        &self.executable_path
    }

    /// Returns the arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns the timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Executes the command and returns the output.
    ///
    /// # Errors
    ///
    /// This function will return an error if the command could not be executed, or if the process timed out.
    pub async fn execute(&self) -> Result<ProcessOutput> {
        execute_command(&self.executable_path, &self.args, self.timeout).await
    }
}
