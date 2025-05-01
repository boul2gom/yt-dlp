//! The errors that can occur.

use crate::utils::platform::{Architecture, Platform};
use std::time::Duration;
use thiserror::Error;

/// A type alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// The possible errors that can occur.
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred while running the runtime.
    #[error("An error occurred while running the runtime: {0}")]
    Runtime(#[from] tokio::task::JoinError),
    /// An error occurred while interacting with the file system.
    #[error("An IO error occurred: {0}")]
    IO(#[from] std::io::Error),
    /// An error occurred while zipping or unzipping a file.
    #[error("An error occurred while extracting the archive: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// An error occurred while fetching a file.
    #[error("An error occurred while fetching: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// An error occurred while parsing JSON.
    #[error("An error occurred while parsing JSON: {0}")]
    Serde(#[from] serde_json::Error),
    /// An error occurred while interacting with the SQLite database.
    #[error("An error occurred while interacting with the database: {0}")]
    #[cfg(feature = "cache")]
    Database(#[from] rusqlite::Error),

    /// An error occurred while interacting with GitHub.
    #[error("No GitHub asset found for platform {0}/{1}")]
    Github(Platform, Architecture),
    /// An error occurred while interacting with ffmpeg.
    #[error("No ffmpeg binary found for platform {0}/{1}")]
    Binary(Platform, Architecture),
    /// An error occurred while running a command.
    #[error("Failed to execute command: {0}")]
    Command(String),
    /// An error occurred while fetching a video.
    #[error("Failed to fetch video: {0}")]
    Video(String),
    /// An error occurred manipulating a path.
    #[error("An invalid path was provided: {0}")]
    Path(String),
    /// An error occurred due to a timeout.
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),
    /// An error occurred due to missing URL in format.
    #[error("Format {0} has no URL available")]
    MissingUrl(String),
    /// An error occurred due to missing format.
    #[error("No {0} format available for video")]
    MissingFormat(String),
    /// An error occurred due to incompatible format.
    #[error("Format {0} is not compatible: {1}")]
    IncompatibleFormat(String, String),
    /// An error occurred due to missing thumbnail.
    #[error("No thumbnail available for video")]
    MissingThumbnail,

    /// An error occurred due to missing format.
    #[error("Not found: {0}")]
    FormatNotFound(String),

    /// An unknown error occurred.
    #[error("An unknown error occurred: {0}")]
    Unknown(String),
}
