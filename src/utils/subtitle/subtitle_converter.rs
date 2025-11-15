//! Subtitle format conversion utilities.
//!
//! This module provides functionality to convert between different subtitle formats,
//! primarily VTT (WebVTT) and SRT (SubRip).

use crate::error::{Error, Result};
use crate::model::caption::Extension;
use regex::Regex;
use std::path::Path;
use tokio::fs;

/// Convert a subtitle file from one format to another.
///
/// # Arguments
///
/// * `input_path` - Path to the input subtitle file
/// * `output_path` - Path to the output subtitle file
/// * `target_format` - Target format for conversion
///
/// # Errors
///
/// Returns an error if the file cannot be read, the format is unsupported, or conversion fails
///
/// # Supported Conversions
///
/// - VTT to SRT
/// - SRT to VTT
pub async fn convert_subtitle(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    target_format: Extension,
) -> Result<()> {
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Converting subtitle from {:?} to {:?} (format: {:?})",
        input_path,
        output_path,
        target_format
    );

    // Read input file
    let content = fs::read_to_string(input_path).await?;

    // Detect source format
    let source_format = detect_subtitle_format(&content)?;

    #[cfg(feature = "tracing")]
    tracing::debug!("Detected source format: {:?}", source_format);

    // Convert based on source and target formats
    let converted_content = match (source_format, target_format) {
        (Extension::Vtt, Extension::Srt) => vtt_to_srt(&content)?,
        (Extension::Srt, Extension::Vtt) => srt_to_vtt(&content)?,
        (source, target) if source == target => {
            #[cfg(feature = "tracing")]
            tracing::debug!("Source and target formats are the same, copying file");
            content
        }
        (source, target) => {
            return Err(Error::Unknown(format!(
                "Unsupported subtitle conversion: {:?} to {:?}",
                source, target
            )));
        }
    };

    // Write output file
    fs::write(output_path, converted_content).await?;

    #[cfg(feature = "tracing")]
    tracing::info!("Successfully converted subtitle to {:?}", output_path);

    Ok(())
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

    // SRT files start with a number (the first subtitle index)
    // and have the timestamp format HH:MM:SS,mmm --> HH:MM:SS,mmm
    if trimmed
        .lines()
        .any(|line| line.contains(" --> ") && line.contains(','))
    {
        return Ok(Extension::Srt);
    }

    Err(Error::Unknown(
        "Could not detect subtitle format".to_string(),
    ))
}

/// Convert VTT (WebVTT) format to SRT (SubRip) format.
///
/// # Arguments
///
/// * `vtt_content` - The VTT subtitle content
///
/// # Errors
///
/// Returns an error if the conversion fails
fn vtt_to_srt(vtt_content: &str) -> Result<String> {
    let mut srt_output = String::new();
    let mut subtitle_index = 1;
    let mut lines = vtt_content.lines();

    // Skip the WEBVTT header and any metadata
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            break;
        }
    }

    let mut current_subtitle: Vec<String> = Vec::new();
    let mut in_subtitle = false;

    for line in lines {
        let trimmed = line.trim();

        // Skip cue identifiers and NOTE comments
        if trimmed.starts_with("NOTE") || trimmed.starts_with("STYLE") {
            continue;
        }

        // Check if this is a timestamp line
        if trimmed.contains(" --> ") {
            // Convert VTT timestamp format (HH:MM:SS.mmm) to SRT format (HH:MM:SS,mmm)
            let converted_timestamp = trimmed.replace('.', ",");
            current_subtitle.push(converted_timestamp);
            in_subtitle = true;
        } else if trimmed.is_empty() {
            // End of current subtitle
            if in_subtitle && !current_subtitle.is_empty() {
                srt_output.push_str(&format!("{}\n", subtitle_index));
                for sub_line in &current_subtitle {
                    srt_output.push_str(&format!("{}\n", sub_line));
                }
                srt_output.push('\n');

                subtitle_index += 1;
                current_subtitle.clear();
                in_subtitle = false;
            }
        } else if in_subtitle {
            // This is subtitle text
            // Remove VTT tags like <c.classname> or <v Speaker>
            let cleaned_text = remove_vtt_tags(trimmed);
            if !cleaned_text.is_empty() {
                current_subtitle.push(cleaned_text);
            }
        }
    }

    // Handle last subtitle if present
    if in_subtitle && !current_subtitle.is_empty() {
        srt_output.push_str(&format!("{}\n", subtitle_index));
        for sub_line in &current_subtitle {
            srt_output.push_str(&format!("{}\n", sub_line));
        }
        srt_output.push('\n');
    }

    Ok(srt_output)
}

/// Convert SRT (SubRip) format to VTT (WebVTT) format.
///
/// # Arguments
///
/// * `srt_content` - The SRT subtitle content
///
/// # Errors
///
/// Returns an error if the conversion fails
fn srt_to_vtt(srt_content: &str) -> Result<String> {
    let mut vtt_output = String::from("WEBVTT\n\n");
    let lines: Vec<&str> = srt_content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip subtitle index numbers
        if line.chars().all(|c| c.is_ascii_digit()) {
            i += 1;
            continue;
        }

        // Check if this is a timestamp line
        if line.contains(" --> ") {
            // Convert SRT timestamp format (HH:MM:SS,mmm) to VTT format (HH:MM:SS.mmm)
            let converted_timestamp = line.replace(',', ".");
            vtt_output.push_str(&format!("{}\n", converted_timestamp));
            i += 1;

            // Add subtitle text lines until we hit an empty line
            while i < lines.len() {
                let text_line = lines[i].trim();
                if text_line.is_empty() {
                    vtt_output.push('\n');
                    break;
                }
                vtt_output.push_str(&format!("{}\n", text_line));
                i += 1;
            }
        }

        i += 1;
    }

    Ok(vtt_output)
}

/// Remove VTT-specific tags from subtitle text.
///
/// # Arguments
///
/// * `text` - The subtitle text potentially containing VTT tags
fn remove_vtt_tags(text: &str) -> String {
    // Remove voice tags: <v Speaker>
    let re_voice = Regex::new(r"<v\s+[^>]+>").unwrap();
    let text = re_voice.replace_all(text, "");

    // Remove class tags: <c.classname>
    let re_class = Regex::new(r"<c\.[^>]+>").unwrap();
    let text = re_class.replace_all(&text, "");

    // Remove closing tags: </c> or </v>
    let re_closing = Regex::new(r"</[cv]>").unwrap();
    let text = re_closing.replace_all(&text, "");

    // Remove timestamp tags: <00:00:00.000>
    let re_timestamp = Regex::new(r"<\d{2}:\d{2}:\d{2}\.\d{3}>").unwrap();
    let text = re_timestamp.replace_all(&text, "");

    text.to_string()
}
