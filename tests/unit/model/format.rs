use std::str::FromStr;

use pretty_assertions::assert_eq;
use yt_dlp::model::format::{Container, DynamicRange, Extension, Format, FormatType, Protocol};

use crate::common::fixtures;

// ============================== Format ==============================

#[test]
fn format_is_video() {
    let video = fixtures::load_video_fixture();
    let format_248 = video.formats.iter().find(|f| f.format_id == "248").unwrap();
    assert!(format_248.is_video());
    assert!(!format_248.is_audio());
    assert_eq!(format_248.format_type(), FormatType::Video);
}

#[test]
fn format_is_audio() {
    let video = fixtures::load_video_fixture();
    let format_251 = video.formats.iter().find(|f| f.format_id == "251").unwrap();
    assert!(format_251.is_audio());
    assert!(!format_251.is_video());
    assert_eq!(format_251.format_type(), FormatType::Audio);
}

#[test]
fn format_is_storyboard() {
    let video = fixtures::load_video_fixture();
    let sb = video.formats.iter().find(|f| f.format_id == "sb0").unwrap();
    assert_eq!(sb.format_type(), FormatType::Storyboard);
}

#[test]
fn format_url_available() {
    let video = fixtures::load_video_fixture();
    let format_251 = video.formats.iter().find(|f| f.format_id == "251").unwrap();
    assert!(format_251.url().is_ok());
    assert!(format_251.url().unwrap().contains("MOCK_SERVER"));
}

#[test]
fn format_display() {
    let video = fixtures::load_video_fixture();
    let format_248 = video.formats.iter().find(|f| f.format_id == "248").unwrap();
    let display = format!("{}", format_248);
    assert!(display.contains("248"));
}

// ============================== FormatType ==============================

#[test]
fn format_type_checks() {
    assert!(FormatType::Audio.is_audio());
    assert!(!FormatType::Audio.is_video());
    assert!(FormatType::Video.is_video());
    assert!(!FormatType::Video.is_audio());
    assert!(FormatType::AudioVideo.is_audio_and_video());
    assert!(FormatType::Storyboard.is_storyboard());
    assert!(FormatType::Manifest.is_manifest());
    assert!(!FormatType::Unknown.is_audio());
    assert!(!FormatType::Unknown.is_video());
}

#[test]
fn format_type_display() {
    assert_eq!(format!("{}", FormatType::Audio), "Audio");
    assert_eq!(format!("{}", FormatType::Video), "Video");
    assert_eq!(format!("{}", FormatType::AudioVideo), "AudioVideo");
    assert_eq!(format!("{}", FormatType::Manifest), "Manifest");
    assert_eq!(format!("{}", FormatType::Storyboard), "Storyboard");
    assert_eq!(format!("{}", FormatType::Unknown), "Unknown");
}

// ============================== Extension ==============================

#[test]
fn extension_as_str() {
    assert_eq!(Extension::Mp4.as_str(), "mp4");
    assert_eq!(Extension::Webm.as_str(), "webm");
    assert_eq!(Extension::M4A.as_str(), "m4a");
    assert_eq!(Extension::Mp3.as_str(), "mp3");
    assert_eq!(Extension::Flac.as_str(), "flac");
    assert_eq!(Extension::Ogg.as_str(), "ogg");
    assert_eq!(Extension::Wav.as_str(), "wav");
    assert_eq!(Extension::Aac.as_str(), "aac");
    assert_eq!(Extension::Aiff.as_str(), "aiff");
    assert_eq!(Extension::Avi.as_str(), "avi");
    assert_eq!(Extension::Ts.as_str(), "ts");
    assert_eq!(Extension::Flv.as_str(), "flv");
    assert_eq!(Extension::Mhtml.as_str(), "mhtml");
    assert_eq!(Extension::None.as_str(), "bin");
    assert_eq!(Extension::Unknown.as_str(), "bin");
}

#[test]
fn extension_from_str() {
    assert_eq!(Extension::from_str("mp4").unwrap(), Extension::Mp4);
    assert_eq!(Extension::from_str("WEBM").unwrap(), Extension::Webm);
    assert_eq!(Extension::from_str("m4a").unwrap(), Extension::M4A);
    assert_eq!(Extension::from_str("opus").unwrap(), Extension::Ogg);
    assert_eq!(Extension::from_str("aif").unwrap(), Extension::Aiff);
    assert_eq!(Extension::from_str("m2ts").unwrap(), Extension::Ts);
    assert_eq!(Extension::from_str("xyz").unwrap(), Extension::Unknown);
    assert_eq!(Extension::from_str("none").unwrap(), Extension::None);
}

#[test]
fn extension_display() {
    assert_eq!(format!("{}", Extension::Mp4), "Mp4");
    assert_eq!(format!("{}", Extension::Webm), "Webm");
    assert_eq!(format!("{}", Extension::M4A), "M4A");
}

#[test]
fn extension_serde_roundtrip() {
    let ext = Extension::Mp4;
    let json = serde_json::to_string(&ext).unwrap();
    let deserialized: Extension = serde_json::from_str(&json).unwrap();
    assert_eq!(ext, deserialized);
}

#[test]
fn extension_serde_unknown_variant() {
    let deserialized: Extension = serde_json::from_str("\"some_unknown_ext\"").unwrap();
    assert_eq!(deserialized, Extension::Unknown);
}

// ============================== Protocol ==============================

#[test]
fn protocol_display() {
    assert_eq!(format!("{}", Protocol::Https), "Https");
    assert_eq!(format!("{}", Protocol::M3U8Native), "HLS");
    assert_eq!(format!("{}", Protocol::Mhtml), "Mhtml");
    assert_eq!(format!("{}", Protocol::Unknown), "Unknown");
}

// ============================== DynamicRange ==============================

#[test]
fn dynamic_range_display() {
    assert_eq!(format!("{}", DynamicRange::SDR), "SDR");
    assert_eq!(format!("{}", DynamicRange::HDR), "HDR");
    assert_eq!(format!("{}", DynamicRange::Unknown), "Unknown");
}

// ============================== Container ==============================

#[test]
fn container_display() {
    assert_eq!(format!("{}", Container::Mp4), "Mp4");
    assert_eq!(format!("{}", Container::Webm), "Webm");
    assert_eq!(format!("{}", Container::M4A), "M4A");
    assert_eq!(format!("{}", Container::Unknown), "Unknown");
}

// ============================== HttpHeaders ==============================

#[test]
fn http_headers_browser_defaults() {
    let headers = yt_dlp::model::format::HttpHeaders::browser_defaults("my-agent/1.0".to_string());
    assert_eq!(headers.user_agent, "my-agent/1.0");
    assert_eq!(headers.accept, "*/*");
    assert_eq!(headers.accept_language, "en-US,en");
    assert_eq!(headers.sec_fetch_mode, "navigate");
}

#[test]
fn http_headers_to_header_map() {
    let headers = yt_dlp::model::format::HttpHeaders::browser_defaults("test-agent".to_string());
    let map = headers.to_header_map();

    assert_eq!(map.get("user-agent").unwrap(), "test-agent");
    assert_eq!(map.get("accept").unwrap(), "*/*");
    assert_eq!(map.get("accept-language").unwrap(), "en-US,en");
    assert_eq!(map.get("sec-fetch-mode").unwrap(), "navigate");
}

#[test]
fn http_headers_serde_round_trip() {
    let headers = yt_dlp::model::format::HttpHeaders::browser_defaults("agent".to_string());
    let json = serde_json::to_string(&headers).unwrap();
    let back: yt_dlp::model::format::HttpHeaders = serde_json::from_str(&json).unwrap();
    assert_eq!(headers.user_agent, back.user_agent);
    assert_eq!(headers.accept, back.accept);
}

// ============================== Format serde round-trip ==============================

#[test]
fn format_serde_round_trip() {
    let video = fixtures::load_video_fixture();
    let format = &video.formats[0];
    let serialized = serde_json::to_string(format).expect("Serialize failed");
    let deserialized: Format = serde_json::from_str(&serialized).expect("Deserialize failed");

    assert_eq!(format.format_id, deserialized.format_id);
    assert_eq!(format.format, deserialized.format);
    assert_eq!(format.codec_info, deserialized.codec_info);
    assert_eq!(format.video_resolution, deserialized.video_resolution);
}
