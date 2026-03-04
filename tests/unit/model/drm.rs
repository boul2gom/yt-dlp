use pretty_assertions::assert_eq;
use yt_dlp::model::DrmStatus;

// ============================== DrmStatus ==============================

#[test]
fn drm_status_display() {
    assert_eq!(format!("{}", DrmStatus::Yes), "Yes");
    assert_eq!(format!("{}", DrmStatus::No), "No");
    assert_eq!(format!("{}", DrmStatus::Maybe), "Maybe");
}

#[test]
fn drm_status_deserialize_bool() {
    let yes: DrmStatus = serde_json::from_str("true").unwrap();
    assert_eq!(yes, DrmStatus::Yes);
    let no: DrmStatus = serde_json::from_str("false").unwrap();
    assert_eq!(no, DrmStatus::No);
}

#[test]
fn drm_status_deserialize_string() {
    let yes: DrmStatus = serde_json::from_str("\"yes\"").unwrap();
    assert_eq!(yes, DrmStatus::Yes);
    let maybe: DrmStatus = serde_json::from_str("\"maybe\"").unwrap();
    assert_eq!(maybe, DrmStatus::Maybe);
}

#[test]
fn drm_status_default() {
    assert_eq!(DrmStatus::default(), DrmStatus::No);
}

// ============================== DrmStatus polymorphic deserialization ==============================

#[test]
fn drm_status_deserialize_true_bool() {
    let status: DrmStatus = serde_json::from_str("true").unwrap();
    assert_eq!(status, DrmStatus::Yes);
}

#[test]
fn drm_status_deserialize_false_bool() {
    let status: DrmStatus = serde_json::from_str("false").unwrap();
    assert_eq!(status, DrmStatus::No);
}

#[test]
fn drm_status_deserialize_string_yes() {
    let status: DrmStatus = serde_json::from_str("\"yes\"").unwrap();
    assert_eq!(status, DrmStatus::Yes);
}

#[test]
fn drm_status_deserialize_string_no() {
    let status: DrmStatus = serde_json::from_str("\"no\"").unwrap();
    assert_eq!(status, DrmStatus::No);
}

#[test]
fn drm_status_deserialize_string_maybe() {
    let status: DrmStatus = serde_json::from_str("\"maybe\"").unwrap();
    assert_eq!(status, DrmStatus::Maybe);
}

#[test]
fn drm_status_deserialize_string_fairplay_is_error() {
    // Only "yes", "no", "maybe" are valid string values
    let result = serde_json::from_str::<DrmStatus>("\"fairplay\"");
    assert!(result.is_err());
}

#[test]
fn drm_status_deserialize_unknown_string_is_error() {
    let result = serde_json::from_str::<DrmStatus>("\"unknown_drm_system\"");
    assert!(result.is_err());
}
