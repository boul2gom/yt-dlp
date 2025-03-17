//! Utility functions and types used throughout the application.
//!
//! This module contains various utility to interact with the shell, and the file system.

use crate::error::Result;
use platform::Platform;
use tokio::task::JoinHandle;

pub mod file_system;
pub mod platform;

/// Converts a vector of string slices to a vector of owned strings.
pub fn to_owned(vec: Vec<impl AsRef<str>>) -> Vec<String> {
    vec.into_iter().map(|s| s.as_ref().to_owned()).collect()
}

/// Find the name of the executable for the given platform.
pub fn find_executable(name: impl AsRef<str>) -> String {
    let platform = Platform::detect();

    match platform {
        Platform::Windows => format!("{}.exe", name.as_ref()),
        _ => name.as_ref().to_string(),
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
    #[cfg(feature = "tracing")]
    tracing::debug!("Awaiting two futures");

    let (first_result, second_result) = tokio::try_join!(first, second)?;

    let first = first_result?;
    let second = second_result?;

    Ok((first, second))
}

/// Awaits all futures and returns a vector of their results.
/// If any future returns an error, the error is propagated.
///
/// # Arguments
///
/// * `handles` - The futures to await.
pub async fn await_all<T, I>(handles: I) -> Result<Vec<T>>
where
    I: IntoIterator<Item = JoinHandle<Result<T>>> + std::fmt::Debug,
    T: Send + 'static,
{
    #[cfg(feature = "tracing")]
    tracing::debug!("Awaiting multiple futures");

    let results = futures_util::future::try_join_all(handles).await?;

    results.into_iter().collect()
}

/// A macro to mimic the ternary operator in Rust.
#[macro_export]
macro_rules! ternary {
    ($condition:expr, $true:expr, $false:expr) => {
        if $condition { $true } else { $false }
    };
}
