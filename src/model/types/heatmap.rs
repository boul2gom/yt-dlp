//! Heatmap-related models.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Heatmap(Vec<HeatmapPoint>);

/// Represents the complete heatmap data for a video.
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

impl Heatmap {
    /// Creates a new Heatmap from a list of points.
    ///
    /// # Arguments
    ///
    /// * `points` - The heatmap points to include
    ///
    /// # Returns
    ///
    /// A new `Heatmap` instance containing the provided points
    pub fn new(points: Vec<HeatmapPoint>) -> Self {
        Self(points)
    }

    /// Returns a slice of all heatmap points.
    ///
    /// # Returns
    ///
    /// A slice of all `HeatmapPoint` entries in this heatmap
    pub fn points(&self) -> &[HeatmapPoint] {
        &self.0
    }

    /// Returns the heatmap point with the highest engagement value.
    /// This represents the most replayed segment of the video.
    ///
    /// # Returns
    ///
    /// The most engaged `HeatmapPoint`, or `None` if the heatmap is empty
    pub fn most_engaged_segment(&self) -> Option<&HeatmapPoint> {
        self.points()
            .iter()
            .max_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal))
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
        self.points().iter().filter(|p| p.value >= threshold).collect()
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
        self.points().iter().find(|p| p.contains_timestamp(timestamp))
    }

    /// Checks if the heatmap is empty.
    ///
    /// # Returns
    ///
    /// True if the heatmap is empty, false otherwise
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Heatmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Heatmap(points={})", self.0.len())
    }
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
