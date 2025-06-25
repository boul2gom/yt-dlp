//! Captions-related models.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents an automatic caption of a YouTube video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomaticCaption {
    /// The extension of the caption file.
    #[serde(rename = "ext")]
    pub extension: Extension,
    /// The URL of the caption file.
    pub url: String,
    /// The language of the caption file, e.g. 'English'.
    pub name: Option<String>,
}

/// The available extensions for automatic caption files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extension {
    /// The JSON extension.
    Json3,
    /// The Srv1 extension.
    Srv1,
    /// The Srv2 extension.
    Srv2,
    /// The Srv3 extension.
    Srv3,
    /// The Ttml extension.
    Ttml,
    /// The Vtt extension.
    Vtt,
    /// The Srt extension.
    Srt
}

// Implementation of the Display trait for AutomaticCaption
impl fmt::Display for AutomaticCaption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Caption(lang={}, ext={:?})",
            self.name.as_deref().unwrap_or("unknown"),
            self.extension
        )
    }
}

// Implementation of Eq for AutomaticCaption
impl Eq for AutomaticCaption {}

// Implementation of Hash for AutomaticCaption
impl Hash for AutomaticCaption {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
        self.name.hash(state);
        std::mem::discriminant(&self.extension).hash(state);
    }
}

// Implementation of the Display trait for Extension
impl fmt::Display for Extension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Extension::Json3 => write!(f, "json3"),
            Extension::Srv1 => write!(f, "srv1"),
            Extension::Srv2 => write!(f, "srv2"),
            Extension::Srv3 => write!(f, "srv3"),
            Extension::Ttml => write!(f, "ttml"),
            Extension::Vtt => write!(f, "vtt"),
            Extension::Srt => write!(f, "srt"),
        }
    }
}

// Implementation of Eq for Extension
impl Eq for Extension {}

// Implementation of Hash for Extension
impl Hash for Extension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}
