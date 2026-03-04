use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pretty_assertions::assert_eq;
use yt_dlp::model::Video;
use yt_dlp::model::format::FormatType;

use crate::common::fixtures;

// ============================== Video deserialization ==============================

#[test]
fn video_deserialize_from_fixture() {
    let video = fixtures::load_video_fixture();
    assert_eq!(video.id, "gXtp6C-3JKo");
    assert_eq!(video.title, "The scariest virus in History");
    assert_eq!(video.channel.as_deref(), Some("Micode"));
    assert!(video.tags.is_empty());
}

#[test]
fn video_has_correct_format_count() {
    let video = fixtures::load_video_fixture();
    assert_eq!(video.formats.len(), 10);
}

#[test]
fn video_formats_include_all_types() {
    let video = fixtures::load_video_fixture();

    let audio_count = video
        .formats
        .iter()
        .filter(|f| f.format_type() == FormatType::Audio)
        .count();
    let video_count = video
        .formats
        .iter()
        .filter(|f| f.format_type() == FormatType::Video)
        .count();
    let storyboard_count = video
        .formats
        .iter()
        .filter(|f| f.format_type() == FormatType::Storyboard)
        .count();

    assert_eq!(audio_count, 3);
    assert_eq!(video_count, 4);
    assert_eq!(storyboard_count, 1);
}

#[test]
fn video_chapters_loaded() {
    let video = fixtures::load_video_fixture();
    assert!(video.has_chapters());
    assert_eq!(video.get_chapters().len(), 5);
    assert_eq!(video.get_chapters()[0].title.as_deref(), Some("Introduction"));
}

#[test]
fn video_chapter_at_time() {
    let video = fixtures::load_video_fixture();
    let chapter = video.get_chapter_at_time(15.0).expect("Expected chapter");
    assert_eq!(chapter.title.as_deref(), Some("Introduction"));

    let chapter = video.get_chapter_at_time(300.0).expect("Expected chapter");
    assert_eq!(chapter.title.as_deref(), Some("How the virus works"));

    assert!(video.get_chapter_at_time(3000.0).is_none());
}

#[test]
fn video_heatmap_loaded() {
    let video = fixtures::load_video_fixture();
    let heatmap = video.get_heatmap().expect("Expected heatmap");
    assert_eq!(heatmap.points().len(), 5);
}

#[test]
fn video_heatmap_most_engaged() {
    let video = fixtures::load_video_fixture();
    let heatmap = video.get_heatmap().unwrap();
    let most = heatmap.most_engaged_segment().unwrap();
    assert!((most.value - 0.4601123901718956).abs() < 1e-10);
    assert!((most.start_time - 2457.18).abs() < 1e-10);
}

#[test]
fn video_heatmap_highly_engaged() {
    let video = fixtures::load_video_fixture();
    let heatmap = video.get_heatmap().unwrap();
    let segments = heatmap.get_highly_engaged_segments(0.4);
    assert_eq!(segments.len(), 2); // 0.4555 and 0.4601
}

#[test]
fn video_thumbnails_loaded() {
    let video = fixtures::load_video_fixture();
    assert_eq!(video.thumbnails.len(), 5);
}

#[test]
fn video_display() {
    let video = fixtures::load_video_fixture();
    let display = format!("{}", video);
    assert!(display.contains("gXtp6C-3JKo"));
    assert!(display.contains("The scariest virus in History"));
}

#[test]
fn video_hash_uses_identity_fields() {
    let v1 = fixtures::load_video_fixture();
    let mut v2 = fixtures::load_video_fixture();
    v2.view_count = Some(9999);

    let hash1 = {
        let mut h = DefaultHasher::new();
        v1.hash(&mut h);
        h.finish()
    };
    let hash2 = {
        let mut h = DefaultHasher::new();
        v2.hash(&mut h);
        h.finish()
    };
    // Hash should be the same because identity fields are the same
    assert_eq!(hash1, hash2);
}

// ============================== Live video fixture ==============================

#[test]
fn live_video_fixture_is_live() {
    let json = fixtures::load_json_string("live_video.json");
    let video: Video = serde_json::from_str(&json).expect("Failed to load live_video.json");

    assert_eq!(video.id, "Z-Nwo-ypKtM");
    assert_eq!(video.is_live, Some(true));
    assert!(video.duration.is_none(), "Live videos should not have a duration");
}

// ============================== Reel fixture ==============================

#[test]
fn reel_fixture_deserializes() {
    let video = fixtures::load_reel_fixture();
    assert_eq!(video.id, "DVWFcoHjsnI");
    assert_eq!(video.channel.as_deref(), Some("dustinmotors"));
    assert_eq!(video.formats.len(), 7);
    assert_eq!(video.duration, Some(12));
}

// ============================== Short video fixture ==============================

#[test]
fn short_video_fixture_is_short() {
    let video = fixtures::load_short_video_fixture();
    assert_eq!(video.id, "wBe97k57KxY");
    assert_eq!(video.media_type.as_deref(), Some("short"));
    assert_eq!(video.duration, Some(77));
    assert_eq!(video.channel.as_deref(), Some("Underscore_"));
}

// ============================== Twitch live fixture ==============================

#[test]
fn twitch_live_fixture_is_live() {
    let video = fixtures::load_twitch_live_fixture();
    assert_eq!(video.id, "316138848867");
    assert_eq!(video.is_live, Some(true));
    assert!(video.duration.is_none());
    assert_eq!(video.uploader.as_deref(), Some("samueletienne"));
}

// ============================== Video helper methods ==============================

#[test]
fn video_has_chapters_method() {
    let video = fixtures::load_video_fixture();
    assert!(video.has_chapters());
}

#[test]
fn video_get_chapters() {
    let video = fixtures::load_video_fixture();
    let chapters = video.get_chapters();
    assert!(!chapters.is_empty());
}

#[test]
fn video_has_heatmap() {
    let video = fixtures::load_video_fixture();
    assert!(video.get_heatmap().is_some());
}

// ============================== has_drm ==============================

#[test]
fn format_has_drm_none() {
    let video = fixtures::load_video_fixture();
    use yt_dlp::model::DrmStatus;
    // Regular formats should not have DRM
    for format in &video.formats {
        assert!(format.has_drm.is_none() || format.has_drm == Some(DrmStatus::No));
    }
}

#[test]
fn drm_video_fixture_has_drm_formats() {
    use yt_dlp::model::DrmStatus;
    let json = fixtures::load_json_string("drm_video.json");
    let video: Video = serde_json::from_str(&json).expect("Failed to load drm_video.json");

    let drm_formats: Vec<_> = video
        .formats
        .iter()
        .filter(|f| f.has_drm.is_some() && f.has_drm != Some(DrmStatus::No))
        .collect();
    assert!(!drm_formats.is_empty(), "DRM fixture should have DRM-protected formats");
}

// ============================== Serde round-trip ==============================

#[test]
fn video_serde_round_trip() {
    let original = fixtures::load_video_fixture();
    let serialized = serde_json::to_string(&original).expect("Serialize failed");
    let deserialized: Video = serde_json::from_str(&serialized).expect("Deserialize failed");

    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.title, deserialized.title);
    assert_eq!(original.channel, deserialized.channel);
    assert_eq!(original.formats.len(), deserialized.formats.len());
    assert_eq!(original.tags, deserialized.tags);
    assert_eq!(original.chapters.len(), deserialized.chapters.len());
}
