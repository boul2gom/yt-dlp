//! Subtitle format conversion utilities.
//!
//! This module provides functionality to convert between different subtitle formats,
//! primarily VTT (WebVTT) and SRT (SubRip).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tokio::fs;

use crate::error::{Error, Result};
use crate::model::caption::Extension;

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

    tracing::debug!(
        input = ?input_path,
        output = ?output_path,
        target_format = ?target_format,
        "💬 Converting subtitle"
    );

    // Read input file
    let content = fs::read_to_string(input_path).await?;

    // Detect source format
    let source_format = super::detect_subtitle_format(&content)?;

    tracing::debug!(format = ?source_format, "💬 Detected source subtitle format");

    // Convert based on source and target formats
    let converted_content = match (source_format, target_format) {
        (Extension::Vtt, Extension::Srt) => vtt_to_srt(&content)?,
        (Extension::Srt, Extension::Vtt) => srt_to_vtt(&content)?,
        (source, target) if source == target => {
            tracing::debug!("💬 Source and target formats are the same, copying file");
            content
        }
        (source, target) => {
            return Err(Error::FormatIncompatible {
                format_id: format!("{source:?}"),
                reason: format!("Unsupported subtitle conversion to {target:?}"),
            });
        }
    };

    // Write output file
    fs::write(output_path, converted_content).await?;

    tracing::info!(path = ?output_path, "✅ Successfully converted subtitle");

    Ok(())
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
    let mut in_note_block = false;

    for line in lines {
        let trimmed = line.trim();

        // NOTE and STYLE blocks can span multiple lines until the next blank line
        if trimmed.starts_with("NOTE") || trimmed.starts_with("STYLE") {
            in_note_block = true;
            continue;
        }

        if in_note_block {
            if trimmed.is_empty() {
                in_note_block = false;
            }
            continue;
        }

        if trimmed.contains(" --> ") {
            // Only replace dots in the timestamp portions, not in position/alignment metadata
            let converted_timestamp = convert_vtt_timestamp_line(trimmed);
            current_subtitle.push(converted_timestamp);
            in_subtitle = true;
        } else if trimmed.is_empty() {
            flush_subtitle(
                &mut srt_output,
                &mut subtitle_index,
                &mut current_subtitle,
                &mut in_subtitle,
            );
        } else if in_subtitle {
            let cleaned_text = remove_vtt_tags(trimmed);
            if !cleaned_text.is_empty() {
                current_subtitle.push(cleaned_text);
            }
        }
    }

    // Handle last subtitle if present
    flush_subtitle(
        &mut srt_output,
        &mut subtitle_index,
        &mut current_subtitle,
        &mut in_subtitle,
    );

    Ok(srt_output)
}

fn flush_subtitle(output: &mut String, index: &mut usize, subtitle: &mut Vec<String>, in_subtitle: &mut bool) {
    if !*in_subtitle || subtitle.is_empty() {
        return;
    }

    output.push_str(&format!("{}\n", index));
    for sub_line in subtitle.iter() {
        output.push_str(&format!("{}\n", sub_line));
    }
    output.push('\n');

    *index += 1;
    subtitle.clear();
    *in_subtitle = false;
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
    static RE_VOICE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<v\s+[^>]+>").unwrap());
    static RE_CLASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<c\.[^>]+>").unwrap());
    static RE_CLOSING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"</[cv]>").unwrap());
    static RE_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<\d{2}:\d{2}:\d{2}\.\d{3}>").unwrap());

    let text = RE_VOICE.replace_all(text, "");
    let text = RE_CLASS.replace_all(&text, "");
    let text = RE_CLOSING.replace_all(&text, "");
    let text = RE_TIMESTAMP.replace_all(&text, "");

    text.to_string()
}

/// Converts a VTT timestamp line to SRT format.
///
/// Only replaces dots with commas in the timestamp portions (HH:MM:SS.mmm),
/// preserving any VTT position/alignment metadata that follows.
fn convert_vtt_timestamp_line(line: &str) -> String {
    static RE_TS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{2}:\d{2}:\d{2})\.(\d{3})").unwrap());
    RE_TS.replace_all(line, "$1,$2").to_string()
}
