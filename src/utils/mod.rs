//! Utility functions and types used throughout the application.
//!
//! This module contains various utilities for file system operations,
//! HTTP connections, retry logic, and validation.

use std::time::{SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;

use crate::error::Result;

pub mod fs;
pub mod http;
pub mod network;
pub mod platform;
pub mod subtitle;
pub mod validation;

// Re-export for backward compatibility
// Re-export commonly used functions from fs
pub use fs::*;
pub use network::{retry, url_expiry};
pub use platform::Platform;
pub use subtitle::subtitle_converter::convert_subtitle;
pub use subtitle::subtitle_validator::{ValidationResult, is_format_compatible, validate_subtitle};
pub use url_expiry::{ExpiryConfig, UrlStatus, check_download_error, should_refresh_url};

/// Converts a vector of string slices to a vector of owned strings.
///
/// # Arguments
///
/// * `vec` - The vector of string references to convert
///
/// # Returns
///
/// A vector of owned strings
pub fn to_owned(vec: Vec<impl AsRef<str>>) -> Vec<String> {
    vec.into_iter().map(|s| s.as_ref().to_owned()).collect()
}

/// Find the name of the executable for the given platform.
///
/// # Arguments
///
/// * `name` - The base name of the executable
///
/// # Returns
///
/// The platform-specific executable name (with .exe extension on Windows)
pub fn find_executable(name: impl AsRef<str>) -> String {
    let platform = Platform::detect();
    let name_str = name.as_ref();

    match platform {
        Platform::Windows => format!("{}.exe", name_str),
        _ => name_str.to_string(),
    }
}

/// Awaits two futures and returns a tuple of their results.
/// If either future returns an error, the error is propagated.
///
/// # Arguments
///
/// * `first` - The first future to await.
/// * `second` - The second future to await.
pub async fn await_two<T: std::fmt::Debug>(
    first: JoinHandle<Result<T>>,
    second: JoinHandle<Result<T>>,
) -> Result<(T, T)> {
    tracing::debug!("⚙️ Awaiting two futures");

    let (first_result, second_result) = tokio::try_join!(first, second)?;

    let first = first_result?;
    let second = second_result?;

    tracing::debug!("✅ Both futures completed successfully");

    Ok((first, second))
}

/// Awaits all futures and returns a vector of their results.
/// If any future returns an error, the error is propagated.
///
/// # Arguments
///
/// * `handles` - The futures to await
///
/// # Returns
///
/// A vector containing all the results
///
/// # Errors
///
/// Returns an error if any future fails
pub async fn await_all<T, I>(handles: I) -> Result<Vec<T>>
where
    I: IntoIterator<Item = JoinHandle<Result<T>>> + std::fmt::Debug,
    T: Send + 'static,
{
    tracing::debug!("⚙️ Awaiting multiple futures");

    let results = futures_util::future::try_join_all(handles).await?;

    let result_vec: Result<Vec<T>> = results.into_iter().collect();

    if let Ok(ref vec) = result_vec {
        tracing::debug!(completed_count = vec.len(), "✅ All futures completed successfully");
    }

    result_vec
}

/// Returns the current timestamp in seconds since UNIX epoch.
///
/// # Returns
///
/// Unix timestamp in seconds as i64.
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Checks if a timestamp is expired given a TTL.
///
/// # Arguments
///
/// * `cached_at` - The timestamp when the item was cached (Unix timestamp in seconds)
/// * `ttl` - Time-to-live in seconds
///
/// # Returns
///
/// `true` if the cached item has expired, `false` otherwise.
pub fn is_expired(cached_at: i64, ttl: u64) -> bool {
    let now = current_timestamp();
    (now - cached_at) > ttl as i64
}
