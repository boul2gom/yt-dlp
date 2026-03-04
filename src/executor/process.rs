//! Process execution and output handling.

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::{Error, Result};

/// Represents the output of a process.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessOutput {
    /// The stdout of the process.
    pub stdout: String,
    /// The stderr of the process.
    pub stderr: String,
    /// The exit code of the process.
    pub code: i32,
}

impl std::fmt::Display for ProcessOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ProcessOutput(code={}, stdout_len={}, stderr_len={})",
            self.code,
            self.stdout.len(),
            self.stderr.len()
        )
    }
}

/// Executes a command with the given arguments and timeout.
///
/// # Arguments
///
/// * `executable_path` - Path to the executable
/// * `args` - Arguments to pass to the command
/// * `timeout` - Maximum duration to wait for the process
///
/// # Errors
///
/// Returns an error if the command fails, times out, or cannot be executed
pub async fn execute_command(
    executable_path: impl Into<PathBuf>,
    args: &[String],
    timeout: Duration,
) -> Result<ProcessOutput> {
    execute_command_internal(executable_path, args, timeout, None).await
}

/// Executes a command and redirects stdout to a file.
///
/// # Arguments
///
/// * `executable_path` - Path to the executable
/// * `args` - Arguments to pass to the command
/// * `timeout` - Maximum duration to wait for the process
/// * `output_path` - Path to the file where stdout will be written
///
/// # Errors
///
/// Returns an error if the command fails, times out, or cannot be executed
pub async fn execute_command_to_file(
    executable_path: impl Into<PathBuf>,
    args: &[String],
    timeout: Duration,
    output_path: impl Into<PathBuf>,
) -> Result<ProcessOutput> {
    execute_command_internal(executable_path, args, timeout, Some(output_path.into())).await
}

/// Internal command execution with optional file output
///
/// # Arguments
///
/// * `executable_path` - Path to the executable
/// * `args` - Arguments to pass to the command
/// * `timeout` - Maximum duration to wait for the process
/// * `output_path` - Optional path to redirect stdout to a file
///
/// # Returns
///
/// ProcessOutput containing stdout (if not redirected), stderr, and exit code
///
/// # Errors
///
/// Returns an error if the command fails, times out, or cannot be executed
// LCOV_EXCL_START — requires real yt-dlp/ffmpeg binary on PATH
async fn execute_command_internal(
    executable_path: impl Into<PathBuf>,
    args: &[String],
    timeout: Duration,
    output_path: Option<PathBuf>,
) -> Result<ProcessOutput> {
    let executable_path: PathBuf = executable_path.into();

    tracing::debug!(
        executable = ?executable_path,
        arg_count = args.len(),
        timeout_secs = timeout.as_secs(),
        output_to_file = output_path.is_some(),
        output_path = ?output_path,
        "⚙️ Starting command execution"
    );

    let mut command = tokio::process::Command::new(&executable_path);

    // Configure stdout: either pipe (memory) or file
    if let Some(path) = &output_path {
        let file = std::fs::File::create(path)?;
        command.stdout(std::process::Stdio::from(file));
    } else {
        command.stdout(std::process::Stdio::piped());
    }

    command.stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);

    command.args(args);

    tracing::debug!(
        executable = ?executable_path,
        "⚙️ Spawning child process"
    );

    let mut child = command.spawn()?;

    tracing::debug!(
        executable = ?executable_path,
        pid = ?child.id(),
        "✅ Child process spawned"
    );

    // Read streams asynchronously
    let stdout_task = if output_path.is_none() {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::io("capture stdout", std::io::Error::other("stdout stream not available")))?;

        Some(tokio::spawn(read_stream(stdout)))
    } else {
        None
    };

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::io("capture stderr", std::io::Error::other("stderr stream not available")))?;

    let stderr_task = tokio::spawn(read_stream(stderr));

    tracing::debug!(
        executable = ?executable_path,
        timeout_secs = timeout.as_secs(),
        "⚙️ Waiting for process to complete"
    );

    // Wait for the process to finish with timeout
    let exit_status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => result?,
        Err(_) => {
            tracing::warn!(
                executable = ?executable_path,
                timeout_secs = timeout.as_secs(),
                "⚙️ Process timed out, killing it"
            );

            if let Err(e) = child.kill().await {
                tracing::error!(
                    executable = ?executable_path,
                    error = %e,
                    "⚙️ Failed to kill process after timeout"
                );
            } else if let Err(e) = child.wait().await {
                tracing::error!(
                    executable = ?executable_path,
                    error = %e,
                    "⚙️ Failed to wait for process after kill"
                );
            }

            return Err(Error::Timeout {
                operation: format!("executing command: {}", executable_path.display()),
                duration: timeout,
            });
        }
    };

    tracing::debug!(
        executable = ?executable_path,
        exit_code = exit_status.code().unwrap_or(-1),
        success = exit_status.success(),
        "⚙️ Process completed"
    );

    // Read stderr stream
    let stderr_result = match stderr_task.await {
        Ok(Ok(buffer)) => buffer,
        Ok(Err(e)) => return Err(Error::io("reading command stderr", e)),
        Err(e) => return Err(Error::runtime("reading command stderr task", e)),
    };

    let stdout_result = if let Some(task) = stdout_task {
        match task.await {
            Ok(Ok(buffer)) => buffer,
            Ok(Err(e)) => return Err(Error::io("reading command stdout", e)),
            Err(e) => return Err(Error::runtime("reading command stdout task", e)),
        }
    } else {
        Vec::new()
    };

    // Convert the buffers to Strings (lossy to avoid errors on non-UTF8 output)
    let stdout = String::from_utf8_lossy(&stdout_result).to_string();
    let stderr = String::from_utf8_lossy(&stderr_result).to_string();
    let code = exit_status.code().unwrap_or(-1);

    tracing::debug!(
        executable = ?executable_path,
        exit_code = code,
        stdout_len = stdout.len(),
        stderr_len = stderr.len(),
        "⚙️ Command output captured"
    );

    if exit_status.success() {
        tracing::debug!(
            executable = ?executable_path,
            exit_code = code,
            "✅ Command execution succeeded"
        );

        return Ok(ProcessOutput { stdout, stderr, code });
    }

    tracing::warn!(
        executable = ?executable_path,
        exit_code = code,
        stderr_preview = if stderr.len() > 100 {
            &stderr[..100]
        } else {
            &stderr
        },
        "⚙️ Command execution failed"
    );

    Err(Error::CommandFailed {
        command: executable_path.display().to_string(),
        exit_code: code,
        stderr,
    })
}
// LCOV_EXCL_STOP

/// Helper function to read a stream into a buffer
///
/// # Arguments
///
/// * `stream` - An async readable stream (stdout or stderr)
///
/// # Returns
///
/// A vector of bytes containing all data read from the stream
///
/// # Errors
///
/// Returns an IO error if reading fails
async fn read_stream<R>(mut stream: R) -> std::io::Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut buffer = Vec::new();
    let bytes_read = tokio::io::copy(&mut tokio::io::BufReader::new(&mut stream), &mut buffer).await?;

    tracing::trace!(bytes_read, "Stream read completed");
    Ok(buffer)
}
