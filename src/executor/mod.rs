//! Command execution module.
//!
//! This module provides tools for executing commands with timeout support.

pub mod ffmpeg;
pub mod process;

pub use ffmpeg::{FfmpegArgs, run_ffmpeg_with_tempfile};
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
    ///
    /// # Returns
    ///
    /// A new Executor instance
    pub fn new<I, S>(executable_path: impl Into<PathBuf>, args: I, timeout: Duration) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let executable_path = executable_path.into();
        let args: Vec<String> = args.into_iter().map(Into::into).collect();

        tracing::debug!(
            executable = ?executable_path,
            arg_count = args.len(),
            timeout_secs = timeout.as_secs(),
            "Creating new Executor"
        );

        Self {
            executable_path,
            args,
            timeout,
        }
    }

    /// Returns the executable path.
    ///
    /// # Returns
    ///
    /// Reference to the executable path
    pub fn executable_path(&self) -> &PathBuf {
        &self.executable_path
    }

    /// Returns the arguments.
    ///
    /// # Returns
    ///
    /// Slice of command arguments
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Returns the timeout.
    ///
    /// # Returns
    ///
    /// Timeout duration for command execution
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Executes the command and returns the output.
    ///
    /// # Returns
    ///
    /// ProcessOutput containing stdout, stderr, and exit code
    ///
    /// # Errors
    ///
    /// This function will return an error if the command could not be executed, or if the process timed out.
    pub async fn execute(&self) -> Result<ProcessOutput> {
        tracing::debug!(
            executable = ?self.executable_path,
            arg_count = self.args.len(),
            timeout_secs = self.timeout.as_secs(),
            "Executing command"
        );

        let result = execute_command(&self.executable_path, &self.args, self.timeout).await;

        match &result {
            Ok(output) => tracing::debug!(
                executable = ?self.executable_path,
                exit_code = output.code,
                stdout_len = output.stdout.len(),
                stderr_len = output.stderr.len(),
                "Command execution completed successfully"
            ),
            Err(e) => tracing::warn!(
                executable = ?self.executable_path,
                error = %e,
                "Command execution failed"
            ),
        }

        result
    }

    /// Executes the command and redirects stdout to a file.
    ///
    /// # Arguments
    ///
    /// * `output_path` - The path where stdout will be written
    ///
    /// # Returns
    ///
    /// ProcessOutput containing stderr and exit code (stdout is written to file)
    ///
    /// # Errors
    ///
    /// This function will return an error if the command could not be executed, if the process timed out,
    /// or if the output file could not be created.
    pub async fn execute_to_file(&self, output_path: impl Into<PathBuf>) -> Result<ProcessOutput> {
        let output_path = output_path.into();

        tracing::debug!(
            executable = ?self.executable_path,
            arg_count = self.args.len(),
            output_path = ?output_path,
            timeout_secs = self.timeout.as_secs(),
            "Executing command to file"
        );

        let result = process::execute_command_to_file(
            &self.executable_path,
            &self.args,
            self.timeout,
            &output_path,
        )
        .await;

        match &result {
            Ok(output) => tracing::debug!(
                executable = ?self.executable_path,
                output_path = ?output_path,
                exit_code = output.code,
                stderr_len = output.stderr.len(),
                "Command execution to file completed successfully"
            ),
            Err(e) => tracing::warn!(
                executable = ?self.executable_path,
                output_path = ?output_path,
                error = %e,
                "Command execution to file failed"
            ),
        }

        result
    }
}
