use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Represents whether content is protected by digital rights management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HasDrm {
    /// Is protected by DRM and cannot be downloaded.
    Yes,
    /// May be protected by DRM and has to be tested before download.
    Maybe,
    /// Is not protected by DRM.
    No,
}

impl Serialize for HasDrm {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            HasDrm::Yes => serializer.serialize_bool(true),
            HasDrm::Maybe => serializer.serialize_str("maybe"),
            HasDrm::No => serializer.serialize_bool(false),
        }
    }
}

impl<'de> Deserialize<'de> for HasDrm {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Bool(true) => Ok(HasDrm::Yes),
            Value::String(s) if s == "maybe" => Ok(HasDrm::Maybe),
            Value::Bool(false) => Ok(HasDrm::No),
            _ => Err(serde::de::Error::custom("Invalid value for the `HasDrm` type. Expected a boolean or \"maybe\".")),
        }
    }
}
