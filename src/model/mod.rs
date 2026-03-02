//! The models used to represent the data fetched by 'yt-dlp'.
//!
//! The represented data is the video information, thumbnails, automatic captions, and formats.

use std::fmt;

use serde::{Deserialize, Serialize};

pub mod format;
pub mod selector;
pub mod types;
pub mod utils;
pub mod video;

// Re-export main types
// Re-export chapter types
// Re-export selector types
pub use selector::{
    AudioCodecPreference, AudioQuality, FormatPreferences, StoryboardQuality, VideoCodecPreference, VideoQuality,
};
pub use types::chapter::{ChapterList, ChapterValidation};
// Re-export types for convenience
pub use types::{caption, chapter, heatmap, playlist, thumbnail};
// Re-export utility traits
pub use utils::{AllTraits, CommonTraits};
pub use video::{FORMAT_URL_LIFETIME, Video};

/// DRM status of a video or format
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum DrmStatus {
    Yes,
    #[default]
    No,
    Maybe,
}

impl fmt::Display for DrmStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrmStatus::Yes => f.write_str("Yes"),
            DrmStatus::No => f.write_str("No"),
            DrmStatus::Maybe => f.write_str("Maybe"),
        }
    }
}

impl<'de> Deserialize<'de> for DrmStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DrmStatusVisitor;

        impl<'de> serde::de::Visitor<'de> for DrmStatusVisitor {
            type Value = DrmStatus;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("A boolean or the string \"maybe\"")
            }

            fn visit_bool<E>(self, value: bool) -> Result<DrmStatus, E>
            where
                E: serde::de::Error,
            {
                Ok(if value { DrmStatus::Yes } else { DrmStatus::No })
            }

            fn visit_str<E>(self, value: &str) -> Result<DrmStatus, E>
            where
                E: serde::de::Error,
            {
                match value.to_lowercase().as_str() {
                    "yes" => Ok(DrmStatus::Yes),
                    "no" => Ok(DrmStatus::No),
                    "maybe" => Ok(DrmStatus::Maybe),
                    _ => Err(E::custom(format!(
                        "Expected \"yes\", \"no\" or \"maybe\", got \"{}\"",
                        value
                    ))),
                }
            }
        }

        deserializer.deserialize_any(DrmStatusVisitor)
    }
}
