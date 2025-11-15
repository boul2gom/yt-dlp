//! URL expiry detection and handling utilities.
//!
//! This module provides functionality to detect when URLs (for videos, audio, subtitles)
//! have expired and need to be refreshed from yt-dlp.

use crate::error::Error;
use reqwest::StatusCode;

/// Represents the result of a URL expiry check.
#[derive(Debug, Clone, PartialEq)]
pub enum UrlStatus {
    /// The URL is valid and accessible
    Valid,
    /// The URL has expired and needs to be refreshed
    Expired(ExpiredReason),
    /// The URL status could not be determined
    Unknown,
}

/// Reasons why a URL might be considered expired.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpiredReason {
    /// HTTP 403 Forbidden - typically means the temporary URL has expired
    Forbidden,
    /// HTTP 404 Not Found - resource no longer exists at this URL
    NotFound,
    /// HTTP 410 Gone - resource explicitly marked as permanently deleted
    Gone,
    /// HTTP 401 Unauthorized - authentication/authorization failed
    Unauthorized,
    /// Other expiry-related error
    Other(String),
}

impl UrlStatus {
    /// Checks if the URL is expired.
    pub fn is_expired(&self) -> bool {
        matches!(self, UrlStatus::Expired(_))
    }

    /// Gets the expiry reason if the URL is expired.
    pub fn expired_reason(&self) -> Option<&ExpiredReason> {
        match self {
            UrlStatus::Expired(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Check if an HTTP status code indicates an expired URL.
///
/// # Arguments
///
/// * `status` - The HTTP status code to check
///
/// # Returns
///
/// Returns `UrlStatus::Expired` if the status indicates expiry, `UrlStatus::Valid` otherwise
pub fn check_http_status(status: StatusCode) -> UrlStatus {
    match status {
        StatusCode::FORBIDDEN => UrlStatus::Expired(ExpiredReason::Forbidden),
        StatusCode::NOT_FOUND => UrlStatus::Expired(ExpiredReason::NotFound),
        StatusCode::GONE => UrlStatus::Expired(ExpiredReason::Gone),
        StatusCode::UNAUTHORIZED => UrlStatus::Expired(ExpiredReason::Unauthorized),
        _ if status.is_success() => UrlStatus::Valid,
        _ => UrlStatus::Unknown,
    }
}

/// Check if a reqwest error indicates an expired URL.
///
/// # Arguments
///
/// * `error` - The reqwest error to analyze
///
/// # Returns
///
/// Returns `UrlStatus::Expired` if the error indicates expiry
pub fn check_error(error: &reqwest::Error) -> UrlStatus {
    if let Some(status) = error.status() {
        check_http_status(status)
    } else {
        UrlStatus::Unknown
    }
}

/// Check if a crate Error indicates an expired URL.
///
/// # Arguments
///
/// * `error` - The Error to analyze
///
/// # Returns
///
/// Returns `UrlStatus::Expired` if the error indicates expiry
pub fn check_download_error(error: &Error) -> UrlStatus {
    match error {
        Error::Http { source, .. } => check_error(source),
        _ => UrlStatus::Unknown,
    }
}

/// Determine if a URL should be refreshed based on the error.
///
/// # Arguments
///
/// * `error` - The error that occurred during download/access
///
/// # Returns
///
/// Returns `true` if the URL should be refreshed from yt-dlp
pub fn should_refresh_url(error: &Error) -> bool {
    check_download_error(error).is_expired()
}

/// Determine if an HTTP error should trigger URL refresh.
///
/// # Arguments
///
/// * `error` - The reqwest error that occurred
///
/// # Returns
///
/// Returns `true` if the URL should be refreshed
pub fn should_refresh_url_from_http_error(error: &reqwest::Error) -> bool {
    check_error(error).is_expired()
}

/// Configuration for URL expiry handling.
#[derive(Debug, Clone)]
pub struct ExpiryConfig {
    /// Maximum number of refresh attempts before giving up
    pub max_refresh_attempts: usize,
    /// Whether to automatically refresh URLs when they expire
    pub auto_refresh: bool,
}

impl Default for ExpiryConfig {
    fn default() -> Self {
        Self {
            max_refresh_attempts: 2,
            auto_refresh: true,
        }
    }
}

impl ExpiryConfig {
    /// Creates a new expiry configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of refresh attempts.
    pub fn with_max_refresh_attempts(mut self, attempts: usize) -> Self {
        self.max_refresh_attempts = attempts;
        self
    }

    /// Sets whether to automatically refresh expired URLs.
    pub fn with_auto_refresh(mut self, enabled: bool) -> Self {
        self.auto_refresh = enabled;
        self
    }
}
