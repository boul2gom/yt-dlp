//! Captions-related models.

use std::fmt;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

/// Represents an automatic caption of a YouTube video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extension {
    /// The JSON extension.
    Json,
    /// The JSON3 extension.
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
    #[default]
    Vtt,
    /// The Srt extension.
    Srt,
    /// The ASS (Advanced SubStation Alpha) extension.
    Ass,
    /// The SSA (SubStation Alpha) extension.
    Ssa,
    /// An unknown extension not yet covered by the library.
    #[serde(other)]
    Unknown,
}

impl Extension {
    /// Returns the extension as a string slice.
    ///
    /// # Returns
    ///
    /// A static string representation of this extension variant (e.g. `"vtt"`, `"srt"`).
    pub fn as_str(&self) -> &'static str {
        match self {
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
            Extension::Unknown => "unknown",
        }
    }
}

impl fmt::Display for AutomaticCaption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AutomaticCaption(lang={}, ext={:?})",
            self.name.as_deref().unwrap_or("unknown"),
            self.extension
        )
    }
}

impl Hash for AutomaticCaption {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
        self.name.hash(state);
        std::mem::discriminant(&self.extension).hash(state);
    }
}

impl fmt::Display for Extension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Creates a new [`Subtitle`] from an [`AutomaticCaption`], marking it as automatically generated.
    ///
    /// # Arguments
    ///
    /// * `caption` - The automatic caption to convert.
    /// * `language_code` - The language code to assign (e.g. `"en"`, `"fr"`).
    ///
    /// # Returns
    ///
    /// A [`Subtitle`] with `is_automatic` set to `true`.
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
    ///
    /// # Arguments
    ///
    /// * `format` - The extension variant to compare against.
    ///
    /// # Returns
    ///
    /// `true` if the subtitle's extension matches the given format.
    pub fn is_format(&self, format: &Extension) -> bool {
        &self.extension == format
    }

    /// Returns the file extension as a string.
    ///
    /// # Returns
    ///
    /// The extension string (e.g. `"vtt"`, `"srt"`).
    pub fn file_extension(&self) -> &str {
        self.extension.as_str()
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

// Implementation of Hash for Subtitle
impl Hash for Subtitle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.language_code.hash(state);
        self.url.hash(state);
        std::mem::discriminant(&self.extension).hash(state);
    }
}
