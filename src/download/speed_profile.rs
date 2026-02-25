//! Speed profiles for download optimization.
//!
//! This module provides different speed profiles that automatically configure
//! download parameters based on the user's bandwidth and use case.

use std::fmt;

/// Default concurrent downloads for Conservative profile
const CONSERVATIVE_CONCURRENT: usize = 3;
/// Default concurrent downloads for Balanced profile
const BALANCED_CONCURRENT: usize = 4;
/// Default concurrent downloads for Aggressive profile
const AGGRESSIVE_CONCURRENT: usize = 6;

/// Default segment size for Conservative profile (5 MB)
const CONSERVATIVE_SEGMENT_SIZE: usize = 5 * 1024 * 1024;
/// Default segment size for Balanced profile (8 MB)
const BALANCED_SEGMENT_SIZE: usize = 8 * 1024 * 1024;
/// Default segment size for Aggressive profile (10 MB)
const AGGRESSIVE_SEGMENT_SIZE: usize = 10 * 1024 * 1024;

/// Default parallel segments for Conservative profile
const CONSERVATIVE_PARALLEL: usize = 4;
/// Default parallel segments for Balanced profile
const BALANCED_PARALLEL: usize = 5;
/// Default parallel segments for Aggressive profile
const AGGRESSIVE_PARALLEL: usize = 6;

/// Default buffer size for Conservative profile (10 MB)
const CONSERVATIVE_BUFFER: usize = 10 * 1024 * 1024;
/// Default buffer size for Balanced profile (20 MB)
const BALANCED_BUFFER: usize = 20 * 1024 * 1024;
/// Default buffer size for Aggressive profile (30 MB)
const AGGRESSIVE_BUFFER: usize = 30 * 1024 * 1024;

/// Download speed profile
///
/// Different profiles optimize download parameters for various network conditions
/// and use cases. Each profile adjusts concurrent downloads, parallel segments,
/// segment size, and buffer size to match the expected bandwidth.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SpeedProfile {
    /// Conservative profile for slower connections (< 50 Mbps)
    ///
    /// Best for:
    /// - Standard internet connections
    /// - Avoiding network congestion
    /// - Limited bandwidth scenarios
    Conservative,

    /// Balanced profile for medium-speed connections (50-500 Mbps)
    ///
    /// Best for:
    /// - Most modern internet connections
    /// - General use cases
    /// - Balance between speed and resource usage
    #[default]
    Balanced,

    /// Aggressive profile for high-speed connections (> 500 Mbps)
    ///
    /// Best for:
    /// - High-bandwidth connections (fiber, gigabit)
    /// - Maximizing download speed
    /// - Systems with ample resources
    Aggressive,
}

impl SpeedProfile {
    /// Get the maximum number of concurrent downloads for this profile
    ///
    /// # Returns
    ///
    /// Maximum number of concurrent downloads
    pub fn max_concurrent_downloads(&self) -> usize {
        match self {
            Self::Conservative => CONSERVATIVE_CONCURRENT,
            Self::Balanced => BALANCED_CONCURRENT,
            Self::Aggressive => AGGRESSIVE_CONCURRENT,
        }
    }

    /// Get the segment size in bytes for this profile
    ///
    /// # Returns
    ///
    /// Segment size in bytes
    pub fn segment_size(&self) -> usize {
        match self {
            Self::Conservative => CONSERVATIVE_SEGMENT_SIZE,
            Self::Balanced => BALANCED_SEGMENT_SIZE,
            Self::Aggressive => AGGRESSIVE_SEGMENT_SIZE,
        }
    }

    /// Get the number of parallel segments per download for this profile
    ///
    /// # Returns
    ///
    /// Number of parallel segments
    pub fn parallel_segments(&self) -> usize {
        match self {
            Self::Conservative => CONSERVATIVE_PARALLEL,
            Self::Balanced => BALANCED_PARALLEL,
            Self::Aggressive => AGGRESSIVE_PARALLEL,
        }
    }

    /// Get the maximum buffer size in bytes for this profile
    ///
    /// # Returns
    ///
    /// Maximum buffer size in bytes
    pub fn max_buffer_size(&self) -> usize {
        match self {
            Self::Conservative => CONSERVATIVE_BUFFER,
            Self::Balanced => BALANCED_BUFFER,
            Self::Aggressive => AGGRESSIVE_BUFFER,
        }
    }

    /// Get the maximum parallel segments for large files (> 2 GB)
    ///
    /// This is used by the dynamic segment calculation in Fetcher
    pub fn max_parallel_segments_for_large_files(&self) -> usize {
        match self {
            Self::Conservative => 16,
            Self::Balanced => 20,
            Self::Aggressive => 24,
        }
    }

    /// Calculate optimal number of segments based on file size and profile
    ///
    /// # Arguments
    ///
    /// * `file_size` - The total size of the file in bytes
    /// * `segment_size` - The size of each segment in bytes
    ///
    /// # Returns
    ///
    /// The optimal number of parallel segments for this file size and profile
    pub fn calculate_optimal_segments(&self, file_size: u64, segment_size: u64) -> usize {
        let total_segments = file_size.div_ceil(segment_size);
        let file_size_mb = file_size / (1024 * 1024);

        tracing::debug!(
            profile = %self,
            file_size_mb = file_size_mb,
            segment_size = segment_size,
            total_segments = total_segments,
            "📥 Calculating optimal segments"
        );

        let max_parallel_segments = match self {
            Self::Conservative => match file_size_mb {
                size if size < 10 => 1,
                size if size < 50 => 2,
                size if size < 100 => 4,
                size if size < 500 => 6,
                size if size < 1000 => 8,
                size if size < 2000 => 12,
                _ => 16,
            },
            Self::Balanced => match file_size_mb {
                size if size < 10 => 2,
                size if size < 50 => 3,
                size if size < 100 => 5,
                size if size < 500 => 8,
                size if size < 1000 => 12,
                size if size < 2000 => 16,
                _ => 20,
            },
            Self::Aggressive => match file_size_mb {
                size if size < 10 => 3,
                size if size < 50 => 5,
                size if size < 100 => 6,
                size if size < 500 => 10,
                size if size < 1000 => 14,
                size if size < 2000 => 20,
                _ => 24,
            },
        };

        let result = std::cmp::min(total_segments as usize, max_parallel_segments);

        tracing::debug!(
            profile = %self,
            file_size_mb = file_size_mb,
            max_parallel = max_parallel_segments,
            optimal = result,
            "📥 Optimal segments calculated"
        );

        result
    }

    /// Get the maximum number of concurrent downloads for playlists
    ///
    /// This limits how many videos can be downloaded simultaneously in a playlist
    pub fn max_playlist_concurrent_downloads(&self) -> usize {
        match self {
            Self::Conservative => 2,
            Self::Balanced => 3,
            Self::Aggressive => 5,
        }
    }
}

impl fmt::Display for SpeedProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conservative => write!(f, "Conservative"),
            Self::Balanced => write!(f, "Balanced"),
            Self::Aggressive => write!(f, "Aggressive"),
        }
    }
}
