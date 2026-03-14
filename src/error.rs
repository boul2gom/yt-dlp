//! Error types with enhanced context and structured information.
//!
//! This module provides comprehensive error handling for the yt-dlp library,
//! with detailed context, error chaining, and structured error information.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

use crate::model::format::FormatType;
use crate::utils::platform::{Architecture, Platform};

/// A type alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// The possible errors that can occur in the yt-dlp library.
///
/// Each error variant provides detailed context about what went wrong,
/// including the operation being performed and any relevant parameters.
#[derive(Debug, Error)]
pub enum Error {
    // ==================== Runtime & System Errors ====================
    /// An async task failed to complete.
    ///
    /// This typically indicates a panic in a spawned tokio task or a cancellation.
    #[error("Async task failed: {context}")]
    Runtime {
        context: String,
        #[source]
        source: tokio::task::JoinError,
    },

    /// A file system operation failed.
    ///
    /// Includes the operation being performed and the path involved.
    #[error("IO error during {operation}")]
    IO {
        operation: String,
        path: Option<PathBuf>,
        #[source]
        source: std::io::Error,
    },

    /// An archive extraction operation failed.
    ///
    /// This occurs when extracting yt-dlp or ffmpeg archives.
    #[error("Failed to extract archive {file}: {source}")]
    Archive {
        file: String,
        #[source]
        source: ArchiveError,
    },

    // ==================== Network & HTTP Errors ====================
    /// An HTTP request failed.
    ///
    /// Includes the URL being accessed and the operation context.
    #[error("HTTP request failed for {url}: {context}")]
    Http {
        url: String,
        context: String,
        #[source]
        source: reqwest::Error,
    },

    /// A network timeout occurred.
    ///
    /// Indicates the operation and duration that was exceeded.
    #[error("Timeout after {duration:?} while {operation}")]
    Timeout { operation: String, duration: Duration },

    // ==================== Data & Serialization Errors ====================
    /// JSON parsing or serialization failed.
    ///
    /// Includes the context of what was being parsed/serialized.
    #[error("JSON error while {context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    /// Database operation failed (redb backend).
    ///
    /// Includes the specific operation that failed.
    #[cfg(feature = "cache-redb")]
    #[error("Database error during {operation}: {source}")]
    Database {
        operation: String,
        #[source]
        source: Box<redb::Error>,
    },

    /// Redis operation failed.
    ///
    /// Includes the specific operation that failed.
    #[cfg(feature = "cache-redis")]
    #[error("Redis error during {operation}: {source}")]
    Redis {
        operation: String,
        #[source]
        source: redis::RedisError,
    },

    // ==================== Dependency & Binary Errors ====================
    /// No GitHub release asset found for the current platform.
    ///
    /// This occurs when trying to download yt-dlp or ffmpeg binaries.
    #[error("No {binary} release found for {platform}/{architecture}")]
    NoBinaryRelease {
        binary: String,
        platform: Platform,
        architecture: Architecture,
    },

    /// The required binary was not found after installation.
    ///
    /// This indicates an installation issue or corrupted download.
    #[error("{binary} binary not found at {path} after installation")]
    BinaryNotFound { binary: String, path: PathBuf },

    /// Command execution failed.
    ///
    /// Includes the command, exit status, and stderr output.
    #[error("Command '{command}' failed (exit code: {exit_code}): {stderr}")]
    CommandFailed {
        command: String,
        exit_code: i32,
        stderr: String,
    },

    // ==================== Video & Format Errors ====================
    /// Failed to fetch video information from YouTube.
    ///
    /// Includes the URL and reason for failure.
    #[error("Failed to fetch video from {url}: {reason}")]
    VideoFetch { url: String, reason: String },

    /// Video information is missing expected data.
    ///
    /// This occurs when YouTube's API returns incomplete data.
    #[error("Video {video_id} is missing required field: {field}")]
    VideoMissingField { video_id: String, field: String },

    /// The requested format is not available.
    ///
    /// Includes the format type and available alternatives.
    #[error("No {format_type} format available for video {video_id}")]
    FormatNotAvailable {
        video_id: String,
        format_type: FormatType,
        available_formats: Vec<String>,
    },

    /// The format has no URL available for download.
    ///
    /// This can occur with DRM-protected or geo-restricted content.
    #[error("Format {format_id} for video {video_id} has no download URL")]
    FormatNoUrl { video_id: String, format_id: String },

    /// The format is incompatible with the requested operation.
    ///
    /// For example, trying to extract audio from a video-only format.
    #[error("Format {format_id} is incompatible: {reason}")]
    FormatIncompatible { format_id: String, reason: String },

    /// No thumbnail is available for the video.
    #[error("No thumbnail available for video {video_id}")]
    NoThumbnail { video_id: String },

    /// No subtitles are available for the requested language.
    #[error("No subtitles available for video {video_id} in language '{language}'")]
    SubtitleNotAvailable { video_id: String, language: String },

    /// The URL has expired and needs to be refreshed.
    #[error("URL expired")]
    UrlExpired,

    // ==================== Path & Security Errors ====================
    /// Path validation failed due to security concerns.
    ///
    /// This prevents path traversal and other security issues.
    #[error("Invalid path '{path}': {reason}")]
    PathValidation { path: PathBuf, reason: String },

    /// URL validation failed.
    ///
    /// This ensures only valid YouTube URLs are processed.
    #[error("Invalid URL '{url}': {reason}")]
    UrlValidation { url: String, reason: String },

    // ==================== Download Errors ====================
    /// Download operation failed.
    ///
    /// Includes the download ID and reason for failure.
    #[error("Download {download_id} failed: {reason}")]
    DownloadFailed { download_id: u64, reason: String },

    /// Download was cancelled by user or system.
    #[error("Download {download_id} was cancelled")]
    DownloadCancelled { download_id: u64 },

    /// A partial download range is invalid or the container format does not support seeking.
    #[error("Invalid partial range: {reason}")]
    InvalidPartialRange { reason: String },

    // ==================== Live Stream Errors ====================
    /// The video is not currently a live stream.
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    #[error("Video at {url} is not live (status={live_status}): {reason}")]
    LiveStreamUnavailable {
        url: String,
        live_status: String,
        reason: String,
    },

    /// Failed to parse an HLS manifest.
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    #[error("HLS parsing failed for {url}: {context}")]
    HlsParsing { url: String, context: String },

    /// A live recording operation failed.
    #[cfg(feature = "live-recording")]
    #[error("Live recording failed for {url}: {reason}")]
    LiveRecording { url: String, reason: String },

    /// A live streaming operation failed.
    #[cfg(feature = "live-streaming")]
    #[error("Live streaming failed for {url}: {reason}")]
    LiveStreaming { url: String, reason: String },

    // ==================== Metadata Errors ====================
    /// A metadata tagging operation failed.
    ///
    /// This occurs when reading or writing audio/video tags (ID3, MP4, lofty).
    #[error("Metadata {operation} failed for {path}: {reason}")]
    Metadata {
        operation: String,
        path: PathBuf,
        reason: String,
    },

    // ==================== Cache Errors ====================
    /// The requested item was not found in the cache.
    #[error("Cache miss for {key}")]
    CacheMiss { key: String },

    /// The cached item has expired.
    #[error("Cache entry expired for {key}")]
    CacheExpired { key: String },

    /// Multiple persistent cache backends are compiled in but none was selected.
    ///
    /// Set `CacheConfig::persistent_backend` explicitly when more than one of
    /// `cache-json`, `cache-redb`, or `cache-redis` features are active.
    #[cfg(persistent_cache)]
    #[error(
        "ambiguous persistent cache backend: {count} backends compiled in, set `persistent_backend` in CacheConfig"
    )]
    AmbiguousCacheBackend { count: usize },

    /// A checksum verification failed after downloading.
    #[error("Checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    /// An HTTP header value was invalid.
    #[error("Invalid header '{header}': {reason}")]
    InvalidHeader { header: String, reason: String },

    /// An HTTP response status was unexpected.
    #[error("Unexpected HTTP status {status} for {url}")]
    UnexpectedStatus { status: u16, url: String },

    // ==================== Generic Errors ====================
    /// An unexpected error occurred that doesn't fit other categories.
    ///
    /// This should be used sparingly and ideally replaced with more specific variants.
    #[error("Unexpected error: {0}")]
    Unknown(String),
}

/// Archive extraction errors.
#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("ZIP extraction error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Invalid archive format")]
    InvalidFormat,

    #[error("Corrupted archive")]
    Corrupted,
}

// ==================== Helper constructors for common error patterns ====================

impl Error {
    /// Create an IO error with operation context.
    ///
    /// # Arguments
    ///
    /// * `operation` - Description of the operation that failed
    /// * `source` - The underlying IO error
    ///
    /// # Returns
    ///
    /// An Error::IO variant with the provided context
    pub fn io(operation: impl Into<String>, source: std::io::Error) -> Self {
        let operation_str = operation.into();

        tracing::warn!(
            operation = operation_str,
            error = %source,
            "⚙️ IO error occurred"
        );

        Self::IO {
            operation: operation_str,
            path: None,
            source,
        }
    }

    /// Create an IO error with operation and path context.
    ///
    /// # Arguments
    ///
    /// * `operation` - Description of the operation that failed
    /// * `path` - The file path involved in the operation
    /// * `source` - The underlying IO error
    ///
    /// # Returns
    ///
    /// An Error::IO variant with the provided context and path
    pub fn io_with_path(operation: impl Into<String>, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let operation_str = operation.into();
        let path_buf = path.into();

        tracing::warn!(
            operation = operation_str,
            path = ?path_buf,
            error = %source,
            "⚙️ IO error occurred with path"
        );

        Self::IO {
            operation: operation_str,
            path: Some(path_buf),
            source,
        }
    }

    /// Create an HTTP error with URL context.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL that was being accessed
    /// * `context` - Additional context about the operation
    /// * `source` - The underlying reqwest error
    ///
    /// # Returns
    ///
    /// An Error::Http variant with the provided context
    pub fn http(url: impl Into<String>, context: impl Into<String>, source: reqwest::Error) -> Self {
        let url_str = url.into();
        let context_str = context.into();

        tracing::warn!(
            url = url_str,
            context = context_str,
            error = %source,
            is_timeout = source.is_timeout(),
            is_connect = source.is_connect(),
            status = ?source.status(),
            "⚙️ HTTP error occurred"
        );

        Self::Http {
            url: url_str,
            context: context_str,
            source,
        }
    }

    /// Create a JSON parsing error with context.
    ///
    /// # Arguments
    ///
    /// * `context` - Description of what was being parsed/serialized
    /// * `source` - The underlying serde_json error
    ///
    /// # Returns
    ///
    /// An Error::Json variant with the provided context
    pub fn json(context: impl Into<String>, source: serde_json::Error) -> Self {
        let context_str = context.into();

        tracing::warn!(
            context = context_str,
            error = %source,
            line = source.line(),
            column = source.column(),
            "⚙️ JSON error occurred"
        );

        Self::Json {
            context: context_str,
            source,
        }
    }

    /// Create a database error with operation context (redb).
    ///
    /// # Arguments
    ///
    /// * `operation` - Description of the database operation that failed
    /// * `source` - The underlying redb error
    ///
    /// # Returns
    ///
    /// An Error::Database variant with the provided context
    #[cfg(feature = "cache-redb")]
    pub fn database(operation: impl Into<String>, source: impl Into<redb::Error>) -> Self {
        let operation_str = operation.into();
        let source = source.into();

        tracing::warn!(
            operation = operation_str,
            error = %source,
            "⚙️ Database error occurred"
        );

        Self::Database {
            operation: operation_str,
            source: Box::new(source),
        }
    }

    /// Create a Redis error with operation context.
    ///
    /// # Arguments
    ///
    /// * `operation` - Description of the Redis operation that failed
    /// * `source` - The underlying Redis error
    ///
    /// # Returns
    ///
    /// An Error::Redis variant with the provided context
    #[cfg(feature = "cache-redis")]
    pub fn redis(operation: impl Into<String>, source: redis::RedisError) -> Self {
        let operation_str = operation.into();

        tracing::warn!(
            operation = operation_str,
            error = %source,
            "⚙️ Redis error occurred"
        );

        Self::Redis {
            operation: operation_str,
            source,
        }
    }

    /// Create a runtime error with context.
    ///
    /// # Arguments
    ///
    /// * `context` - Description of the task that failed
    /// * `source` - The underlying tokio JoinError
    ///
    /// # Returns
    ///
    /// An Error::Runtime variant with the provided context
    pub fn runtime(context: impl Into<String>, source: tokio::task::JoinError) -> Self {
        let context_str = context.into();

        tracing::error!(
            context = context_str,
            error = %source,
            is_cancelled = source.is_cancelled(),
            is_panic = source.is_panic(),
            "Runtime task error occurred"
        );

        Self::Runtime {
            context: context_str,
            source,
        }
    }

    /// Create a video fetch error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL that failed to fetch
    /// * `reason` - The reason for the fetch failure
    ///
    /// # Returns
    ///
    /// An Error::VideoFetch variant with the provided details
    pub fn video_fetch(url: impl Into<String>, reason: impl Into<String>) -> Self {
        let url_str = url.into();
        let reason_str = reason.into();

        tracing::warn!(url = url_str, reason = reason_str, "Video fetch failed");

        Self::VideoFetch {
            url: url_str,
            reason: reason_str,
        }
    }

    /// Create a path validation error.
    ///
    /// # Arguments
    ///
    /// * `path` - The path that failed validation
    /// * `reason` - The reason for validation failure
    ///
    /// # Returns
    ///
    /// An Error::PathValidation variant with the provided details
    pub fn path_validation(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        let path_buf = path.into();
        let reason_str = reason.into();

        tracing::warn!(
            path = ?path_buf,
            reason = reason_str,
            "⚙️ Path validation failed"
        );

        Self::PathValidation {
            path: path_buf,
            reason: reason_str,
        }
    }

    /// Create a URL validation error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL that failed validation
    /// * `reason` - The reason for validation failure
    ///
    /// # Returns
    ///
    /// An Error::UrlValidation variant with the provided details
    pub fn url_validation(url: impl Into<String>, reason: impl Into<String>) -> Self {
        let url_str = url.into();
        let reason_str = reason.into();

        tracing::warn!(url = url_str, reason = reason_str, "URL validation failed");

        Self::UrlValidation {
            url: url_str,
            reason: reason_str,
        }
    }

    /// Create an invalid partial range error.
    ///
    /// # Arguments
    ///
    /// * `reason` - Description of why the partial range is invalid
    ///
    /// # Returns
    ///
    /// An Error::InvalidPartialRange variant with the provided reason
    pub fn invalid_partial_range(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        tracing::warn!(reason = %reason, "Invalid partial range");
        Self::InvalidPartialRange { reason }
    }

    /// Create a download failed error.
    ///
    /// # Arguments
    ///
    /// * `download_id` - The ID of the download that failed
    /// * `reason` - The reason for the download failure
    ///
    /// # Returns
    ///
    /// An Error::DownloadFailed variant with the provided details
    pub fn download_failed(download_id: u64, reason: impl Into<String>) -> Self {
        let reason_str = reason.into();

        tracing::error!(download_id = download_id, reason = reason_str, "Download failed");

        Self::DownloadFailed {
            download_id,
            reason: reason_str,
        }
    }

    /// Create a live stream unavailable error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video
    /// * `live_status` - The current live status of the video
    /// * `reason` - Why the stream is not available
    ///
    /// # Returns
    ///
    /// An Error::LiveStreamUnavailable variant with the provided details
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    pub fn live_unavailable(url: impl Into<String>, live_status: impl Into<String>, reason: impl Into<String>) -> Self {
        let url_str = url.into();
        let live_status_str = live_status.into();
        let reason_str = reason.into();

        tracing::warn!(
            url = url_str,
            live_status = live_status_str,
            reason = reason_str,
            "📡 Live stream unavailable"
        );

        Self::LiveStreamUnavailable {
            url: url_str,
            live_status: live_status_str,
            reason: reason_str,
        }
    }

    /// Create an HLS parsing error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the manifest that failed to parse
    /// * `context` - Description of the parsing failure
    ///
    /// # Returns
    ///
    /// An Error::HlsParsing variant with the provided details
    #[cfg(any(feature = "live-recording", feature = "live-streaming"))]
    pub fn hls_parsing(url: impl Into<String>, context: impl Into<String>) -> Self {
        let url_str = url.into();
        let context_str = context.into();

        tracing::warn!(url = url_str, context = context_str, "HLS parsing failed");

        Self::HlsParsing {
            url: url_str,
            context: context_str,
        }
    }

    /// Create a live recording error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the live stream
    /// * `reason` - Why the recording failed
    ///
    /// # Returns
    ///
    /// An Error::LiveRecording variant with the provided details
    #[cfg(feature = "live-recording")]
    pub fn live_recording(url: impl Into<String>, reason: impl Into<String>) -> Self {
        let url_str = url.into();
        let reason_str = reason.into();

        tracing::error!(url = url_str, reason = reason_str, "Live recording failed");

        Self::LiveRecording {
            url: url_str,
            reason: reason_str,
        }
    }

    /// Create a live streaming error.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the live stream
    /// * `reason` - Why the streaming failed
    ///
    /// # Returns
    ///
    /// An Error::LiveStreaming variant with the provided details
    #[cfg(feature = "live-streaming")]
    pub fn live_streaming(url: impl Into<String>, reason: impl Into<String>) -> Self {
        let url_str = url.into();
        let reason_str = reason.into();

        tracing::error!(url = url_str, reason = reason_str, "Live streaming failed");

        Self::LiveStreaming {
            url: url_str,
            reason: reason_str,
        }
    }

    /// Create an error for a failed live segment fetch.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the live segment
    /// * `status` - The HTTP status code returned
    ///
    /// # Returns
    ///
    /// An Error::LiveStreaming variant with the provided details
    #[cfg(feature = "live-streaming")]
    pub fn live_segment_fetch_failed(url: &str, status: reqwest::StatusCode) -> Self {
        let reason = format!("{} {}", SEGMENT_FETCH_ERROR_PREFIX, status);
        Self::live_streaming(url, reason)
    }

    /// Create a metadata error with operation and path context.
    ///
    /// # Arguments
    ///
    /// * `operation` - Description of the metadata operation (e.g. "read MP4 tags")
    /// * `path` - The file path involved
    /// * `reason` - The reason for the failure
    ///
    /// # Returns
    ///
    /// An Error::Metadata variant with the provided context
    pub fn metadata(operation: impl Into<String>, path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        let operation_str = operation.into();
        let path_buf = path.into();
        let reason_str = reason.into();

        tracing::warn!(
            operation = operation_str,
            path = ?path_buf,
            reason = reason_str,
            "🏷️ Metadata operation failed"
        );

        Self::Metadata {
            operation: operation_str,
            path: path_buf,
            reason: reason_str,
        }
    }

    /// Create a cache miss error.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key that was not found
    ///
    /// # Returns
    ///
    /// An Error::CacheMiss variant
    pub fn cache_miss(key: impl Into<String>) -> Self {
        let key_str = key.into();
        tracing::debug!(key = key_str, "🔍 Cache miss");
        Self::CacheMiss { key: key_str }
    }

    /// Create a cache expired error.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key that expired
    ///
    /// # Returns
    ///
    /// An Error::CacheExpired variant
    pub fn cache_expired(key: impl Into<String>) -> Self {
        let key_str = key.into();
        tracing::debug!(key = key_str, "🔍 Cache entry expired");
        Self::CacheExpired { key: key_str }
    }

    /// Create an ambiguous cache backend error.
    ///
    /// # Arguments
    ///
    /// * `count` - The number of persistent backends compiled in
    ///
    /// # Returns
    ///
    /// An Error::AmbiguousCacheBackend variant
    #[cfg(persistent_cache)]
    pub fn ambiguous_cache_backend(count: usize) -> Self {
        tracing::error!(count, "🔍 Ambiguous persistent cache backend");
        Self::AmbiguousCacheBackend { count }
    }
}

// ==================== Automatic conversions for convenience ====================

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        tracing::error!(
            error = %err,
            is_cancelled = err.is_cancelled(),
            is_panic = err.is_panic(),
            "Task execution failed (automatic conversion)"
        );

        Self::Runtime {
            context: "Task execution".to_string(),
            source: err,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        tracing::warn!(
            error = %err,
            kind = ?err.kind(),
            "⚙️ IO error (automatic conversion)"
        );

        Self::IO {
            operation: "File operation".to_string(),
            path: None,
            source: err,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        let url = err.url().map(|u| u.to_string()).unwrap_or_default();

        tracing::warn!(
            url = url,
            error = %err,
            is_timeout = err.is_timeout(),
            is_connect = err.is_connect(),
            status = ?err.status(),
            "⚙️ HTTP error (automatic conversion)"
        );

        Self::Http {
            url,
            context: "HTTP request".to_string(),
            source: err,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        tracing::warn!(
            error = %err,
            line = err.line(),
            column = err.column(),
            "⚙️ JSON error (automatic conversion)"
        );

        Self::Json {
            context: "JSON parsing".to_string(),
            source: err,
        }
    }
}

#[cfg(feature = "cache-redb")]
impl From<redb::Error> for Error {
    fn from(err: redb::Error) -> Self {
        tracing::warn!(
            error = %err,
            "⚙️ Database error (automatic conversion)"
        );

        Self::Database {
            operation: "Database operation".to_string(),
            source: Box::new(err),
        }
    }
}

#[cfg(feature = "cache-redis")]
impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        tracing::warn!(
            error = %err,
            "⚙️ Redis error (automatic conversion)"
        );

        Self::Redis {
            operation: "Redis operation".to_string(),
            source: err,
        }
    }
}

impl From<media_seek::Error> for Error {
    fn from(err: media_seek::Error) -> Self {
        tracing::warn!(
            error = %err,
            "⚙️ media-seek error (automatic conversion)"
        );

        Self::InvalidPartialRange {
            reason: err.to_string(),
        }
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        tracing::warn!(
            error = %err,
            "⚙️ ZIP archive error (automatic conversion)"
        );

        Self::Archive {
            file: "unknown".to_string(),
            source: ArchiveError::Zip(err),
        }
    }
}
/// Error context prefix for failed segment fetches.
#[cfg(feature = "live-streaming")]
const SEGMENT_FETCH_ERROR_PREFIX: &str = "segment fetch returned HTTP";
