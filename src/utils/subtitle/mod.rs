pub mod subtitle_converter;
pub mod subtitle_validator;

use crate::error::{Error, Result};
use crate::model::caption::Extension;

/// Detect the format of a subtitle file based on its content.
///
/// # Arguments
///
/// * `content` - The subtitle file content
///
/// # Errors
///
/// Returns an error if the format cannot be detected
pub fn detect_subtitle_format(content: &str) -> Result<Extension> {
    tracing::debug!(content_length = content.len(), "💬 Detecting subtitle format");

    let trimmed = content.trim();

    if trimmed.starts_with("WEBVTT") {
        return Ok(Extension::Vtt);
    }

    if trimmed.contains("[Script Info]") {
        if trimmed.contains("[V4+ Styles]") {
            return Ok(Extension::Ass);
        }
        return Ok(Extension::Ssa);
    }

    if trimmed
        .lines()
        .any(|line| line.contains(" --> ") && line.contains(',') && !trimmed.starts_with("WEBVTT"))
    {
        return Ok(Extension::Srt);
    }

    Err(Error::FormatIncompatible {
        format_id: "unknown".to_string(),
        reason: "Could not detect subtitle format".to_string(),
    })
}
