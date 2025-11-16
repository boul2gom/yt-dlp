//! Speed profiles for download optimization.
//!
//! This module provides different speed profiles that automatically configure
//! download parameters based on the user's bandwidth and use case.

use std::fmt;

/// Download speed profile
///
/// Different profiles optimize download parameters for various network conditions
/// and use cases. Each profile adjusts concurrent downloads, parallel segments,
/// segment size, and buffer size to match the expected bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

impl fmt::Display for SpeedProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conservative => write!(f, "Conservative"),
            Self::Balanced => write!(f, "Balanced"),
            Self::Aggressive => write!(f, "Aggressive"),
        }
    }
}

impl SpeedProfile {
    /// Get the maximum number of concurrent downloads for this profile
    pub fn max_concurrent_downloads(&self) -> usize {
        match self {
            Self::Conservative => 3,
            Self::Balanced => 5,
            Self::Aggressive => 8,
        }
    }

    /// Get the segment size in bytes for this profile
    pub fn segment_size(&self) -> usize {
        match self {
            Self::Conservative => 5 * 1024 * 1024, // 5 MB
            Self::Balanced => 8 * 1024 * 1024,     // 8 MB
            Self::Aggressive => 10 * 1024 * 1024,  // 10 MB
        }
    }

    /// Get the number of parallel segments per download for this profile
    pub fn parallel_segments(&self) -> usize {
        match self {
            Self::Conservative => 4,
            Self::Balanced => 8,
            Self::Aggressive => 12,
        }
    }

    /// Get the maximum buffer size in bytes for this profile
    pub fn max_buffer_size(&self) -> usize {
        match self {
            Self::Conservative => 10 * 1024 * 1024, // 10 MB
            Self::Balanced => 20 * 1024 * 1024,     // 20 MB
            Self::Aggressive => 30 * 1024 * 1024,   // 30 MB
        }
    }

    /// Get the maximum parallel segments for large files (> 2 GB)
    ///
    /// This is used by the dynamic segment calculation in Fetcher
    pub fn max_parallel_segments_for_large_files(&self) -> usize {
        match self {
            Self::Conservative => 24,
            Self::Balanced => 32,
            Self::Aggressive => 48,
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

        let max_parallel_segments = match self {
            Self::Conservative => match file_size_mb {
                size if size < 10 => 1,
                size if size < 50 => 2,
                size if size < 100 => 4,
                size if size < 500 => 8,
                size if size < 1000 => 12,
                size if size < 2000 => 16,
                _ => 24,
            },
            Self::Balanced => match file_size_mb {
                size if size < 10 => 2,
                size if size < 50 => 4,
                size if size < 100 => 8,
                size if size < 500 => 12,
                size if size < 1000 => 16,
                size if size < 2000 => 24,
                _ => 32,
            },
            Self::Aggressive => match file_size_mb {
                size if size < 10 => 4,
                size if size < 50 => 8,
                size if size < 100 => 12,
                size if size < 500 => 16,
                size if size < 1000 => 24,
                size if size < 2000 => 32,
                _ => 48,
            },
        };

        std::cmp::min(total_segments as usize, max_parallel_segments)
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
