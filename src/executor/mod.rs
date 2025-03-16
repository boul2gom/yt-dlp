//! A tool for executing commands.

use crate::error::{Error, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncReadExt;

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
    #[cfg_attr(feature = "tracing", tracing::instrument(level = "debug", skip(self)))]
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
        let mut stdout = Vec::new();
        let child_stdout = child.stdout.take();
        tokio::io::copy(&mut child_stdout.unwrap(), &mut stdout).await?;

        let exit_code = match tokio::time::timeout(self.timeout, child.wait()).await {
            Ok(result) => result?,
            Err(_) => {
                child.kill().await?;
                return Err(Error::Timeout(self.timeout));
            }
        };

        let mut stderr = Vec::new();
        if let Some(mut reader) = child.stderr {
            reader.read_to_end(&mut stderr).await?;
        }

        let stdout = String::from_utf8(stdout)
            .map_err(|_| Error::Command("Failed to parse stdout".to_string()))?;
        let stderr = String::from_utf8(stderr)
            .map_err(|_| Error::Command("Failed to parse stderr".to_string()))?;

        let code = exit_code.code().unwrap_or(-1);
        if exit_code.success() {
            return Ok(ProcessOutput {
                stdout,
                stderr,
                code: exit_code.code().unwrap_or(-1),
            });
        }

        Err(Error::Command(format!(
            "Process failed with code {}: {}",
            code, stderr
        )))
    }
}
