//! A tool for executing commands.

use crate::error::{Error, Result};
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
#[derive(Debug, Clone, PartialEq)]
pub struct Executor {
    /// The path to the command executable.
    pub executable_path: PathBuf,
    /// The timeout for the process.
    pub timeout: Duration,

    /// The arguments to pass to the command.
    pub args: Vec<String>,
}

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

impl Executor {
    /// Executes the command and returns the output.
    ///
    /// # Errors
    ///
    /// This function will return an error if the command could not be executed, or if the process timed out.
    pub async fn execute(&self) -> Result<ProcessOutput> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Executing command: {:?}", self);

        let mut command = tokio::process::Command::new(&self.executable_path);
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        command.args(&self.args);
        let mut child = command.spawn()?;

        // Continually read from stdout so that it does not fill up with large output and hang forever.
        // We don't need to do this for stderr since only stdout has potentially giant JSON.
        // This code was taken from youtube-dl-rs.
        let stdout_handle = child
            .stdout
            .take()
            .ok_or_else(|| Error::Command("Failed to capture stdout".to_string()))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or_else(|| Error::Command("Failed to capture stderr".to_string()))?;

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
        let exit_status = match tokio::time::timeout(self.timeout, child.wait()).await {
            Ok(result) => result?,
            Err(_) => {
                // In case of timeout, kill the process and all its children
                #[cfg(feature = "tracing")]
                tracing::warn!("Process timed out after {:?}, killing it", self.timeout);

                // Try to kill the process
                if let Err(_e) = child.kill().await {
                    #[cfg(feature = "tracing")]
                    tracing::error!("Failed to kill process after timeout: {}", _e);
                }

                return Err(Error::Timeout(self.timeout));
            }
        };

        // Get the results of the read tasks
        let stdout_result = match stdout_task.await {
            Ok(Ok(buffer)) => buffer,
            Ok(Err(e)) => return Err(Error::IO(e)),
            Err(e) => return Err(Error::Runtime(e)),
        };

        let stderr_result = match stderr_task.await {
            Ok(Ok(buffer)) => buffer,
            Ok(Err(e)) => return Err(Error::IO(e)),
            Err(e) => return Err(Error::Runtime(e)),
        };

        // Convert the buffers to Strings
        let stdout = String::from_utf8(stdout_result)
            .map_err(|_| Error::Command("Failed to parse stdout as UTF-8".to_string()))?;
        let stderr = String::from_utf8(stderr_result)
            .map_err(|_| Error::Command("Failed to parse stderr as UTF-8".to_string()))?;

        let code = exit_status.code().unwrap_or(-1);
        if exit_status.success() {
            return Ok(ProcessOutput {
                stdout,
                stderr,
                code,
            });
        }

        Err(Error::Command(format!(
            "Process failed with code {}: {}",
            code, stderr
        )))
    }
}
