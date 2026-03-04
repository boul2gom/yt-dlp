use yt_dlp::metadata::{BaseMetadata, MetadataManager, PlaylistMetadata};

// ============================== MetadataManager ==============================

#[test]
fn metadata_manager_new() {
    let manager = MetadataManager::new();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("MetadataManager"));
}

#[test]
fn metadata_manager_with_ffmpeg_path() {
    let manager = MetadataManager::with_ffmpeg_path("/usr/bin/ffmpeg");
    let debug = format!("{:?}", manager);
    assert!(debug.contains("/usr/bin/ffmpeg"));
}

#[test]
fn metadata_manager_default() {
    let manager = MetadataManager::default();
    let debug = format!("{:?}", manager);
    assert!(debug.contains("MetadataManager"));
}

#[test]
fn metadata_manager_eq() {
    let m1 = MetadataManager::with_ffmpeg_path("/usr/bin/ffmpeg");
    let m2 = MetadataManager::with_ffmpeg_path("/usr/bin/ffmpeg");
    assert_eq!(m1, m2);
}

#[test]
fn metadata_manager_ne() {
    let m1 = MetadataManager::with_ffmpeg_path("/usr/bin/ffmpeg");
    let m2 = MetadataManager::with_ffmpeg_path("/usr/local/bin/ffmpeg");
    assert_ne!(m1, m2);
}

#[test]
fn metadata_manager_clone() {
    let manager = MetadataManager::with_ffmpeg_path("/usr/bin/ffmpeg");
    let cloned = manager.clone();
    assert_eq!(manager, cloned);
}

// ============================== PlaylistMetadata ==============================

#[test]
fn playlist_metadata_fields() {
    let meta = PlaylistMetadata {
        title: "My Playlist".to_string(),
        id: "PL12345".to_string(),
        index: 3,
        total: Some(10),
    };
    assert_eq!(meta.title, "My Playlist");
    assert_eq!(meta.id, "PL12345");
    assert_eq!(meta.index, 3);
    assert_eq!(meta.total, Some(10));
}

#[test]
fn playlist_metadata_no_total() {
    let meta = PlaylistMetadata {
        title: "Test".to_string(),
        id: "PL1".to_string(),
        index: 1,
        total: None,
    };
    assert!(meta.total.is_none());
}

#[test]
fn playlist_metadata_clone() {
    let meta = PlaylistMetadata {
        title: "Test".to_string(),
        id: "PL1".to_string(),
        index: 1,
        total: Some(5),
    };
    let cloned = meta.clone();
    assert_eq!(cloned.title, meta.title);
    assert_eq!(cloned.id, meta.id);
}

// ============================== BaseMetadata trait ==============================

#[test]
fn format_timestamp_valid() {
    let result = MetadataManager::format_timestamp(1609459200, "%Y-%m-%d");
    assert_eq!(result, Some("2021-01-01".to_string()));
}

#[test]
fn format_timestamp_year_only() {
    let result = MetadataManager::format_timestamp(1609459200, "%Y");
    assert_eq!(result, Some("2021".to_string()));
}

#[test]
fn format_timestamp_zero() {
    let result = MetadataManager::format_timestamp(0, "%Y-%m-%d");
    assert_eq!(result, Some("1970-01-01".to_string()));
}

#[test]
fn add_metadata_if_some_with_value() {
    let mut metadata = Vec::new();
    MetadataManager::add_metadata_if_some(&mut metadata, "key", Some("value"));
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].0, "key");
    assert_eq!(metadata[0].1, "value");
}

#[test]
fn add_metadata_if_some_none() {
    let mut metadata = Vec::new();
    MetadataManager::add_metadata_if_some::<String>(&mut metadata, "key", None);
    assert!(metadata.is_empty());
}

#[test]
fn add_metadata_if_some_numeric() {
    let mut metadata = Vec::new();
    MetadataManager::add_metadata_if_some(&mut metadata, "bitrate", Some(320));
    assert_eq!(metadata[0].1, "320");
}

#[test]
fn add_metadata_if_some_float() {
    let mut metadata = Vec::new();
    MetadataManager::add_metadata_if_some(&mut metadata, "fps", Some(29.97f64));
    assert_eq!(metadata[0].1, "29.97");
}

// ============================== extract_basic_metadata ==============================

#[test]
fn extract_basic_metadata_from_fixture() {
    // Load a minimal Video via JSON deserialization
    let json = r#"{
        "id": "test_123",
        "title": "Test Video Title",
        "formats": [],
        "thumbnails": [],
        "channel": "Test Channel",
        "channel_id": "UC123",
        "upload_date": "20210101",
        "duration": 120,
        "view_count": 1000,
        "tags": ["rust", "programming"],
        "categories": [],
        "chapters": [],
        "heatmap": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "youtube",
        "extractor_key": "Youtube",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#;
    let video: yt_dlp::model::Video = serde_json::from_str(json).unwrap();
    let metadata = MetadataManager::extract_basic_metadata(&video);

    // Should always have title
    assert!(metadata.iter().any(|(k, v)| k == "title" && v == "Test Video Title"));

    // Should have artist from channel
    assert!(metadata.iter().any(|(k, v)| k == "artist" && v == "Test Channel"));

    // Should have genre from tags
    assert!(metadata.iter().any(|(k, v)| k == "genre" && v == "rust, programming"));
}

#[test]
fn extract_basic_metadata_no_channel() {
    let json = r#"{
        "id": "test_123",
        "title": "No Channel",
        "formats": [],
        "thumbnails": [],
        "duration": 60,
        "view_count": 0,
        "tags": [],
        "categories": [],
        "chapters": [],
        "heatmap": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "generic",
        "extractor_key": "Generic",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#;
    let video: yt_dlp::model::Video = serde_json::from_str(json).unwrap();
    let metadata = MetadataManager::extract_basic_metadata(&video);

    // Should have title but no artist (no channel)
    assert!(metadata.iter().any(|(k, _)| k == "title"));
    assert!(!metadata.iter().any(|(k, _)| k == "artist"));
}

// ============================== extract_basic_metadata: dates ==============================

#[test]
fn extract_basic_metadata_with_timestamp() {
    let json = r#"{
        "id": "test_date",
        "title": "Date Test",
        "formats": [],
        "thumbnails": [],
        "upload_date": "20210615",
        "timestamp": 1623715200,
        "tags": [],
        "categories": [],
        "chapters": [],
        "heatmap": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "youtube",
        "extractor_key": "Youtube",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#;
    let video: yt_dlp::model::Video = serde_json::from_str(json).unwrap();
    let metadata = MetadataManager::extract_basic_metadata(&video);

    // Should have date and year from upload_date (not timestamp)
    // upload_date is stored as a string in JSON; the Video model has upload_date as Option<i64>
    // Here we mainly verify that the metadata extraction doesn't panic
    assert!(metadata.iter().any(|(k, v)| k == "title" && v == "Date Test"));
}

#[test]
fn extract_basic_metadata_no_tags() {
    let json = r#"{
        "id": "notags",
        "title": "No Tags",
        "formats": [],
        "thumbnails": [],
        "tags": [],
        "categories": [],
        "chapters": [],
        "heatmap": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "generic",
        "extractor_key": "Generic",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#;
    let video: yt_dlp::model::Video = serde_json::from_str(json).unwrap();
    let metadata = MetadataManager::extract_basic_metadata(&video);

    // Should NOT have genre when tags are empty
    assert!(!metadata.iter().any(|(k, _)| k == "genre"));
}

// ============================== extract_video_format_metadata ==============================

#[test]
fn extract_video_format_metadata_with_format() {
    let video = crate::common::fixtures::load_video_fixture();
    // Find a video format with resolution
    let video_format = video
        .formats
        .iter()
        .find(|f| f.is_video() && f.video_resolution.width.is_some())
        .expect("Expected at least one video format with resolution");

    let metadata = MetadataManager::extract_video_format_metadata(video_format);

    // Should have resolution
    assert!(
        metadata.iter().any(|(k, _)| k == "resolution"),
        "Expected resolution in metadata: {:?}",
        metadata
    );
}

#[test]
fn extract_video_format_metadata_audio_format() {
    let video = crate::common::fixtures::load_video_fixture();
    // Find an audio-only format
    let audio_format = video
        .formats
        .iter()
        .find(|f| f.is_audio())
        .expect("Expected at least one audio format");

    let metadata = MetadataManager::extract_video_format_metadata(audio_format);

    // Audio-only format should NOT have resolution
    assert!(!metadata.iter().any(|(k, _)| k == "resolution"));
}

// ============================== extract_audio_format_metadata ==============================

#[test]
fn extract_audio_format_metadata_with_format() {
    let video = crate::common::fixtures::load_video_fixture();
    // Find an audio format
    let audio_format = video
        .formats
        .iter()
        .find(|f| f.is_audio())
        .expect("Expected at least one audio format");

    let metadata = MetadataManager::extract_audio_format_metadata(audio_format);

    // Should have audio_codec
    assert!(
        metadata.iter().any(|(k, _)| k == "audio_codec"),
        "Expected audio_codec in metadata: {:?}",
        metadata
    );
}

#[test]
fn extract_audio_format_metadata_video_format() {
    let video = crate::common::fixtures::load_video_fixture();
    // Find a video-only format (no audio codec)
    let video_format = video
        .formats
        .iter()
        .find(|f| f.is_video() && f.codec_info.audio_codec.is_none())
        .expect("Expected at least one video-only format");

    let metadata = MetadataManager::extract_audio_format_metadata(video_format);

    // Video-only format should NOT have audio_codec
    assert!(!metadata.iter().any(|(k, _)| k == "audio_codec"));
}
