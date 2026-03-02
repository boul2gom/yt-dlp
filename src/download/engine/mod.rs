//! Download engine internals.
//!
//! This module contains the core download engine components: HTTP fetching,
//! segment-based parallel downloads, range request support, and partial downloads.

pub mod fetcher;
mod parallel;
pub mod partial;
pub mod range_fetcher;
pub mod segment;
