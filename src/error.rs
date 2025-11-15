//! Error types with enhanced context and structured information.
//!
//! This module provides comprehensive error handling for the yt-dlp library,
//! with detailed context, error chaining, and structured error information.

use crate::utils::platform::{Architecture, Platform};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

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
    Timeout {
        operation: String,
        duration: Duration,
    },

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

    /// Database operation failed.
    ///
    /// Includes the specific operation and table/query context.
    #[cfg(feature = "cache")]
    #[error("Database error during {operation}: {source}")]
    Database {
        operation: String,
        #[source]
        source: sqlx::Error,
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
        format_type: String,
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
    pub fn io(operation: impl Into<String>, source: std::io::Error) -> Self {
        Self::IO {
            operation: operation.into(),
            path: None,
            source,
        }
    }

    /// Create an IO error with operation and path context.
    pub fn io_with_path(
        operation: impl Into<String>,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::IO {
            operation: operation.into(),
            path: Some(path.into()),
            source,
        }
    }

    /// Create an HTTP error with URL context.
    pub fn http(
        url: impl Into<String>,
        context: impl Into<String>,
        source: reqwest::Error,
    ) -> Self {
        Self::Http {
            url: url.into(),
            context: context.into(),
            source,
        }
    }

    /// Create a JSON parsing error with context.
    pub fn json(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json {
            context: context.into(),
            source,
        }
    }

    /// Create a database error with operation context.
    #[cfg(feature = "cache")]
    pub fn database(operation: impl Into<String>, source: sqlx::Error) -> Self {
        Self::Database {
            operation: operation.into(),
            source,
        }
    }

    /// Create a runtime error with context.
    pub fn runtime(context: impl Into<String>, source: tokio::task::JoinError) -> Self {
        Self::Runtime {
            context: context.into(),
            source,
        }
    }

    /// Create a video fetch error.
    pub fn video_fetch(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::VideoFetch {
            url: url.into(),
            reason: reason.into(),
        }
    }

    /// Create a path validation error.
    pub fn path_validation(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Self::PathValidation {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Create a URL validation error.
    pub fn url_validation(url: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::UrlValidation {
            url: url.into(),
            reason: reason.into(),
        }
    }

    /// Create a download failed error.
    pub fn download_failed(download_id: u64, reason: impl Into<String>) -> Self {
        Self::DownloadFailed {
            download_id,
            reason: reason.into(),
        }
    }
}

// ==================== Automatic conversions for convenience ====================

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::Runtime {
            context: "Task execution".to_string(),
            source: err,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
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
        Self::Http {
            url,
            context: "HTTP request".to_string(),
            source: err,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json {
            context: "JSON parsing".to_string(),
            source: err,
        }
    }
}

#[cfg(feature = "cache")]
impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Self::Database {
            operation: "Database operation".to_string(),
            source: err,
        }
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Self::Archive {
            file: "unknown".to_string(),
            source: ArchiveError::Zip(err),
        }
    }
}
