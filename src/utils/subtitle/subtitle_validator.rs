//! Subtitle validation utilities.
//!
//! This module provides functionality to validate subtitle files before embedding
//! or processing them, ensuring they are well-formed and compatible.

use crate::error::{Error, Result};
use crate::model::caption::Extension;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;
use tokio::fs;

static VTT_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d{2}):(\d{2}):(\d{2})\.(\d{3})\s+-->\s+(\d{2}):(\d{2}):(\d{2})\.(\d{3})")
        .expect("valid VTT timestamp regex")
});

static SRT_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d{2}):(\d{2}):(\d{2}),(\d{3})\s+-->\s+(\d{2}):(\d{2}):(\d{2}),(\d{3})")
        .expect("valid SRT timestamp regex")
});

static SRT_INDEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+$").expect("valid SRT index regex"));

/// Validation result containing detailed information about subtitle file validity.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    /// Whether the subtitle file is valid
    pub is_valid: bool,
    /// The detected subtitle format
    pub format: Option<Extension>,
    /// Number of subtitle entries found
    pub entry_count: usize,
    /// Validation errors encountered
    pub errors: Vec<String>,
    /// Validation warnings (non-critical issues)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Creates a new validation result indicating success.
    pub fn valid(format: Extension, entry_count: usize) -> Self {
        Self {
            is_valid: true,
            format: Some(format),
            entry_count,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Creates a new validation result indicating failure.
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            format: None,
            entry_count: 0,
            errors,
            warnings: Vec::new(),
        }
    }

    /// Adds a warning to the validation result.
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Adds multiple warnings to the validation result.
    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings.extend(warnings);
        self
    }
}

/// Validate a subtitle file.
///
/// # Arguments
///
/// * `subtitle_path` - Path to the subtitle file to validate
///
/// # Errors
///
/// Returns an error if the file cannot be read
///
/// # Returns
///
/// Returns a `ValidationResult` containing detailed validation information
pub async fn validate_subtitle(subtitle_path: impl AsRef<Path>) -> Result<ValidationResult> {
    let subtitle_path = subtitle_path.as_ref();

    tracing::debug!("Validating subtitle file: {:?}", subtitle_path);

    // Check if file exists
    if !subtitle_path.exists() {
        return Ok(ValidationResult::invalid(vec![
            "Subtitle file does not exist".to_string(),
        ]));
    }

    // Check if file is readable
    let content = match fs::read_to_string(subtitle_path).await {
        Ok(content) => content,
        Err(e) => {
            return Ok(ValidationResult::invalid(vec![format!(
                "Failed to read subtitle file: {}",
                e
            )]));
        }
    };

    // Check if file is empty
    if content.trim().is_empty() {
        return Ok(ValidationResult::invalid(vec![
            "Subtitle file is empty".to_string(),
        ]));
    }

    // Detect format
    let format = match detect_subtitle_format(&content) {
        Ok(fmt) => fmt,
        Err(_) => {
            return Ok(ValidationResult::invalid(vec![
                "Could not detect subtitle format".to_string(),
            ]));
        }
    };

    tracing::debug!("Detected subtitle format: {:?}", format);

    // Validate based on format
    match format {
        Extension::Vtt => validate_vtt(&content),
        Extension::Srt => validate_srt(&content),
        Extension::Ass | Extension::Ssa => validate_ass(&content),
        _ => Ok(ValidationResult::invalid(vec![format!(
            "Unsupported subtitle format for validation: {:?}",
            format
        )])),
    }
}

/// Detect the format of a subtitle file based on its content.
///
/// # Arguments
///
/// * `content` - The subtitle file content
///
/// # Errors
///
/// Returns an error if the format cannot be detected
fn detect_subtitle_format(content: &str) -> Result<Extension> {
    let trimmed = content.trim();

    // VTT files start with "WEBVTT"
    if trimmed.starts_with("WEBVTT") {
        return Ok(Extension::Vtt);
    }

    // ASS/SSA files have a [Script Info] section
    if trimmed.contains("[Script Info]") {
        if trimmed.contains("[V4+ Styles]") {
            return Ok(Extension::Ass);
        }
        return Ok(Extension::Ssa);
    }

    // SRT files start with a number and have the timestamp format HH:MM:SS,mmm --> HH:MM:SS,mmm
    if trimmed
        .lines()
        .any(|line| line.contains(" --> ") && line.contains(',') && !trimmed.starts_with("WEBVTT"))
    {
        return Ok(Extension::Srt);
    }

    Err(Error::Unknown(
        "Could not detect subtitle format".to_string(),
    ))
}

/// Helper to parse timestamp captures
fn parse_timestamp(caps: &regex::Captures, offset: usize) -> f64 {
    let h: f64 = caps[offset + 1].parse().unwrap_or(0.0);
    let m: f64 = caps[offset + 2].parse().unwrap_or(0.0);
    let s: f64 = caps[offset + 3].parse().unwrap_or(0.0);
    let ms: f64 = caps[offset + 4].parse().unwrap_or(0.0);
    h * 3600.0 + m * 60.0 + s + ms / 1000.0
}

/// Validate VTT subtitle content.
fn validate_vtt(content: &str) -> Result<ValidationResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut entry_count = 0;

    // Check for WEBVTT header
    if !content.trim().starts_with("WEBVTT") {
        errors.push("VTT file must start with 'WEBVTT' header".to_string());
    }

    // Validate timestamps and count entries
    let mut last_end_time: Option<f64> = None;

    for line in content.lines() {
        if let Some(caps) = VTT_TIMESTAMP_RE.captures(line) {
            entry_count += 1;

            // Parse start and end times
            let start_time = parse_timestamp(&caps, 0);
            let end_time = parse_timestamp(&caps, 4);

            // Validate time range
            if start_time >= end_time {
                errors.push(format!(
                    "Invalid time range in entry {}: start time must be before end time",
                    entry_count
                ));
            }

            // Check timestamp ordering
            if let Some(last_end) = last_end_time
                && start_time < last_end
            {
                warnings.push(format!(
                    "Entry {} has overlapping or out-of-order timestamps",
                    entry_count
                ));
            }

            last_end_time = Some(end_time);
        }
    }

    if entry_count == 0 {
        errors.push("No valid subtitle entries found".to_string());
    }

    let result = if errors.is_empty() {
        ValidationResult::valid(Extension::Vtt, entry_count).with_warnings(warnings)
    } else {
        ValidationResult::invalid(errors).with_warnings(warnings)
    };

    Ok(result)
}

/// Validate SRT subtitle content.
fn validate_srt(content: &str) -> Result<ValidationResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut entry_count = 0;

    // Validate timestamps and count entries

    let mut last_end_time: Option<f64> = None;
    let mut last_index = 0;
    let mut expect_index = true;
    let mut has_index = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            expect_index = true;
            continue;
        }

        // Check for subtitle index
        if expect_index && SRT_INDEX_RE.is_match(trimmed) {
            has_index = true;
            let index: usize = trimmed.parse().unwrap_or(0);

            if index != last_index + 1 && last_index != 0 {
                warnings.push(format!(
                    "Subtitle index {} is not sequential (expected {})",
                    index,
                    last_index + 1
                ));
            }

            last_index = index;
            expect_index = false;
            continue;
        }

        // Check for timestamp line
        if let Some(caps) = SRT_TIMESTAMP_RE.captures(trimmed) {
            entry_count += 1;
            expect_index = false;

            // Parse start and end times
            let start_time = parse_timestamp(&caps, 0);
            let end_time = parse_timestamp(&caps, 4);

            // Validate time range
            if start_time >= end_time {
                errors.push(format!(
                    "Invalid time range in entry {}: start time must be before end time",
                    entry_count
                ));
            }

            // Check timestamp ordering
            if let Some(last_end) = last_end_time
                && start_time < last_end
            {
                warnings.push(format!(
                    "Entry {} has overlapping or out-of-order timestamps",
                    entry_count
                ));
            }

            last_end_time = Some(end_time);
        }
    }

    if entry_count == 0 {
        errors.push("No valid subtitle entries found".to_string());
    }

    if !has_index && entry_count > 0 {
        warnings.push("SRT file is missing subtitle indices".to_string());
    }

    let result = if errors.is_empty() {
        ValidationResult::valid(Extension::Srt, entry_count).with_warnings(warnings)
    } else {
        ValidationResult::invalid(errors).with_warnings(warnings)
    };

    Ok(result)
}

/// Validate ASS/SSA subtitle content.
fn validate_ass(content: &str) -> Result<ValidationResult> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut entry_count = 0;

    // Check for required sections
    if !content.contains("[Script Info]") {
        errors.push("ASS/SSA file must contain [Script Info] section".to_string());
    }

    if !content.contains("[Events]") {
        errors.push("ASS/SSA file must contain [Events] section".to_string());
    }

    // Determine format
    let format = if content.contains("[V4+ Styles]") {
        Extension::Ass
    } else {
        Extension::Ssa
    };

    // Count dialogue lines
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Dialogue:") {
            entry_count += 1;
        }
    }

    if entry_count == 0 {
        warnings.push("No dialogue lines found in ASS/SSA file".to_string());
    }

    let result = if errors.is_empty() {
        ValidationResult::valid(format, entry_count).with_warnings(warnings)
    } else {
        ValidationResult::invalid(errors).with_warnings(warnings)
    };

    Ok(result)
}

/// Check if a subtitle format is compatible with a video container format.
///
/// # Arguments
///
/// * `subtitle_format` - The subtitle format extension
/// * `container_format` - The video container format extension (e.g., "mp4", "mkv", "webm")
///
/// # Returns
///
/// Returns `true` if the subtitle format can be embedded in the container
pub fn is_format_compatible(subtitle_format: &Extension, container_format: &str) -> bool {
    match container_format.to_lowercase().as_str() {
        "mp4" | "m4v" => {
            // MP4 supports MOV_TEXT format (VTT-like)
            matches!(subtitle_format, Extension::Vtt | Extension::Srt)
        }
        "mkv" | "webm" => {
            // Matroska supports most subtitle formats
            matches!(
                subtitle_format,
                Extension::Vtt | Extension::Srt | Extension::Ass | Extension::Ssa
            )
        }
        "avi" => {
            // AVI typically supports SRT
            matches!(subtitle_format, Extension::Srt)
        }
        _ => {
            // For unknown formats, be permissive
            true
        }
    }
}
