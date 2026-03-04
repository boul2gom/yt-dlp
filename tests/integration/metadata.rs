use yt_dlp::metadata::{BaseMetadata, MetadataManager};
use yt_dlp::model::chapter::Chapter;

fn load_video_fixture() -> yt_dlp::model::Video {
    let data = include_str!("../fixtures/json/video.json");
    serde_json::from_str(data).expect("fixture deserialization failed")
}

// ---------------------------------------------------------------------------
// extract_basic_metadata — tests unique to integration layer
// ---------------------------------------------------------------------------

#[test]
fn extract_basic_metadata_skips_date_when_zero() {
    let mut video = load_video_fixture();
    video.upload_date = Some(0);

    let metadata = MetadataManager::extract_basic_metadata(&video);

    assert!(
        metadata.iter().all(|(k, _)| k != "date"),
        "date should not be present when upload_date is 0"
    );
}

// ---------------------------------------------------------------------------
// Chapter model — additional integration-level tests
// ---------------------------------------------------------------------------

#[test]
fn chapter_duration() {
    let chapter = Chapter {
        start_time: 10.0,
        end_time: 70.0,
        title: Some("Test".to_string()),
    };
    assert!((chapter.duration() - 60.0).abs() < f64::EPSILON);
    assert!((chapter.duration_minutes() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn chapter_contains_timestamp() {
    let chapter = Chapter {
        start_time: 10.0,
        end_time: 70.0,
        title: None,
    };
    assert!(chapter.contains_timestamp(10.0));
    assert!(chapter.contains_timestamp(50.0));
    assert!(!chapter.contains_timestamp(70.0)); // exclusive
    assert!(!chapter.contains_timestamp(5.0));
}

// ---------------------------------------------------------------------------
// PlaylistMetadata
// ---------------------------------------------------------------------------

#[test]
fn playlist_metadata_fields() {
    let pm = yt_dlp::metadata::PlaylistMetadata {
        title: "My Playlist".to_string(),
        id: "PL123".to_string(),
        index: 3,
        total: Some(10),
    };

    assert_eq!(pm.title, "My Playlist");
    assert_eq!(pm.id, "PL123");
    assert_eq!(pm.index, 3);
    assert_eq!(pm.total, Some(10));

    let debug = format!("{:?}", pm);
    assert!(debug.contains("PlaylistMetadata"));
}
