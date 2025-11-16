//! Post-processing execution using FFmpeg.
//!
//! This module provides functions to apply post-processing operations
//! to video files using FFmpeg based on PostProcessConfig.

use crate::client::Libraries;
use crate::download::postprocess::PostProcessConfig;
use crate::error::{Error, Result};
use crate::executor::Executor;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    config: &PostProcessConfig,
    libraries: &Libraries,
    timeout: Duration,
) -> Result<PathBuf> {
    if config.is_empty() {
        // No processing needed, just copy or return input
        return Ok(input_path.as_ref().to_path_buf());
    }

    let input_str = input_path
        .as_ref()
        .to_str()
        .ok_or_else(|| Error::PathValidation {
            path: input_path.as_ref().to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

    let output_str = output_path
        .as_ref()
        .to_str()
        .ok_or_else(|| Error::PathValidation {
            path: output_path.as_ref().to_path_buf(),
            reason: "Invalid UTF-8 in path".to_string(),
        })?;

    let args = build_ffmpeg_command(input_str, output_str, config)?;

    let executor = Executor {
        executable_path: libraries.ffmpeg.clone(),
        timeout,
        args,
    };

    executor.execute().await?;
    Ok(output_path.as_ref().to_path_buf())
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
pub fn build_ffmpeg_command(
    input: &str,
    output: &str,
    config: &PostProcessConfig,
) -> Result<Vec<String>> {
    let mut args = vec!["-i".to_string(), input.to_string()];

    // Add video codec
    if let Some(ref video_codec) = config.video_codec {
        args.push("-c:v".to_string());
        args.push(video_codec.to_ffmpeg_name().to_string());
    }

    // Add audio codec
    if let Some(ref audio_codec) = config.audio_codec {
        args.push("-c:a".to_string());
        args.push(audio_codec.to_ffmpeg_name().to_string());
    }

    // Add video bitrate
    if let Some(ref bitrate) = config.video_bitrate {
        args.push("-b:v".to_string());
        args.push(bitrate.clone());
    }

    // Add audio bitrate
    if let Some(ref bitrate) = config.audio_bitrate {
        args.push("-b:a".to_string());
        args.push(bitrate.clone());
    }

    // Add framerate
    if let Some(fps) = config.framerate {
        args.push("-r".to_string());
        args.push(fps.to_string());
    }

    // Add preset
    if let Some(ref preset) = config.preset {
        args.push("-preset".to_string());
        args.push(preset.to_ffmpeg_name().to_string());
    }

    // Build video filter chain
    let mut filter_chain = Vec::new();

    // Add resolution/scale filter
    if let Some(ref resolution) = config.resolution {
        filter_chain.push(format!("scale={}", resolution.to_ffmpeg_scale()));
    }

    // Add custom filters
    for filter in &config.filters {
        filter_chain.push(filter.to_ffmpeg_string());
    }

    // Add filter chain to args
    if !filter_chain.is_empty() {
        args.push("-vf".to_string());
        args.push(filter_chain.join(","));
    }

    // Add output file
    args.push(output.to_string());

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::postprocess::{AudioCodec, EncodingPreset, Resolution, VideoCodec};

    #[test]
    fn test_build_basic_command() {
        let config = PostProcessConfig::new()
            .with_video_codec(VideoCodec::H264)
            .with_audio_codec(AudioCodec::AAC)
            .with_video_bitrate("2M")
            .with_audio_bitrate("192k");

        let args = build_ffmpeg_command("input.mp4", "output.mp4", &config).unwrap();

        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"input.mp4".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(args.contains(&"-b:v".to_string()));
        assert!(args.contains(&"2M".to_string()));
        assert!(args.contains(&"-b:a".to_string()));
        assert!(args.contains(&"192k".to_string()));
        assert!(args.contains(&"output.mp4".to_string()));
    }

    #[test]
    fn test_build_command_with_resolution() {
        let config = PostProcessConfig::new().with_resolution(Resolution::HD);

        let args = build_ffmpeg_command("input.mp4", "output.mp4", &config).unwrap();

        assert!(args.contains(&"-vf".to_string()));
        let vf_index = args.iter().position(|x| x == "-vf").unwrap();
        assert_eq!(args[vf_index + 1], "scale=1280:720");
    }

    #[test]
    fn test_build_command_with_preset() {
        let config = PostProcessConfig::new()
            .with_video_codec(VideoCodec::H264)
            .with_preset(EncodingPreset::Fast);

        let args = build_ffmpeg_command("input.mp4", "output.mp4", &config).unwrap();

        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"fast".to_string()));
    }

    #[test]
    fn test_empty_config() {
        let config = PostProcessConfig::new();
        assert!(config.is_empty());

        let config_with_codec = PostProcessConfig::new().with_video_codec(VideoCodec::H264);
        assert!(!config_with_codec.is_empty());
    }
}
