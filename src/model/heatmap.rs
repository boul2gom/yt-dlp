//! Heatmap-related models.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents the complete heatmap data for a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heatmap {
    /// The start time of this heatmap segment in seconds.
    pub start_time: f64,
    /// The end time of this heatmap segment in seconds.
    pub end_time: f64,
    /// The normalized engagement value for this segment (typically 0.0 to 1.0).
    /// Higher values indicate more viewer engagement (replays, watches).
    pub value: f64,
}

impl Heatmap {
    /// Returns the duration of this heatmap segment in seconds.
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }

    /// Checks if a given timestamp (in seconds) falls within this heatmap segment.
    pub fn contains_timestamp(&self, timestamp: f64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }
}

// Implementation of the Display trait for Heatmap
impl fmt::Display for Heatmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HeatmapPoint(start={:.2}s, end={:.2}s, value={:.2})",
            self.start_time, self.end_time, self.value
        )
    }
}

// Implementation of Eq for Heatmap
impl Eq for Heatmap {}

// Implementation of Hash for HeatmapPoint
impl Hash for Heatmap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start_time.to_bits().hash(state);
        self.end_time.to_bits().hash(state);
        self.value.to_bits().hash(state);
    }
}
