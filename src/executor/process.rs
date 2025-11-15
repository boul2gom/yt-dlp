//! Process execution and output handling.

use crate::error::{Error, Result};
use std::path::PathBuf;
use std::time::Duration;

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
    executable_path: &PathBuf,
    args: &[String],
    timeout: Duration,
) -> Result<ProcessOutput> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Executing command: {:?} with args: {:?}",
        executable_path,
        args
    );

    let mut command = tokio::process::Command::new(executable_path);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    command.args(args);
    let mut child = command.spawn()?;

    // Read stdout and stderr asynchronously
    let stdout_handle = child
        .stdout
        .take()
        .ok_or_else(|| Error::Unknown("Failed to capture stdout".to_string()))?;
    let stderr_handle = child
        .stderr
        .take()
        .ok_or_else(|| Error::Unknown("Failed to capture stderr".to_string()))?;

    // Create tasks to read stdout and stderr asynchronously
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        tokio::io::copy(&mut tokio::io::BufReader::new(stdout_handle), &mut buffer).await?;
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });

    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        tokio::io::copy(&mut tokio::io::BufReader::new(stderr_handle), &mut buffer).await?;
        Ok::<Vec<u8>, std::io::Error>(buffer)
    });

    // Wait for the process to finish with timeout
    let exit_status = match tokio::time::timeout(timeout, child.wait()).await {
        Ok(result) => result?,
        Err(_) => {
            #[cfg(feature = "tracing")]
            tracing::warn!("Process timed out after {:?}, killing it", timeout);

            if let Err(_e) = child.kill().await {
                #[cfg(feature = "tracing")]
                tracing::error!("Failed to kill process after timeout: {}", _e);
            }

            return Err(Error::Timeout {
                operation: format!("executing command: {}", executable_path.display()),
                duration: timeout,
            });
        }
    };

    // Get the results of the read tasks
    let stdout_result = match stdout_task.await {
        Ok(Ok(buffer)) => buffer,
        Ok(Err(e)) => return Err(Error::io("reading command stdout", e)),
        Err(e) => return Err(Error::runtime("reading command stdout task", e)),
    };

    let stderr_result = match stderr_task.await {
        Ok(Ok(buffer)) => buffer,
        Ok(Err(e)) => return Err(Error::io("reading command stderr", e)),
        Err(e) => return Err(Error::runtime("reading command stderr task", e)),
    };

    // Convert the buffers to Strings
    let stdout = String::from_utf8(stdout_result)
        .map_err(|_| Error::Unknown("Failed to parse stdout as UTF-8".to_string()))?;
    let stderr = String::from_utf8(stderr_result)
        .map_err(|_| Error::Unknown("Failed to parse stderr as UTF-8".to_string()))?;

    let code = exit_status.code().unwrap_or(-1);
    if exit_status.success() {
        return Ok(ProcessOutput {
            stdout,
            stderr,
            code,
        });
    }

    Err(Error::CommandFailed {
        command: executable_path.display().to_string(),
        exit_code: code,
        stderr,
    })
}
