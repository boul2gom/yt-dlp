//! FFmpeg command builder and execution utilities.
//!
//! Provides a builder pattern for constructing FFmpeg arguments
//! and a helper for the common temp-file + rename execution pattern.

use std::path::Path;
use std::time::Duration;

use super::Executor;
use crate::error::{Error, Result};
use crate::utils::fs::remove_temp_file;

/// Builder for constructing FFmpeg command arguments.
///
/// # Example
///
/// ```rust,no_run
/// use yt_dlp::executor::FfmpegArgs;
///
/// let args = FfmpegArgs::new()
///     .input("/tmp/input.mp4")
///     .input("/tmp/audio.mp3")
///     .args(["-map", "0:v", "-map", "1:a"])
///     .codec_copy()
///     .output("/tmp/output.mkv")
///     .build();
/// ```
pub struct FfmpegArgs {
    parts: Vec<String>,
    output: Option<String>,
    overwrite: bool,
}

impl FfmpegArgs {
    /// Creates a new empty FFmpeg argument builder.
    ///
    /// # Returns
    ///
    /// A new `FfmpegArgs` with no arguments or output set.
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            output: None,
            overwrite: false,
        }
    }

    /// Adds an input file (`-i <path>`).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the input file
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn input(mut self, path: impl AsRef<str>) -> Self {
        self.parts.push("-i".to_string());
        self.parts.push(path.as_ref().to_string());
        self
    }

    /// Adds global codec copy (`-c copy`).
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn codec_copy(mut self) -> Self {
        self.parts.push("-c".to_string());
        self.parts.push("copy".to_string());
        self
    }

    /// Adds the overwrite flag (`-y`).
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn overwrite(mut self) -> Self {
        self.overwrite = true;
        self
    }

    /// Sets the output path (always placed last).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the output file
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn output(mut self, path: impl AsRef<str>) -> Self {
        self.output = Some(path.as_ref().to_string());
        self
    }

    /// Adds arbitrary arguments.
    ///
    /// # Arguments
    ///
    /// * `args` - Iterator of arguments to append
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.parts.extend(args.into_iter().map(Into::into));
        self
    }

    /// Adds a single argument.
    ///
    /// # Arguments
    ///
    /// * `arg` - The argument to append
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.parts.push(arg.into());
        self
    }

    /// Builds the final argument list.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` with all arguments in the correct order:
    /// inputs and flags first, then `-y` if set, then the output path.
    pub fn build(mut self) -> Vec<String> {
        if self.overwrite {
            self.parts.push("-y".to_string());
        }

        if let Some(output) = self.output {
            self.parts.push(output);
        }

        self.parts
    }
}

impl Default for FfmpegArgs {
    fn default() -> Self {
        Self::new()
    }
}

/// Executes an FFmpeg command writing to a temporary file, then renames over the original.
///
/// This is the common pattern used by metadata operations: write to a temp file,
/// verify success, then atomically replace the original.
///
/// # Arguments
///
/// * `ffmpeg_path` - Path to the FFmpeg executable
/// * `base_path` - The original file path (will be overwritten on success)
/// * `extension` - File extension for the temp file
/// * `args` - FFmpeg arguments (output path will be appended automatically)
/// * `timeout` - Execution timeout
///
/// # Errors
///
/// Returns an error if FFmpeg fails or the rename operation fails
pub async fn run_ffmpeg_with_tempfile(
    ffmpeg_path: &Path,
    base_path: &Path,
    extension: &str,
    args: FfmpegArgs,
    timeout: Duration,
) -> Result<()> {
    let temp_output_path = crate::utils::fs::create_temp_path(base_path, extension);
    let temp_output_str = temp_output_path
        .to_str()
        .ok_or_else(|| Error::path_validation(&temp_output_path, "Invalid output path"))?;

    tracing::debug!(
        base_path = ?base_path,
        temp_path = ?temp_output_path,
        timeout_secs = timeout.as_secs(),
        "✂️ Running ffmpeg with temp file"
    );

    let final_args = args.overwrite().output(temp_output_str).build();

    let executor = Executor::new(ffmpeg_path.to_path_buf(), final_args, timeout);
    if let Err(e) = executor.execute().await {
        // Clean up temp file on execution failure (timeout, process error, etc.)
        if temp_output_path.exists() {
            remove_temp_file(&temp_output_path).await;
        }
        return Err(e);
    }

    tokio::fs::rename(&temp_output_path, base_path).await?;
    tracing::debug!(base_path = ?base_path, "✅ ffmpeg temp file renamed to final path");
    Ok(())
}
