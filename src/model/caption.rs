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
    Json,
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
    Srt,
    /// The ASS (Advanced SubStation Alpha) extension.
    Ass,
    /// The SSA (SubStation Alpha) extension.
    Ssa,
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
            Extension::Json => write!(f, "json"),
            Extension::Json3 => write!(f, "json3"),
            Extension::Srv1 => write!(f, "srv1"),
            Extension::Srv2 => write!(f, "srv2"),
            Extension::Srv3 => write!(f, "srv3"),
            Extension::Ttml => write!(f, "ttml"),
            Extension::Vtt => write!(f, "vtt"),
            Extension::Srt => write!(f, "srt"),
            Extension::Ass => write!(f, "ass"),
            Extension::Ssa => write!(f, "ssa"),
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

/// Represents a subtitle (user-uploaded or automatic caption) for a video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtitle {
    /// The language code of the subtitle (e.g., 'en', 'fr', 'es').
    pub language_code: Option<String>,
    /// The full language name (e.g., 'English', 'French', 'Spanish').
    pub language_name: Option<String>,
    /// The URL of the subtitle file.
    pub url: String,
    /// The file extension/format of the subtitle.
    #[serde(rename = "ext")]
    pub extension: Extension,
    /// Whether this is an automatically generated subtitle.
    #[serde(default)]
    pub is_automatic: bool,
}

impl Subtitle {
    /// Creates a new Subtitle from an AutomaticCaption.
    pub fn from_automatic_caption(caption: &AutomaticCaption, language_code: String) -> Self {
        Self {
            language_code: Some(language_code),
            language_name: caption.name.clone(),
            url: caption.url.clone(),
            extension: caption.extension.clone(),
            is_automatic: true,
        }
    }

    /// Checks if this subtitle is in a specific format.
    pub fn is_format(&self, format: &Extension) -> bool {
        &self.extension == format
    }

    /// Returns the file extension as a string.
    pub fn file_extension(&self) -> &str {
        match self.extension {
            Extension::Json => "json",
            Extension::Json3 => "json3",
            Extension::Srv1 => "srv1",
            Extension::Srv2 => "srv2",
            Extension::Srv3 => "srv3",
            Extension::Ttml => "ttml",
            Extension::Vtt => "vtt",
            Extension::Srt => "srt",
            Extension::Ass => "ass",
            Extension::Ssa => "ssa",
        }
    }
}

// Implementation of the Display trait for Subtitle
impl fmt::Display for Subtitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Subtitle(lang={}, format={}, auto={})",
            self.language_name
                .as_deref()
                .or(self.language_code.as_deref())
                .unwrap_or("unknown"),
            self.file_extension(),
            self.is_automatic
        )
    }
}

// Implementation of Eq for Subtitle
impl Eq for Subtitle {}

// Implementation of Hash for Subtitle
impl Hash for Subtitle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.language_code.hash(state);
        self.url.hash(state);
        std::mem::discriminant(&self.extension).hash(state);
    }
}
