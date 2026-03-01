//! URL and path validation utilities for security.
//!
//! This module provides functions to validate YouTube URLs and sanitize file paths
//! to prevent security vulnerabilities like path traversal attacks.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::extractor::Youtube;

/// Validates a YouTube URL.
///
/// # Arguments
///
/// * `url` - The URL to validate
///
/// # Returns
///
/// Returns `Ok(())` if the URL is a valid YouTube URL, otherwise an error.
///
/// # Errors
///
/// This function will return an error if:
/// - The URL cannot be parsed
/// - The URL is not from YouTube (youtube.com, youtu.be, or youtube-nocookie.com)
/// - The URL uses an unsafe scheme (only HTTP and HTTPS are allowed)
///
/// # Examples
///
/// ```rust
/// # use yt_dlp::utils::validation::validate_youtube_url;
/// // Valid URLs
/// assert!(validate_youtube_url("https://www.youtube.com/watch?v=gXtp6C-3JKo").is_ok());
/// assert!(validate_youtube_url("https://youtu.be/gXtp6C-3JKo").is_ok());
///
/// // Invalid URLs
/// assert!(validate_youtube_url("https://evil.com/watch?v=gXtp6C-3JKo").is_err());
/// assert!(validate_youtube_url("file:///etc/passwd").is_err());
/// ```
pub fn validate_youtube_url(url: &str) -> Result<()> {
    tracing::debug!(url = url, "⚙️ Validating YouTube URL");

    // Try to parse the URL
    let parsed = url::Url::parse(url).map_err(|e| Error::url_validation(url, format!("Invalid URL format: {}", e)))?;

    // Check the scheme (only HTTP and HTTPS are allowed)
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(Error::url_validation(
            url,
            format!("Unsafe URL scheme '{}'. Only HTTP and HTTPS are allowed", scheme),
        ));
    }

    // Check the host
    let host = parsed
        .host_str()
        .ok_or_else(|| Error::url_validation(url, "URL must have a host"))?;

    // Allow YouTube domains
    let is_youtube = Youtube::supports_url(url);

    if !is_youtube {
        tracing::warn!(url = url, host = host, "URL validation failed: not a YouTube domain");
        return Err(Error::url_validation(
            url,
            format!("URL must be from YouTube (got: {})", host),
        ));
    }

    tracing::debug!(url = url, host = host, "✅ YouTube URL validated successfully");

    Ok(())
}

/// Sanitizes a file path to prevent path traversal attacks.
///
/// # Arguments
///
/// * `path` - The path to sanitize
///
/// # Returns
///
/// Returns a sanitized version of the path with dangerous components removed.
///
/// # Errors
///
/// This function will return an error if the path contains path traversal attempts.
///
/// # Examples
///
/// ```rust
/// # use std::path::PathBuf;
/// # use yt_dlp::utils::validation::sanitize_path;
/// // Safe paths
/// assert!(sanitize_path("video.mp4").is_ok());
/// assert!(sanitize_path("downloads/video.mp4").is_ok());
///
/// // Dangerous paths (will be rejected or sanitized)
/// assert!(sanitize_path("../../../etc/passwd").is_err());
/// assert!(sanitize_path("/etc/passwd").is_err());
/// ```
pub fn sanitize_path(path: impl Into<PathBuf>) -> Result<PathBuf> {
    let path = path.into();

    tracing::debug!(
        path = ?path,
        "⚙️ Sanitizing file path"
    );

    // Check for absolute paths (not allowed for user-provided paths)
    if path.is_absolute() {
        return Err(Error::path_validation(path, "Absolute paths are not allowed"));
    }

    // Build a sanitized path by filtering out dangerous components
    let mut sanitized = PathBuf::new();
    let mut has_parent_ref = false;

    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => {
                // Check for hidden directory traversal in filenames
                let part_str = part.to_string_lossy();
                if part_str.contains("..") {
                    let msg = format!("Path contains suspicious component: {}", part_str);
                    return Err(Error::path_validation(path, msg));
                }
                sanitized.push(part);
            }
            std::path::Component::ParentDir => {
                has_parent_ref = true;
            }
            std::path::Component::CurDir => {
                // Skip current directory references (harmless but unnecessary)
            }
            std::path::Component::RootDir => {
                return Err(Error::path_validation(path, "Root directory reference in path"));
            }
            std::path::Component::Prefix(_) => {
                return Err(Error::path_validation(path, "Windows path prefix not allowed"));
            }
        }
    }

    // Reject paths with parent directory references
    if has_parent_ref {
        let msg = format!("Path traversal detected (..): {}", path.display());
        return Err(Error::path_validation(path, msg));
    }

    // Ensure the sanitized path is not empty
    if sanitized.as_os_str().is_empty() {
        return Err(Error::path_validation(path, "Empty path after sanitization"));
    }

    tracing::debug!(
        original_path = ?path,
        sanitized_path = ?sanitized,
        "✅ Path sanitized successfully"
    );

    Ok(sanitized)
}

/// Sanitizes a filename by removing or replacing unsafe characters.
///
/// # Arguments
///
/// * `filename` - The filename to sanitize
///
/// # Returns
///
/// Returns a sanitized filename with unsafe characters removed or replaced.
///
/// # Examples
///
/// ```rust
/// # use yt_dlp::utils::validation::sanitize_filename;
/// assert_eq!(sanitize_filename("video.mp4"), "video.mp4");
/// assert_eq!(sanitize_filename("my/video\\file.mp4"), "myvideofile.mp4");
/// assert_eq!(sanitize_filename("file:name.mp4"), "filename.mp4");
/// ```
pub fn sanitize_filename(filename: &str) -> String {
    tracing::debug!(filename = filename, "⚙️ Sanitizing filename");

    // Single-pass: remove forbidden chars, control chars, and consecutive dots (..)
    let mut result = String::with_capacity(filename.len());
    let mut prev_dot = false;
    for c in filename.chars() {
        if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
            prev_dot = false;
            continue;
        }
        if c == '.' && prev_dot {
            result.pop();
            prev_dot = false;
            continue;
        }
        prev_dot = c == '.';
        result.push(c);
    }

    // Compute trimmed length up front so `result` is not borrowed when potentially moved
    let trimmed_len = result.trim().len();
    let result = if trimmed_len == 0 {
        "download".to_string()
    } else if trimmed_len == result.len() {
        result
    } else {
        result.trim().to_string()
    };

    tracing::debug!(
        original_filename = filename,
        sanitized_filename = %result,
        "✅ Filename sanitized"
    );

    result
}
