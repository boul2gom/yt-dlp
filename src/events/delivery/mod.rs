//! Event delivery mechanisms.
//!
//! This module contains Rust hooks (feature: hooks) and HTTP webhooks (feature: webhooks).

#[cfg(feature = "hooks")]
pub mod hooks;
#[cfg(feature = "webhooks")]
pub mod webhooks;
