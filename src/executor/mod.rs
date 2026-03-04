//! Command execution module.
//!
//! This module provides tools for executing commands with timeout support,
//! and long-running streaming processes controllable via cancellation tokens.

pub mod ffmpeg;
pub mod process;

use std::path::PathBuf;
use std::time::Duration;

pub use ffmpeg::{FfmpegArgs, run_ffmpeg_with_tempfile};
pub use process::{ProcessOutput, execute_command};
#[cfg(feature = "live-recording")]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::Result;

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

impl std::fmt::Display for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Executor(path={}, args={}, timeout={}s)",
            self.executable_path.display(),
            self.args.len(),
            self.timeout.as_secs()
        )
    }
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
            "🔧 Creating new Executor"
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
            "⚙️ Executing command"
        );

        let result = execute_command(&self.executable_path, &self.args, self.timeout).await;

        match &result {
            Ok(output) => tracing::debug!(
                executable = ?self.executable_path,
                exit_code = output.code,
                stdout_len = output.stdout.len(),
                stderr_len = output.stderr.len(),
                "✅ Command execution completed"
            ),
            Err(e) => tracing::warn!(
                executable = ?self.executable_path,
                error = %e,
                "⚙️ Command execution failed"
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
            "⚙️ Executing command to file"
        );

        let result =
            process::execute_command_to_file(&self.executable_path, &self.args, self.timeout, &output_path).await;

        match &result {
            Ok(output) => tracing::debug!(
                executable = ?self.executable_path,
                output_path = ?output_path,
                exit_code = output.code,
                stderr_len = output.stderr.len(),
                "✅ Command execution to file completed"
            ),
            Err(e) => tracing::warn!(
                executable = ?self.executable_path,
                output_path = ?output_path,
                error = %e,
                "⚙️ Command execution to file failed"
            ),
        }

        result
    }

    /// Spawns the command as a long-running process without timeout.
    ///
    /// Returns a [`StreamingProcess`] handle that can be stopped gracefully
    /// via stdin `q` (for FFmpeg) or killed. Intended for live recording.
    ///
    /// # Errors
    ///
    /// Returns an error if the process could not be spawned.
    #[cfg(feature = "live-recording")]
    // LCOV_EXCL_START — requires real ffmpeg binary on PATH
    pub async fn execute_streaming(&self) -> Result<StreamingProcess> {
        tracing::debug!(
            executable = ?self.executable_path,
            arg_count = self.args.len(),
            "📥 Spawning long-running streaming process"
        );

        let mut command = tokio::process::Command::new(&self.executable_path);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        command.args(&self.args);

        let child = command.spawn()?;

        tracing::debug!(
            executable = ?self.executable_path,
            pid = ?child.id(),
            "✅ Streaming process spawned"
        );

        Ok(StreamingProcess { child })
    }
    // LCOV_EXCL_STOP
}

/// A long-running child process controllable via stdin or kill.
///
/// Used for FFmpeg-based live recording where the process runs indefinitely
/// until explicitly stopped.
#[cfg(feature = "live-recording")]
pub struct StreamingProcess {
    child: tokio::process::Child,
}

#[cfg(feature = "live-recording")]
impl StreamingProcess {
    /// Sends `q` to stdin to trigger a graceful FFmpeg quit, then waits for exit.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to stdin or waiting fails.
    pub async fn stop(&mut self) -> Result<ProcessOutput> {
        tracing::info!("📥 Stopping streaming process gracefully (stdin q)");

        if let Some(stdin) = self.child.stdin.as_mut() {
            // Ignore write errors (process may have already exited)
            let _ = stdin.write_all(b"q").await;
            let _ = stdin.flush().await;
        }

        self.wait().await
    }

    /// Forcefully kills the process.
    ///
    /// # Errors
    ///
    /// Returns an error if the kill signal cannot be sent.
    pub async fn kill(&mut self) -> Result<()> {
        tracing::warn!("Killing streaming process");
        self.child.kill().await?;
        Ok(())
    }

    /// Waits for the process to exit and collects output.
    ///
    /// # Errors
    ///
    /// Returns an error if waiting for the process fails.
    pub async fn wait(&mut self) -> Result<ProcessOutput> {
        // Read stderr before waiting (stdout may be large for recordings)
        let mut stderr_buf = String::new();
        if let Some(stderr) = self.child.stderr.take() {
            let mut reader = tokio::io::BufReader::new(stderr);
            let _ = reader.read_to_string(&mut stderr_buf).await;
        }

        let status = self.child.wait().await?;
        let code = status.code().unwrap_or(-1);

        tracing::debug!(
            exit_code = code,
            stderr_len = stderr_buf.len(),
            "📥 Streaming process exited"
        );

        Ok(ProcessOutput {
            stdout: String::new(),
            stderr: stderr_buf,
            code,
        })
    }
}
#[cfg(feature = "live-recording")]
impl Drop for StreamingProcess {
    fn drop(&mut self) {
        // Prevent orphaned FFmpeg processes: send SIGKILL on drop
        let _ = self.child.start_kill();
    }
}
