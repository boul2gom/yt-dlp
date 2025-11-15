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
/// let executor = Executor {
///     executable_path: PathBuf::from("yt-dlp"),
///     timeout: Duration::from_secs(30),
///     args: utils::to_owned(args),
/// };
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
    pub executable_path: PathBuf,
    /// The timeout for the process.
    pub timeout: Duration,
    /// The arguments to pass to the command.
    pub args: Vec<String>,
}

impl Executor {
    /// Executes the command and returns the output.
    ///
    /// # Errors
    ///
    /// This function will return an error if the command could not be executed, or if the process timed out.
    pub async fn execute(&self) -> Result<ProcessOutput> {
        execute_command(&self.executable_path, &self.args, self.timeout).await
    }
}
