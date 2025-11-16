//! Youtube client module.
//!
//! This module provides the main Youtube client struct and related configuration types.

pub mod builder;
pub mod config;
pub mod deps;
pub mod download_builder;
pub mod proxy;
mod streams;

pub use builder::YoutubeBuilder;
pub use config::YoutubeConfig;
pub use deps::{Libraries, LibraryInstaller};
pub use download_builder::DownloadBuilder;
pub use proxy::{ProxyConfig, ProxyType};

// Re-export from root lib.rs (where Youtube is currently defined)
// This maintains the code in one place while providing the new API structure
pub use crate::Youtube;
