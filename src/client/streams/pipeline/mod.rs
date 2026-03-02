//! Download pipeline steps.
//!
//! This module contains the sequential steps of the download pipeline:
//! fetching video info, downloading streams, combining audio+video,
//! partial/clip downloads, and playlist iteration.

pub mod combine;
pub mod download;
pub mod fetch;
pub mod partial;
pub mod playlist;
