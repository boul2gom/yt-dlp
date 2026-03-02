//! Post-processing execution using FFmpeg.
//!
//! This module provides functions to apply post-processing operations
//! to video files using FFmpeg based on PostProcessConfig.

use std::path::PathBuf;
use std::time::Duration;

use crate::client::Libraries;
use crate::download::config::postprocess::PostProcessConfig;
use crate::error::{Error, Result};
use crate::executor::Executor;

/// Applies post-processing to a video file using FFmpeg.
///
/// # Arguments
///
/// * `input_path` - Path to the input video file
/// * `output_path` - Path for the output processed file
/// * `config` - Post-processing configuration
/// * `libraries` - Libraries (for FFmpeg path)
/// * `timeout` - Execution timeout
///
/// # Errors
///
/// Returns an error if FFmpeg execution fails
///
/// # Returns
///
/// The path to the processed video file
pub async fn apply_postprocess(
    input_path: impl Into<PathBuf>,
    output_path: impl Into<PathBuf>,
    config: &PostProcessConfig,
    libraries: &Libraries,
    timeout: Duration,
) -> Result<PathBuf> {
    let input_path: PathBuf = input_path.into();
    let output_path: PathBuf = output_path.into();

    tracing::debug!(
        input_path = ?input_path,
        output_path = ?output_path,
        is_empty = config.is_empty(),
        timeout_secs = timeout.as_secs(),
        "✂️ Applying post-processing to video file"
    );

    if config.is_empty() {
        tracing::debug!(
            input_path = ?input_path,
            "✂️ No post-processing needed, returning input path"
        );
        // No processing needed, just copy or return input
        return Ok(input_path);
    }

    let input_str = input_path.to_str().ok_or_else(|| Error::PathValidation {
        path: input_path.clone(),
        reason: "Invalid UTF-8 in path".to_string(),
    })?;

    let output_str = output_path.to_str().ok_or_else(|| Error::PathValidation {
        path: output_path.clone(),
        reason: "Invalid UTF-8 in path".to_string(),
    })?;

    let args = build_ffmpeg_command(input_str, output_str, config)?;

    tracing::debug!(
        input_path = ?input_path,
        output_path = ?output_path,
        arg_count = args.len(),
        "✂️ Executing FFmpeg post-processing command"
    );

    let executor = Executor::new(libraries.ffmpeg.clone(), args, timeout);

    let result = executor.execute().await;

    match &result {
        Ok(_) => tracing::debug!(
            output_path = ?output_path,
            "✅ Post-processing completed successfully"
        ),
        Err(e) => tracing::warn!(
            input_path = ?input_path,
            output_path = ?output_path,
            error = %e,
            "Post-processing failed"
        ),
    }

    result?;
    Ok(output_path)
}

/// Builds the FFmpeg command arguments from post-processing configuration.
///
/// # Arguments
///
/// * `input` - Input file path
/// * `output` - Output file path
/// * `config` - Post-processing configuration
///
/// # Errors
///
/// Returns an error if configuration is invalid
///
/// # Returns
///
/// Vector of FFmpeg arguments
pub fn build_ffmpeg_command(input: &str, output: &str, config: &PostProcessConfig) -> Result<Vec<String>> {
    tracing::debug!(
        input = input,
        output = output,
        has_video_codec = config.video_codec.is_some(),
        has_audio_codec = config.audio_codec.is_some(),
        has_video_bitrate = config.video_bitrate.is_some(),
        has_audio_bitrate = config.audio_bitrate.is_some(),
        has_framerate = config.framerate.is_some(),
        has_preset = config.preset.is_some(),
        has_resolution = config.resolution.is_some(),
        filter_count = config.filters.len(),
        "✂️ Building FFmpeg command for post-processing"
    );

    let mut builder = crate::executor::FfmpegArgs::new().input(input);

    // Add video codec
    if let Some(ref video_codec) = config.video_codec {
        tracing::trace!(
            video_codec = %video_codec.to_ffmpeg_name(),
            "⚙️ Adding video codec"
        );
        builder = builder.args(["-c:v", video_codec.to_ffmpeg_name()]);
    }

    // Add audio codec
    if let Some(ref audio_codec) = config.audio_codec {
        tracing::trace!(
            audio_codec = %audio_codec.to_ffmpeg_name(),
            "⚙️ Adding audio codec"
        );
        builder = builder.args(["-c:a", audio_codec.to_ffmpeg_name()]);
    }

    // Add video bitrate
    if let Some(ref bitrate) = config.video_bitrate {
        builder = builder.args(["-b:v", bitrate]);
    }

    // Add audio bitrate
    if let Some(ref bitrate) = config.audio_bitrate {
        builder = builder.args(["-b:a", bitrate]);
    }

    // Add framerate
    if let Some(fps) = config.framerate {
        builder = builder.args(["-r", &fps.to_string()]);
    }

    // Add preset
    if let Some(ref preset) = config.preset {
        builder = builder.args(["-preset", preset.to_ffmpeg_name()]);
    }

    // Build video filter chain
    let mut filter_chain = Vec::new();

    // Add resolution/scale filter
    if let Some(ref resolution) = config.resolution {
        let scale_filter = format!("scale={}", resolution.to_ffmpeg_scale());
        tracing::trace!(
            resolution = ?resolution,
            scale_filter = %scale_filter,
            "⚙️ Adding resolution filter"
        );
        filter_chain.push(scale_filter);
    }

    // Add custom filters
    for filter in &config.filters {
        let filter_str = filter.to_ffmpeg_string();
        tracing::trace!(
            filter = %filter_str,
            "⚙️ Adding custom filter"
        );
        filter_chain.push(filter_str);
    }

    // Add filter chain to args
    if !filter_chain.is_empty() {
        let joined = filter_chain.join(",");
        tracing::trace!(
            filter_count = filter_chain.len(),
            filter_chain = %joined,
            "⚙️ Adding video filter chain"
        );
        builder = builder.args(["-vf".to_string(), joined]);
    }

    let args = builder.output(output).build();

    tracing::debug!(arg_count = args.len(), "✅ FFmpeg command built");

    Ok(args)
}
