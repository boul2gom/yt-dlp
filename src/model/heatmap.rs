//! Heatmap-related models.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents a point in a video heatmap.
/// Heatmaps show viewer engagement across different segments of a video,
/// commonly known as "Most Replayed" segments on YouTube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatmapPoint {
    /// The start time of this heatmap segment in seconds.
    pub start_time: f64,
    /// The end time of this heatmap segment in seconds.
    pub end_time: f64,
    /// The normalized engagement value for this segment (typically 0.0 to 1.0).
    /// Higher values indicate more viewer engagement (replays, watches).
    pub value: f64,
}

impl HeatmapPoint {
    /// Returns the duration of this heatmap segment in seconds.
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }

    /// Checks if a given timestamp (in seconds) falls within this heatmap segment.
    pub fn contains_timestamp(&self, timestamp: f64) -> bool {
        timestamp >= self.start_time && timestamp < self.end_time
    }
}

/// Represents the complete heatmap data for a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Heatmap {
    /// The collection of heatmap points covering the video timeline.
    pub points: Vec<HeatmapPoint>,
}

impl Heatmap {
    /// Returns the heatmap point with the highest engagement value.
    /// This represents the most replayed segment of the video.
    pub fn most_engaged_segment(&self) -> Option<&HeatmapPoint> {
        self.points.iter().max_by(|a, b| {
            a.value
                .partial_cmp(&b.value)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Returns the heatmap point at a specific timestamp.
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The timestamp in seconds
    ///
    /// # Returns
    ///
    /// The heatmap point containing the timestamp, or None if no point matches
    pub fn get_point_at_time(&self, timestamp: f64) -> Option<&HeatmapPoint> {
        self.points
            .iter()
            .find(|point| point.contains_timestamp(timestamp))
    }

    /// Returns all heatmap points with an engagement value above the threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold` - The minimum engagement value (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// A vector of references to highly engaged segments
    pub fn get_highly_engaged_segments(&self, threshold: f64) -> Vec<&HeatmapPoint> {
        self.points
            .iter()
            .filter(|point| point.value >= threshold)
            .collect()
    }
}

// Implementation of the Display trait for HeatmapPoint
impl fmt::Display for HeatmapPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HeatmapPoint(start={:.2}s, end={:.2}s, value={:.2})",
            self.start_time, self.end_time, self.value
        )
    }
}

// Implementation of the Display trait for Heatmap
impl fmt::Display for Heatmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Heatmap(points={})", self.points.len())
    }
}

// Implementation of Eq for HeatmapPoint
impl Eq for HeatmapPoint {}

// Implementation of Eq for Heatmap
impl Eq for Heatmap {}

// Implementation of Hash for HeatmapPoint
impl Hash for HeatmapPoint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start_time.to_bits().hash(state);
        self.end_time.to_bits().hash(state);
        self.value.to_bits().hash(state);
    }
}

// Implementation of Hash for Heatmap
impl Hash for Heatmap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for point in &self.points {
            point.hash(state);
        }
    }
}
