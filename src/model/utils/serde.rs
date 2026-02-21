//! Serde utilities for serializing and deserializing data.

use serde::{Deserialize, Deserializer, Serialize};

/// Fix issue with 'none' string in JSON.
pub fn json_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let string: Option<String> = Option::deserialize(deserializer)?;

    match string.as_deref() {
        Some("none") => Ok(None),
        _ => Ok(string),
    }
}

/// Serializes a value to a JSON string, returning an empty string on failure.
///
/// # Arguments
///
/// * `value` - The value to serialize
///
/// # Returns
///
/// The JSON string representation, or an empty string if serialization fails.
pub fn serialize_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Serializes an optional value to an optional JSON string, returning `None` if the input is `None`.
///
/// # Arguments
///
/// * `value` - The optional value to serialize
///
/// # Returns
///
/// `Some(json_string)` if the value is `Some`, `None` otherwise.
/// On serialization failure, returns `Some("")`.
pub fn serialize_json_opt<T: Serialize>(value: Option<T>) -> Option<String> {
    value.map(|v| serde_json::to_string(&v).unwrap_or_default())
}
