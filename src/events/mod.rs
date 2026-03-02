//! Event system for download lifecycle notifications
//!
//! This module provides a comprehensive event system for tracking all aspects
//! of video downloads, from metadata fetching to post-processing completion.
//!
//! # Architecture
//!
//! The event system is built around three main components:
//! - [`EventBus`] - Central event dispatcher using broadcast channels
//! - [`DownloadEvent`] - Enum of all possible events
//! - [`EventFilter`] - Filtering system for selective event handling
//!
//! # Optional Features
//!
//! - `hooks` - Enables Rust callback hooks (in-process event handlers)
//! - `webhooks` - Enables HTTP webhook delivery with retry logic
//!
//! # Examples
//!
//! ## Basic event stream
//!
//! ```ignore
//! use tokio_stream::StreamExt;
//!
//! let downloader = Downloader::builder(libraries, output_dir).build().await?;
//! let mut stream = downloader.event_stream();
//!
//! while let Some(Ok(event)) = stream.next().await {
//!     match &*event {
//!         DownloadEvent::DownloadCompleted { download_id, output_path, .. } => {
//!             println!("Download {} completed: {:?}", download_id, output_path);
//!         }
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ## With filtering
//!
//! ```ignore
//! use yt_dlp::events::EventFilter;
//!
//! let filter = EventFilter::only_terminal().exclude_progress();
//! // Use filter with hooks or custom stream processing
//! ```

pub mod bus;
pub mod delivery;
pub mod filters;
pub mod types;

pub use bus::EventBus;
#[cfg(feature = "hooks")]
pub use delivery::hooks::{EventHook, HookError, HookRegistry, HookResult};
pub use filters::EventFilter;
#[cfg(feature = "live-recording")]
pub use types::RecordingMethod;
pub use types::{DownloadEvent, MetadataType, PostProcessOperation};

#[cfg(feature = "webhooks")]
mod retry;

#[cfg(feature = "webhooks")]
pub use delivery::webhooks::{WebhookConfig, WebhookDelivery, WebhookMethod};
#[cfg(feature = "webhooks")]
pub use retry::RetryStrategy;
