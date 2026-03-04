use std::error::Error as StdError;
use std::path::PathBuf;

use yt_dlp::error::{ArchiveError, Error};
use yt_dlp::model::format::FormatType;

// ============================== Error Display ==============================

#[test]
fn error_io_display() {
    let err = Error::io(
        "read file",
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    let display = format!("{}", err);
    assert!(display.contains("IO error during read file"));
}

#[test]
fn error_io_with_path_display() {
    let err = Error::io_with_path(
        "read file",
        PathBuf::from("/tmp/test.mp4"),
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    let display = format!("{}", err);
    assert!(display.contains("IO error during read file"));
}

#[test]
fn error_timeout_display() {
    let err = Error::Timeout {
        operation: "downloading".to_string(),
        duration: std::time::Duration::from_secs(30),
    };
    let display = format!("{}", err);
    assert!(display.contains("Timeout"));
    assert!(display.contains("downloading"));
}

#[test]
fn error_command_failed_display() {
    let err = Error::CommandFailed {
        command: "yt-dlp".to_string(),
        exit_code: 1,
        stderr: "error output".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("yt-dlp"));
    assert!(display.contains("1"));
}

#[test]
fn error_video_fetch_display() {
    let err = Error::VideoFetch {
        url: "https://youtube.com/watch?v=abc".to_string(),
        reason: "geo-blocked".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("youtube.com"));
    assert!(display.contains("geo-blocked"));
}

#[test]
fn error_video_missing_field_display() {
    let err = Error::VideoMissingField {
        video_id: "abc123".to_string(),
        field: "title".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("abc123"));
    assert!(display.contains("title"));
}

#[test]
fn error_format_not_available_display() {
    let err = Error::FormatNotAvailable {
        video_id: "abc123".to_string(),
        format_type: FormatType::Audio,
        available_formats: vec!["mp4".to_string(), "webm".to_string()],
    };
    let display = format!("{}", err);
    assert!(display.contains("abc123"));
}

#[test]
fn error_format_no_url_display() {
    let err = Error::FormatNoUrl {
        video_id: "abc".to_string(),
        format_id: "251".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("251"));
    assert!(display.contains("no download URL"));
}

#[test]
fn error_format_incompatible_display() {
    let err = Error::FormatIncompatible {
        format_id: "251".to_string(),
        reason: "audio-only".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("251"));
    assert!(display.contains("audio-only"));
}

#[test]
fn error_no_thumbnail_display() {
    let err = Error::NoThumbnail {
        video_id: "abc".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("abc"));
    assert!(display.contains("thumbnail"));
}

#[test]
fn error_subtitle_not_available_display() {
    let err = Error::SubtitleNotAvailable {
        video_id: "abc".to_string(),
        language: "fr".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("abc"));
    assert!(display.contains("fr"));
}

#[test]
fn error_url_expired_display() {
    let err = Error::UrlExpired;
    assert_eq!(format!("{}", err), "URL expired");
}

#[test]
fn error_path_validation_display() {
    let err = Error::PathValidation {
        path: PathBuf::from("../../../etc/passwd"),
        reason: "path traversal detected".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("path traversal"));
}

#[test]
fn error_url_validation_display() {
    let err = Error::UrlValidation {
        url: "javascript:alert(1)".to_string(),
        reason: "unsafe scheme".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("javascript"));
    assert!(display.contains("unsafe scheme"));
}

#[test]
fn error_download_failed_display() {
    let err = Error::DownloadFailed {
        download_id: 42,
        reason: "connection reset".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("42"));
    assert!(display.contains("connection reset"));
}

#[test]
fn error_download_cancelled_display() {
    let err = Error::DownloadCancelled { download_id: 7 };
    let display = format!("{}", err);
    assert!(display.contains("7"));
    assert!(display.contains("cancelled"));
}

#[test]
fn error_invalid_partial_range_display() {
    let err = Error::InvalidPartialRange {
        reason: "start exceeds end".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("start exceeds end"));
}

#[test]
fn error_metadata_display() {
    let err = Error::Metadata {
        operation: "write".to_string(),
        path: PathBuf::from("/tmp/file.mp4"),
        reason: "unsupported format".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("write"));
    assert!(display.contains("unsupported format"));
}

#[test]
fn error_cache_miss_display() {
    let err = Error::CacheMiss {
        key: "video:abc123".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("video:abc123"));
}

#[test]
fn error_cache_expired_display() {
    let err = Error::CacheExpired {
        key: "video:abc123".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("expired"));
}

#[test]
fn error_checksum_mismatch_display() {
    let err = Error::ChecksumMismatch {
        path: PathBuf::from("/tmp/file.mp4"),
        expected: "abc123".to_string(),
        actual: "def456".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("abc123"));
    assert!(display.contains("def456"));
}

#[test]
fn error_invalid_header_display() {
    let err = Error::InvalidHeader {
        header: "Content-Type".to_string(),
        reason: "invalid value".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Content-Type"));
}

#[test]
fn error_unexpected_status_display() {
    let err = Error::UnexpectedStatus {
        status: 403,
        url: "https://example.com".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("403"));
}

#[test]
fn error_unknown_display() {
    let err = Error::Unknown("something went wrong".to_string());
    let display = format!("{}", err);
    assert!(display.contains("something went wrong"));
}

// ============================== Error Debug ==============================

#[test]
fn error_debug() {
    let err = Error::UrlExpired;
    let debug = format!("{:?}", err);
    assert!(debug.contains("UrlExpired"));
}

// ============================== Error constructors ==============================

#[test]
fn error_io_constructor() {
    let err = Error::io("test op", std::io::Error::other("test"));
    match err {
        Error::IO { operation, path, .. } => {
            assert_eq!(operation, "test op");
            assert!(path.is_none());
        }
        _ => panic!("Expected IO variant"),
    }
}

#[test]
fn error_io_with_path_constructor() {
    let err = Error::io_with_path("test op", "/tmp/test.txt", std::io::Error::other("test"));
    match err {
        Error::IO { operation, path, .. } => {
            assert_eq!(operation, "test op");
            assert_eq!(path, Some(PathBuf::from("/tmp/test.txt")));
        }
        _ => panic!("Expected IO variant"),
    }
}

// ============================== From impls ==============================

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: Error = io_err.into();
    matches!(err, Error::IO { .. });
}

#[test]
fn error_from_serde_json() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let err: Error = json_err.into();
    matches!(err, Error::Json { .. });
}

// ============================== NoBinaryRelease ==============================

#[test]
fn error_no_binary_release_display() {
    let err = Error::NoBinaryRelease {
        binary: "yt-dlp".to_string(),
        platform: yt_dlp::utils::platform::Platform::Mac,
        architecture: yt_dlp::utils::platform::Architecture::X64,
    };
    let display = format!("{}", err);
    assert!(display.contains("yt-dlp"));
}

#[test]
fn error_binary_not_found_display() {
    let err = Error::BinaryNotFound {
        binary: "ffmpeg".to_string(),
        path: PathBuf::from("/usr/local/bin/ffmpeg"),
    };
    let display = format!("{}", err);
    assert!(display.contains("ffmpeg"));
    assert!(display.contains("not found"));
}

// ============================== Error constructor: http ==============================

#[test]
fn error_http_constructor() {
    // Build a reqwest error by trying to parse an invalid URL
    let client = reqwest::Client::new();
    let req_err = client.get("http://[::ffff::127.0.0.1]").build().unwrap_err();
    let err = Error::http("https://example.com/video", "downloading video", req_err);
    match err {
        Error::Http {
            url, context, source, ..
        } => {
            assert_eq!(url, "https://example.com/video");
            assert_eq!(context, "downloading video");
            assert!(!source.to_string().is_empty());
        }
        _ => panic!("Expected Http variant"),
    }
}

// ============================== Error constructor: json ==============================

#[test]
fn error_json_constructor() {
    let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let err = Error::json("parsing video metadata", json_err);
    match err {
        Error::Json { context, source, .. } => {
            assert_eq!(context, "parsing video metadata");
            assert!(!source.to_string().is_empty());
        }
        _ => panic!("Expected Json variant"),
    }
}

// ============================== Error constructor: video_fetch ==============================

#[test]
fn error_video_fetch_constructor() {
    let err = Error::video_fetch("https://youtube.com/watch?v=abc", "geo-restricted");
    match err {
        Error::VideoFetch { url, reason } => {
            assert_eq!(url, "https://youtube.com/watch?v=abc");
            assert_eq!(reason, "geo-restricted");
        }
        _ => panic!("Expected VideoFetch variant"),
    }
}

// ============================== Error constructor: download_failed ==============================

#[test]
fn error_download_failed_constructor() {
    let err = Error::download_failed(42, "connection reset");
    match err {
        Error::DownloadFailed {
            download_id, reason, ..
        } => {
            assert_eq!(download_id, 42);
            assert_eq!(reason, "connection reset");
        }
        _ => panic!("Expected DownloadFailed variant"),
    }
}

// ============================== Error constructor: cache_miss ==============================

#[test]
fn error_cache_miss_constructor() {
    let err = Error::cache_miss("video:abc123");
    match err {
        Error::CacheMiss { key } => {
            assert_eq!(key, "video:abc123");
        }
        _ => panic!("Expected CacheMiss variant"),
    }
}

// ============================== Error constructor: cache_expired ==============================

#[test]
fn error_cache_expired_constructor() {
    let err = Error::cache_expired("video:abc123");
    match err {
        Error::CacheExpired { key } => {
            assert_eq!(key, "video:abc123");
        }
        _ => panic!("Expected CacheExpired variant"),
    }
}

// ============================== Error constructor: path_validation ==============================

#[test]
fn error_path_validation_constructor() {
    let err = Error::path_validation("/etc/passwd", "absolute path not allowed");
    match err {
        Error::PathValidation { path, reason } => {
            assert_eq!(path, PathBuf::from("/etc/passwd"));
            assert_eq!(reason, "absolute path not allowed");
        }
        _ => panic!("Expected PathValidation variant"),
    }
}

// ============================== Error constructor: url_validation ==============================

#[test]
fn error_url_validation_constructor() {
    let err = Error::url_validation("ftp://evil.com", "unsafe scheme");
    match err {
        Error::UrlValidation { url, reason } => {
            assert_eq!(url, "ftp://evil.com");
            assert_eq!(reason, "unsafe scheme");
        }
        _ => panic!("Expected UrlValidation variant"),
    }
}

// ============================== Error constructor: invalid_partial_range ==============================

#[test]
fn error_invalid_partial_range_constructor() {
    let err = Error::invalid_partial_range("start > end");
    match err {
        Error::InvalidPartialRange { reason } => {
            assert_eq!(reason, "start > end");
        }
        _ => panic!("Expected InvalidPartialRange variant"),
    }
}

// ============================== Error constructor: metadata ==============================

#[test]
fn error_metadata_constructor() {
    let err = Error::metadata("write tags", "/tmp/file.mp3", "unsupported format");
    match err {
        Error::Metadata {
            operation,
            path,
            reason,
        } => {
            assert_eq!(operation, "write tags");
            assert_eq!(path, PathBuf::from("/tmp/file.mp3"));
            assert_eq!(reason, "unsupported format");
        }
        _ => panic!("Expected Metadata variant"),
    }
}

// ============================== source() chaining ==============================

#[test]
fn error_io_source_chaining() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let err = Error::io("opening file", io_err);
    let source = err.source();
    assert!(source.is_some());
    assert!(source.unwrap().to_string().contains("access denied"));
}

#[test]
fn error_json_source_chaining() {
    let json_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
    let err = Error::json("parsing response", json_err);
    let source = err.source();
    assert!(source.is_some());
}

#[test]
fn error_from_io_source_chaining() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: Error = io_err.into();
    assert!(err.source().is_some());
}

// ============================== ArchiveError ==============================

#[test]
fn archive_error_invalid_format_display() {
    let err = ArchiveError::InvalidFormat;
    assert_eq!(format!("{}", err), "Invalid archive format");
}

#[test]
fn archive_error_corrupted_display() {
    let err = ArchiveError::Corrupted;
    assert_eq!(format!("{}", err), "Corrupted archive");
}

#[test]
fn archive_error_debug() {
    let err = ArchiveError::InvalidFormat;
    let debug = format!("{:?}", err);
    assert!(debug.contains("InvalidFormat"));
}

#[test]
fn error_archive_display() {
    let err = Error::Archive {
        file: "yt-dlp.zip".to_string(),
        source: ArchiveError::Corrupted,
    };
    let display = format!("{}", err);
    assert!(display.contains("yt-dlp.zip"));
    assert!(display.contains("Corrupted"));
}

#[test]
fn error_archive_source_chaining() {
    let err = Error::Archive {
        file: "test.zip".to_string(),
        source: ArchiveError::InvalidFormat,
    };
    let source = err.source();
    assert!(source.is_some());
    assert_eq!(source.unwrap().to_string(), "Invalid archive format");
}

// ============================== From<media_seek::Error> ==============================

#[test]
fn error_from_zip_error() {
    let err: Error = zip::result::ZipError::FileNotFound.into();
    match err {
        Error::Archive { file, source } => {
            assert_eq!(file, "unknown");
            matches!(source, ArchiveError::Zip(_));
        }
        _ => panic!("Expected Archive variant"),
    }
}
