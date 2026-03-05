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

// ---------------------------------------------------------------------------
// MetadataManager::add_metadata — MP3 file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_metadata_to_mp3_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let src = crate::common::fixtures::media_fixture("small.mp3");
    let dest = dir.path().join("tagged.mp3");
    std::fs::copy(&src, &dest).unwrap();

    let video = load_video_fixture();
    let manager = yt_dlp::metadata::MetadataManager::new();
    let result = manager.add_metadata(&dest, &video).await;
    assert!(result.is_ok(), "add_metadata to MP3 should succeed: {:?}", result.err());
}

// ---------------------------------------------------------------------------
// MetadataManager::add_metadata — M4A file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_metadata_to_m4a_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let src = crate::common::fixtures::media_fixture("small.m4a");
    let dest = dir.path().join("tagged.m4a");
    std::fs::copy(&src, &dest).unwrap();

    let video = load_video_fixture();
    let manager = yt_dlp::metadata::MetadataManager::new();
    let result = manager.add_metadata(&dest, &video).await;
    assert!(result.is_ok(), "add_metadata to M4A should succeed: {:?}", result.err());
}

// ---------------------------------------------------------------------------
// MetadataManager::add_metadata — FLAC file (lofty path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_metadata_to_flac_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let src = crate::common::fixtures::media_fixture("small.flac");
    let dest = dir.path().join("tagged.flac");
    std::fs::copy(&src, &dest).unwrap();

    let video = load_video_fixture();
    let manager = yt_dlp::metadata::MetadataManager::new();
    let result = manager.add_metadata(&dest, &video).await;
    assert!(
        result.is_ok(),
        "add_metadata to FLAC should succeed: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// MetadataManager::add_metadata — WAV file (lofty path)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_metadata_to_wav_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let src = crate::common::fixtures::media_fixture("small.wav");
    let dest = dir.path().join("tagged.wav");
    std::fs::copy(&src, &dest).unwrap();

    let video = load_video_fixture();
    let manager = yt_dlp::metadata::MetadataManager::new();
    let result = manager.add_metadata(&dest, &video).await;
    assert!(result.is_ok(), "add_metadata to WAV should succeed: {:?}", result.err());
}

// ---------------------------------------------------------------------------
// MetadataManager::add_thumbnail_to_file — MP3
// ---------------------------------------------------------------------------

#[tokio::test]
async fn add_thumbnail_to_mp3_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let media_src = crate::common::fixtures::media_fixture("small.mp3");
    let thumb_src = crate::common::fixtures::media_fixture("thumb.jpg");
    let dest = dir.path().join("with_thumb.mp3");
    std::fs::copy(&media_src, &dest).unwrap();

    let manager = yt_dlp::metadata::MetadataManager::new();
    let result = manager.add_thumbnail_to_file(&dest, &thumb_src).await;
    assert!(
        result.is_ok(),
        "add_thumbnail to MP3 should succeed: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// MetadataManager::extract_basic_metadata — key fields present
// ---------------------------------------------------------------------------

#[test]
fn extract_basic_metadata_contains_key_fields() {
    let video = load_video_fixture();
    let metadata = yt_dlp::metadata::MetadataManager::extract_basic_metadata(&video);
    let keys: Vec<&str> = metadata.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"title"), "title should be in metadata");
}
