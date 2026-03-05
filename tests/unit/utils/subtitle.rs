use tempfile::tempdir;
use yt_dlp::model::caption::Extension;
use yt_dlp::utils::subtitle::detect_subtitle_format;
use yt_dlp::utils::{ValidationResult, convert_subtitle, is_format_compatible, validate_subtitle};

use crate::common::fixtures;

// ============================== detect_subtitle_format ==============================

#[test]
fn detect_format_vtt_from_fixture() {
    let content = fixtures::load_subtitle_string("sample.vtt");
    let ext = detect_subtitle_format(&content).unwrap();
    assert_eq!(ext, Extension::Vtt);
}

#[test]
fn detect_format_srt_from_fixture() {
    let content = fixtures::load_subtitle_string("sample.srt");
    let ext = detect_subtitle_format(&content).unwrap();
    assert_eq!(ext, Extension::Srt);
}

#[test]
fn detect_format_ass() {
    let content = "[Script Info]\n[V4+ Styles]\n[Events]\nDialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,Hello\n";
    let ext = detect_subtitle_format(content).unwrap();
    assert_eq!(ext, Extension::Ass);
}

#[test]
fn detect_format_ssa() {
    let content = "[Script Info]\n[V4 Styles]\n[Events]\nDialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,Hello\n";
    let ext = detect_subtitle_format(content).unwrap();
    assert_eq!(ext, Extension::Ssa);
}

#[test]
fn detect_format_unknown_returns_error() {
    let content = "Some random text that is not a subtitle\n";
    let result = detect_subtitle_format(content);
    assert!(result.is_err());
}

// ============================== ValidationResult constructors ==============================

#[test]
fn build_valid_validation_result_succeeds() {
    let result = ValidationResult::valid(Extension::Vtt, 5);
    assert!(result.is_valid);
    assert_eq!(result.format, Some(Extension::Vtt));
    assert_eq!(result.entry_count, 5);
    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn build_invalid_validation_result_sets_errors() {
    let result = ValidationResult::invalid(vec!["error one".to_string()]);
    assert!(!result.is_valid);
    assert_eq!(result.format, None);
    assert_eq!(result.entry_count, 0);
    assert_eq!(result.errors.len(), 1);
}

#[test]
fn add_warning_to_validation_result_sets_warnings() {
    let result = ValidationResult::valid(Extension::Srt, 2).with_warning("Out of order".to_string());
    assert!(result.is_valid);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0], "Out of order");
}

#[test]
fn add_warnings_to_validation_result_sets_all_warnings() {
    let result = ValidationResult::valid(Extension::Srt, 2)
        .with_warnings(vec!["warning a".to_string(), "warning b".to_string()]);
    assert_eq!(result.warnings.len(), 2);
}

// ============================== is_format_compatible ==============================

#[test]
fn check_format_compatible_mp4_accepts_vtt_and_srt() {
    assert!(is_format_compatible(&Extension::Vtt, "mp4"));
    assert!(is_format_compatible(&Extension::Srt, "mp4"));
    assert!(!is_format_compatible(&Extension::Ass, "mp4"));
}

#[test]
fn check_format_compatible_mkv_accepts_all_formats() {
    assert!(is_format_compatible(&Extension::Vtt, "mkv"));
    assert!(is_format_compatible(&Extension::Srt, "mkv"));
    assert!(is_format_compatible(&Extension::Ass, "mkv"));
    assert!(is_format_compatible(&Extension::Ssa, "mkv"));
}

#[test]
fn check_format_compatible_avi_accepts_srt_only() {
    assert!(is_format_compatible(&Extension::Srt, "avi"));
    assert!(!is_format_compatible(&Extension::Vtt, "avi"));
    assert!(!is_format_compatible(&Extension::Ass, "avi"));
}

#[test]
fn check_format_compatible_unknown_container_is_permissive() {
    assert!(is_format_compatible(&Extension::Ass, "xyz"));
    assert!(is_format_compatible(&Extension::Vtt, "unknown"));
}

#[test]
fn check_format_compatible_is_case_insensitive() {
    assert!(is_format_compatible(&Extension::Vtt, "MP4"));
    assert!(is_format_compatible(&Extension::Vtt, "MKV"));
}

// ============================== validate_subtitle — fixture-based ==============================

#[tokio::test]
async fn validate_vtt_fixture_is_valid() {
    let path = fixtures::subtitle_fixture("sample.vtt");
    let result = validate_subtitle(&path).await.unwrap();
    assert!(result.is_valid);
    assert_eq!(result.format, Some(Extension::Vtt));
    assert_eq!(result.entry_count, 3); // sample.vtt has 3 entries
}

#[tokio::test]
async fn validate_srt_fixture_is_valid() {
    let path = fixtures::subtitle_fixture("sample.srt");
    let result = validate_subtitle(&path).await.unwrap();
    assert!(result.is_valid);
    assert_eq!(result.format, Some(Extension::Srt));
    assert_eq!(result.entry_count, 3); // sample.srt has 3 entries
}

// ============================== validate_subtitle — edge cases ==============================

#[tokio::test]
async fn validate_vtt_missing_header() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.vtt");
    std::fs::write(&path, b"00:00:01.000 --> 00:00:04.000\nHello\n").unwrap();

    // Without WEBVTT header, format detection itself fails → "Could not detect"
    let result = validate_subtitle(&path).await.unwrap();
    assert!(!result.is_valid);
    assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn validate_vtt_invalid_time_range() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad_range.vtt");
    std::fs::write(&path, b"WEBVTT\n\n00:00:05.000 --> 00:00:02.000\nInvalid range\n").unwrap();

    let result = validate_subtitle(&path).await.unwrap();
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("Invalid time range")));
}

#[tokio::test]
async fn validate_nonexistent_file_returns_invalid() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nonexistent.vtt");

    let result = validate_subtitle(&path).await.unwrap();
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("does not exist")));
}

#[tokio::test]
async fn validate_empty_file_returns_invalid() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("empty.vtt");
    std::fs::write(&path, b"").unwrap();

    let result = validate_subtitle(&path).await.unwrap();
    assert!(!result.is_valid);
    assert!(result.errors.iter().any(|e| e.contains("empty")));
}

#[tokio::test]
async fn validate_ass_valid_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.ass");
    std::fs::write(
        &path,
        b"[Script Info]\n[V4+ Styles]\n[Events]\nDialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,Hello\n",
    )
    .unwrap();

    let result = validate_subtitle(&path).await.unwrap();
    assert!(result.is_valid);
    assert_eq!(result.format, Some(Extension::Ass));
}

#[tokio::test]
async fn validate_ass_missing_script_info() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.ass");
    std::fs::write(
        &path,
        b"[V4+ Styles]\n[Events]\nDialogue: 0,0:00:01.00,0:00:04.00,Default,,,,,,Hello\n",
    )
    .unwrap();

    // Without [Script Info], format detection fails → "Could not detect"
    let result = validate_subtitle(&path).await.unwrap();
    assert!(!result.is_valid);
    assert!(!result.errors.is_empty());
}

// ============================== convert_subtitle — fixture-based ==============================

#[tokio::test]
async fn convert_vtt_fixture_to_srt() {
    let input = fixtures::subtitle_fixture("sample.vtt");
    let dir = tempdir().unwrap();
    let output = dir.path().join("output.srt");

    convert_subtitle(&input, &output, Extension::Srt).await.unwrap();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains(","));
    assert!(content.contains("Welcome to the video"));
    // Index numbering
    assert!(content.contains("1\n") || content.contains("1\r\n"));
}

#[tokio::test]
async fn convert_srt_fixture_to_vtt() {
    let input = fixtures::subtitle_fixture("sample.srt");
    let dir = tempdir().unwrap();
    let output = dir.path().join("output.vtt");

    convert_subtitle(&input, &output, Extension::Vtt).await.unwrap();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.starts_with("WEBVTT"));
    assert!(content.contains("."));
    assert!(content.contains("Welcome to the video"));
}

#[tokio::test]
async fn convert_same_format_passthrough() {
    let input = fixtures::subtitle_fixture("sample.srt");
    let dir = tempdir().unwrap();
    let output = dir.path().join("output.srt");

    convert_subtitle(&input, &output, Extension::Srt).await.unwrap();

    let original = std::fs::read_to_string(&input).unwrap();
    let converted = std::fs::read_to_string(&output).unwrap();
    assert_eq!(original, converted);
}

#[tokio::test]
async fn convert_vtt_removes_voice_tags() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("tagged.vtt");
    let output = dir.path().join("output.srt");

    std::fs::write(
        &input,
        b"WEBVTT\n\n00:00:01.000 --> 00:00:04.000\n<v Speaker>Hello world</v>\n",
    )
    .unwrap();

    convert_subtitle(&input, &output, Extension::Srt).await.unwrap();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(!content.contains("<v "));
    assert!(content.contains("Hello world"));
}

#[tokio::test]
async fn convert_unsupported_format_returns_error() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.ass");
    let output = dir.path().join("output.srt");

    std::fs::write(
        &input,
        b"[Script Info]\n[V4+ Styles]\n[Events]\nDialogue: 0,0:00:01.00,0:00:04.00,Default,,,,,,Hello\n",
    )
    .unwrap();

    // ASS → SRT conversion is not supported
    let result = convert_subtitle(&input, &output, Extension::Srt).await;
    assert!(result.is_err());
}
