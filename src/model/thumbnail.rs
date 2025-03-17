//! Thumbnails-related models.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents a thumbnail of a YouTube video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thumbnail {
    /// The URL of the thumbnail.
    pub url: String,
    /// The preference index of the thumbnail, e.g. '-35' or '0'.
    pub preference: i64,

    /// The ID of the thumbnail.
    pub id: String,
    /// The height of the thumbnail, can be `None`.
    pub height: Option<i64>,
    /// The width of the thumbnail, can be `None`.
    pub width: Option<i64>,
    /// The resolution of the thumbnail, can be `None`, e.g. '1920x1080'.
    pub resolution: Option<String>,
}

// Implementation of the Display trait for Thumbnail
impl fmt::Display for Thumbnail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Thumbnail(id={}, resolution={})",
            self.id,
            self.resolution.as_deref().unwrap_or("unknown")
        )
    }
}

// Implementation of Eq for Thumbnail
impl Eq for Thumbnail {}

// Implementation of Hash for Thumbnail
impl Hash for Thumbnail {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.url.hash(state);
        self.preference.hash(state);
    }
}
