//! Partial download support for downloading specific portions of videos.
//!
//! This module provides functionality to download only specific parts of a video,
//! either by time range or by chapter index.

use std::fmt;

use crate::error::{Error, Result};

/// Represents a range specification for partial downloads.
///
/// # Examples
///
/// ```rust,no_run
/// use yt_dlp::download::PartialRange;
///
/// // Download from 1:30 to 5:00
/// let time_range = PartialRange::TimeRange {
///     start: 90.0,
///     end: 300.0,
/// };
///
/// // Download chapters 2 through 5
/// let chapter_range = PartialRange::ChapterRange { start: 2, end: 5 };
///
/// // Download only chapter 3
/// let single_chapter = PartialRange::SingleChapter { index: 3 };
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum PartialRange {
    /// Download a specific time range (in seconds)
    TimeRange {
        /// Start time in seconds
        start: f64,
        /// End time in seconds
        end: f64,
    },
    /// Download a range of chapters
    ChapterRange {
        /// First chapter index (0-based)
        start: usize,
        /// Last chapter index (0-based, inclusive)
        end: usize,
    },
    /// Download a single chapter
    SingleChapter {
        /// Chapter index (0-based)
        index: usize,
    },
}

impl PartialRange {
    /// Creates a time range for partial download.
    ///
    /// # Arguments
    ///
    /// * `start` - Start time in seconds (must be non-negative)
    /// * `end` - End time in seconds (must be greater than `start`)
    ///
    /// # Errors
    ///
    /// Returns an error if `start` is negative or `start >= end`.
    ///
    /// # Returns
    ///
    /// A PartialRange instance representing the time range
    pub fn time_range(start: f64, end: f64) -> Result<Self> {
        tracing::debug!(start = start, end = end, "⚙️ Creating time range for partial download");

        if start < 0.0 || start >= end {
            return Err(Error::invalid_partial_range(format!(
                "start={start} must be non-negative and less than end={end}"
            )));
        }
        Ok(Self::TimeRange { start, end })
    }

    /// Creates a chapter range for partial download.
    ///
    /// # Arguments
    ///
    /// * `start` - First chapter index (0-based)
    /// * `end` - Last chapter index (0-based, inclusive, must be >= `start`)
    ///
    /// # Errors
    ///
    /// Returns an error if `start > end`.
    ///
    /// # Returns
    ///
    /// A PartialRange instance representing the chapter range
    pub fn chapter_range(start: usize, end: usize) -> Result<Self> {
        tracing::debug!(
            start = start,
            end = end,
            "⚙️ Creating chapter range for partial download"
        );

        if start > end {
            return Err(Error::invalid_partial_range(format!(
                "chapter start={start} must be <= end={end}"
            )));
        }
        Ok(Self::ChapterRange { start, end })
    }

    /// Creates a single chapter for partial download.
    ///
    /// # Arguments
    ///
    /// * `index` - Chapter index (0-based)
    ///
    /// # Returns
    ///
    /// A PartialRange instance representing a single chapter
    pub fn single_chapter(index: usize) -> Self {
        Self::SingleChapter { index }
    }

    /// Converts this range to yt-dlp's --download-sections format.
    ///
    /// # Returns
    ///
    /// A string in yt-dlp format (e.g., "*00:01:30-00:05:00")
    pub fn to_ytdlp_arg(&self) -> String {
        tracing::debug!(range = %self, "⚙️ Converting partial range to yt-dlp argument");

        let result = match self {
            Self::TimeRange { start, end } => {
                format!("*{}-{}", format_time(*start), format_time(*end))
            }
            Self::ChapterRange { start, end } => {
                // For chapter ranges, we'll need to convert to time ranges
                // This will be done at runtime with actual chapter data
                format!("chapters:{}-{}", start, end)
            }
            Self::SingleChapter { index } => {
                format!("chapters:{}-{}", index, index)
            }
        };

        tracing::debug!(range = %self, ytdlp_arg = %result, "✅ Converted partial range to yt-dlp argument");

        result
    }

    /// Checks if this range needs chapter metadata to be resolved.
    ///
    /// # Returns
    ///
    /// true if chapter metadata is needed, false otherwise
    pub fn needs_chapter_metadata(&self) -> bool {
        matches!(self, Self::ChapterRange { .. } | Self::SingleChapter { .. })
    }

    /// Converts a chapter range to a time range using chapter metadata.
    ///
    /// # Arguments
    ///
    /// * `chapters` - List of chapters with start_time and end_time
    ///
    /// # Returns
    ///
    /// A TimeRange variant if conversion is successful, or None if indices are out of bounds
    ///
    /// # Errors
    ///
    /// Returns None if chapter indices are out of bounds
    pub fn to_time_range(&self, chapters: &[crate::model::chapter::Chapter]) -> Option<Self> {
        tracing::debug!(range = %self, chapter_count = chapters.len(), "⚙️ Converting partial range to time range using chapter metadata");

        match self {
            Self::TimeRange { .. } => Some(self.clone()),
            Self::ChapterRange { start, end } => {
                if *end >= chapters.len() {
                    tracing::warn!(
                        start = start,
                        end = end,
                        chapter_count = chapters.len(),
                        "⚙️ Chapter range end index out of bounds"
                    );
                    return None;
                }
                let start_time = chapters[*start].start_time;
                let end_time = chapters[*end].end_time;

                tracing::debug!(
                    start_chapter = start,
                    end_chapter = end,
                    start_time = start_time,
                    end_time = end_time,
                    "✅ Converted chapter range to time range"
                );

                Some(Self::TimeRange {
                    start: start_time,
                    end: end_time,
                })
            }
            Self::SingleChapter { index } => {
                if *index >= chapters.len() {
                    tracing::warn!(
                        index = index,
                        chapter_count = chapters.len(),
                        "⚙️ Single chapter index out of bounds"
                    );
                    return None;
                }
                let start_time = chapters[*index].start_time;
                let end_time = chapters[*index].end_time;

                tracing::debug!(
                    chapter_index = index,
                    start_time = start_time,
                    end_time = end_time,
                    "✅ Converted single chapter to time range"
                );

                Some(Self::TimeRange {
                    start: start_time,
                    end: end_time,
                })
            }
        }
    }

    /// Gets the start and end times for this range.
    ///
    /// # Returns
    ///
    /// A tuple (start, end) in seconds, or None if chapter conversion is needed
    pub fn get_times(&self) -> Option<(f64, f64)> {
        match self {
            Self::TimeRange { start, end } => Some((*start, *end)),
            _ => None,
        }
    }
}

impl fmt::Display for PartialRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeRange { start, end } => {
                write!(f, "TimeRange(start={}, end={})", format_time(*start), format_time(*end))
            }
            Self::ChapterRange { start, end } => {
                write!(f, "ChapterRange(start={}, end={})", start, end)
            }
            Self::SingleChapter { index } => {
                write!(f, "SingleChapter(index={})", index)
            }
        }
    }
}

/// Formats a time in seconds to HH:MM:SS.mmm format.
///
/// # Arguments
///
/// * `seconds` - Time in seconds
///
/// # Returns
///
/// A formatted string in HH:MM:SS.mmm format
fn format_time(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u64;
    let minutes = ((seconds % 3600.0) / 60.0) as u64;
    let secs = seconds % 60.0;
    format!("{:02}:{:02}:{:06.3}", hours, minutes, secs)
}
