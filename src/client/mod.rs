//! Downloader client module.
//!
//! This module provides the main Downloader client struct and related configuration types.

pub mod builder;
pub mod deps;
pub mod download_builder;
mod pipeline;
pub mod proxy;
mod stream_downloads;
pub mod streams;

pub use builder::DownloaderBuilder;
pub use deps::{Libraries, LibraryInstaller};
pub use download_builder::DownloadBuilder;
pub use proxy::{ProxyConfig, ProxyType};

/// Default timeout for network operations (300 seconds)
pub const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

// Re-export from root lib.rs (where Downloader is currently defined)
// This maintains the code in one place while providing the new API structure
pub use crate::Downloader;
