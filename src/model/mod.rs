//! The models used to represent the data fetched by 'yt-dlp'.
//!
//! The represented data is the video information, thumbnails, automatic captions, and formats.

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod caption;
pub mod chapter;
pub mod format;
pub mod heatmap;
pub mod playlist;
pub mod selector;
pub mod thumbnail;
pub mod utils; // Keep for traits
pub mod video;

// Re-export main types
pub use video::Video;

// Re-export chapter types
pub use chapter::{ChapterList, ChapterValidation};

// Re-export selector types
pub use selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};

// Re-export utility traits
pub use utils::{AllTraits, CommonTraits};

/// DRM status of a video or format
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum DrmStatus {
    Yes,
    No,
    Maybe,
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
                match value {
                    "Yes" => Ok(DrmStatus::Yes),
                    "No" => Ok(DrmStatus::No),
                    "maybe" => Ok(DrmStatus::Maybe),
                    _ => Err(E::custom(format!("Expected \"maybe\", got \"{}\"", value))),
                }
            }
        }

        deserializer.deserialize_any(DrmStatusVisitor)
    }
}
