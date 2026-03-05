use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pretty_assertions::assert_eq;
use yt_dlp::model::caption::{AutomaticCaption, Extension as CaptionExtension, Subtitle};
use yt_dlp::model::chapter::Chapter;
use yt_dlp::model::heatmap::{Heatmap, HeatmapPoint};
use yt_dlp::model::playlist::{Playlist, PlaylistDownloadProgress, PlaylistEntry};
use yt_dlp::model::thumbnail::Thumbnail;
use yt_dlp::model::{ChapterList, ChapterValidation};

use crate::common::fixtures;

// ============================== Chapter ==============================

#[test]
fn chapter_duration() {
    let chapters = fixtures::load_chapters_fixture();
    let first = &chapters[0];
    assert!((first.duration() - 30.0).abs() < f64::EPSILON);
    assert!((first.duration_minutes() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn chapter_contains_timestamp() {
    let chapters = fixtures::load_chapters_fixture();
    let first = &chapters[0];
    assert!(first.contains_timestamp(0.0));
    assert!(first.contains_timestamp(15.0));
    assert!(!first.contains_timestamp(30.0)); // exclusive end
}

#[test]
fn chapter_has_title() {
    let chapters = fixtures::load_chapters_fixture();
    assert!(chapters[0].has_title());
    assert_eq!(chapters[0].title_or("Untitled"), "Introduction");
}

// ============================== Chapter Display ==============================

#[test]
fn chapter_display() {
    let chapter = Chapter {
        start_time: 0.0,
        end_time: 30.0,
        title: Some("Intro".to_string()),
    };
    let display = format!("{:?}", chapter);
    assert!(display.contains("Intro"));
    assert!(display.contains("0.0"));
    assert!(display.contains("30.0"));
}

#[test]
fn chapter_title_or_default() {
    let chapter = Chapter {
        start_time: 0.0,
        end_time: 10.0,
        title: None,
    };
    assert_eq!(chapter.title_or("Untitled"), "Untitled");
}

#[test]
fn chapter_title_contains() {
    let chapter = Chapter {
        start_time: 0.0,
        end_time: 30.0,
        title: Some("Introduction to Rust".to_string()),
    };
    assert!(chapter.title_contains("rust"));
    assert!(chapter.title_contains("INTRODUCTION"));
    assert!(!chapter.title_contains("python"));
}

#[test]
fn chapter_title_matches() {
    let chapter = Chapter {
        start_time: 0.0,
        end_time: 30.0,
        title: Some("Intro".to_string()),
    };
    assert!(chapter.title_matches("intro"));
    assert!(chapter.title_matches("INTRO"));
    assert!(!chapter.title_matches("Introduction"));
}

#[test]
fn chapter_title_starts_with() {
    let chapter = Chapter {
        start_time: 0.0,
        end_time: 30.0,
        title: Some("Introduction to Rust".to_string()),
    };
    assert!(chapter.title_starts_with("intro"));
    assert!(!chapter.title_starts_with("rust"));
}

#[test]
fn chapter_duration_in_range() {
    let chapter = Chapter {
        start_time: 10.0,
        end_time: 40.0,
        title: None,
    };
    assert!(chapter.duration_in_range(20.0, 40.0));
    assert!(!chapter.duration_in_range(31.0, 60.0));
}

#[test]
fn chapter_serde_round_trip() {
    let chapter = Chapter {
        start_time: 10.0,
        end_time: 50.0,
        title: Some("Main Content".to_string()),
    };
    let json = serde_json::to_string(&chapter).unwrap();
    let back: Chapter = serde_json::from_str(&json).unwrap();
    assert!((chapter.start_time - back.start_time).abs() < f64::EPSILON);
    assert!((chapter.end_time - back.end_time).abs() < f64::EPSILON);
    assert_eq!(chapter.title, back.title);
}

// ============================== Playlist ==============================

#[test]
fn playlist_deserialize() {
    let playlist = fixtures::load_playlist_fixture();
    assert_eq!(playlist.id, "PLhixgUqwRTjwvBI-hmbZ2rpkAl4lutnJG");
    assert_eq!(playlist.title, "Minecraft:HACKED");
    assert_eq!(playlist.entries.len(), 5);
}

#[test]
fn playlist_entry_count() {
    let playlist = fixtures::load_playlist_fixture();
    assert_eq!(playlist.entry_count(), 5);
}

#[test]
fn playlist_is_complete() {
    let playlist = fixtures::load_playlist_fixture();
    assert!(playlist.is_complete());
}

#[test]
fn playlist_get_entry_by_index() {
    let playlist = fixtures::load_playlist_fixture();
    let entry = playlist.get_entry_by_index(0).unwrap();
    assert_eq!(entry.id, "Ekcseve-mOg");
    assert!(playlist.get_entry_by_index(10).is_none());
}

#[test]
fn playlist_search_entries_by_title() {
    let playlist = fixtures::load_playlist_fixture();
    let results = playlist.search_entries_by_title("hacking");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, "Ekcseve-mOg");
}

#[test]
fn playlist_filter_by_duration() {
    let playlist = fixtures::load_playlist_fixture();
    let short = playlist.filter_by_duration(None, Some(100.0));
    assert_eq!(short.len(), 1); // 56s video (BW9ybETXAM8)
    let long = playlist.filter_by_duration(Some(1200.0), None);
    assert_eq!(long.len(), 2); // 1343s + 1520s videos
}

#[test]
fn playlist_entry_is_available() {
    let playlist = fixtures::load_playlist_fixture();
    // Entries without availability are considered available
    assert!(playlist.entries[0].is_available());
}

#[test]
fn playlist_display() {
    let playlist = fixtures::load_playlist_fixture();
    let display = format!("{}", playlist);
    assert!(display.contains("PLhixgUqwRTjwvBI-hmbZ2rpkAl4lutnJG"));
    assert!(display.contains("Minecraft:HACKED"));
}

// ============================== Playlist serde round-trip ==============================

#[test]
fn playlist_serde_round_trip() {
    let original = fixtures::load_playlist_fixture();
    let serialized = serde_json::to_string(&original).expect("Serialize failed");
    let deserialized: Playlist = serde_json::from_str(&serialized).expect("Deserialize failed");

    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.title, deserialized.title);
    assert_eq!(original.description, deserialized.description);
    assert_eq!(original.uploader, deserialized.uploader);
    assert_eq!(original.entries.len(), deserialized.entries.len());
    assert_eq!(original.video_count, deserialized.video_count);
}

// ============================== PlaylistEntry fields ==============================

#[test]
fn playlist_entry_fields() {
    let playlist = fixtures::load_playlist_fixture();
    let entry = &playlist.entries[0];
    assert_eq!(entry.id, "Ekcseve-mOg");
    assert_eq!(entry.title, "I Spent 100 Days Hacking Minecraft");
    assert!(!entry.url.is_empty());
}

#[test]
fn playlist_entry_duration_minutes() {
    let playlist = fixtures::load_playlist_fixture();
    let entry = &playlist.entries[0];
    let minutes = entry.duration_minutes();
    assert!(minutes.is_some());
    assert!((minutes.unwrap() - 19.65).abs() < 0.01); // 1179s = 19.65 minutes
}

#[test]
fn playlist_entry_display() {
    let playlist = fixtures::load_playlist_fixture();
    let entry = &playlist.entries[0];
    let display = format!("{}", entry);
    assert!(display.contains("Ekcseve-mOg"));
    assert!(display.contains("I Spent 100 Days Hacking Minecraft"));
}

#[test]
fn playlist_entry_hash() {
    let playlist = fixtures::load_playlist_fixture();
    let hash1 = {
        let mut h = DefaultHasher::new();
        playlist.entries[0].hash(&mut h);
        h.finish()
    };
    let hash2 = {
        let mut h = DefaultHasher::new();
        playlist.entries[1].hash(&mut h);
        h.finish()
    };
    assert_ne!(hash1, hash2);
}

#[test]
fn playlist_available_entries() {
    let playlist = fixtures::load_playlist_fixture();
    let available = playlist.available_entries();
    // Entries without availability info are considered available
    assert!(!available.is_empty());
}

#[test]
fn playlist_get_entries_in_range() {
    let playlist = fixtures::load_playlist_fixture();
    let range = playlist.get_entries_in_range(0, 1);
    assert_eq!(range.len(), 2);
}

#[test]
fn playlist_get_entries_in_range_out_of_bounds() {
    let playlist = fixtures::load_playlist_fixture();
    let range = playlist.get_entries_in_range(100, 200);
    assert!(range.is_empty());
}

#[test]
fn playlist_filter_by_uploader() {
    let playlist = fixtures::load_playlist_fixture();
    let results = playlist.filter_by_uploader("LiveOverflow");
    assert_eq!(results.len(), 5);
}

#[test]
fn playlist_hash() {
    let p1 = fixtures::load_playlist_fixture();
    let p2 = fixtures::load_playlist_fixture();
    let hash1 = {
        let mut h = DefaultHasher::new();
        p1.hash(&mut h);
        h.finish()
    };
    let hash2 = {
        let mut h = DefaultHasher::new();
        p2.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash1, hash2);
}

// ============================== PlaylistDownloadProgress ==============================

#[test]
fn compute_playlist_download_progress_percentage_correctly() {
    let entry = PlaylistEntry {
        id: "vid1".to_string(),
        title: "Test".to_string(),
        url: "https://example.com".to_string(),
        index: Some(1),
        duration: Some(60.0),
        thumbnail: None,
        uploader: None,
        channel_id: None,
        availability: None,
    };
    let progress = PlaylistDownloadProgress {
        entry,
        result: Ok(std::path::PathBuf::from("/tmp/video.mp4")),
        completed: 5,
        total: 10,
    };
    assert!((progress.percentage() - 50.0).abs() < f64::EPSILON);
    assert!(progress.is_success());
    assert!(!progress.is_failure());
}

#[test]
fn compute_playlist_download_progress_zero_when_total_zero() {
    let entry = PlaylistEntry {
        id: "vid1".to_string(),
        title: "Test".to_string(),
        url: "https://example.com".to_string(),
        index: None,
        duration: None,
        thumbnail: None,
        uploader: None,
        channel_id: None,
        availability: None,
    };
    let progress = PlaylistDownloadProgress {
        entry,
        result: Err("failed".to_string()),
        completed: 0,
        total: 0,
    };
    assert!((progress.percentage() - 0.0).abs() < f64::EPSILON);
    assert!(progress.is_failure());
}

#[test]
fn display_playlist_download_progress_shows_completion_ratio() {
    let entry = PlaylistEntry {
        id: "vid1".to_string(),
        title: "A Video".to_string(),
        url: "https://example.com".to_string(),
        index: Some(1),
        duration: None,
        thumbnail: None,
        uploader: None,
        channel_id: None,
        availability: None,
    };
    let progress = PlaylistDownloadProgress {
        entry,
        result: Ok(std::path::PathBuf::from("/tmp/video.mp4")),
        completed: 3,
        total: 5,
    };
    let display = format!("{}", progress);
    assert!(display.contains("3/5"));
}

// ============================== Thumbnail serde round-trip ==============================

#[test]
fn thumbnail_serde_round_trip() {
    let thumb = Thumbnail {
        url: "https://example.com/thumb.jpg".to_string(),
        preference: 10,
        id: "thumb1".to_string(),
        height: Some(720),
        width: Some(1280),
        resolution: Some("1280x720".to_string()),
    };
    let json = serde_json::to_string(&thumb).unwrap();
    let back: Thumbnail = serde_json::from_str(&json).unwrap();
    assert_eq!(thumb, back);
}

#[test]
fn thumbnail_display() {
    let thumb = Thumbnail {
        url: "https://example.com/thumb.jpg".to_string(),
        preference: 10,
        id: "thumb1".to_string(),
        height: Some(720),
        width: Some(1280),
        resolution: Some("1280x720".to_string()),
    };
    let display = format!("{}", thumb);
    assert!(display.contains("thumb1"));
    assert!(display.contains("1280x720"));
}

#[test]
fn thumbnail_display_no_resolution() {
    let thumb = Thumbnail {
        url: "https://example.com/thumb.jpg".to_string(),
        preference: 0,
        id: "thumb_none".to_string(),
        height: None,
        width: None,
        resolution: None,
    };
    let display = format!("{}", thumb);
    assert!(display.contains("unknown"));
}

#[test]
fn thumbnail_hash() {
    let t1 = Thumbnail {
        url: "https://example.com/a.jpg".to_string(),
        preference: 1,
        id: "a".to_string(),
        height: None,
        width: None,
        resolution: None,
    };
    let t2 = Thumbnail {
        url: "https://example.com/b.jpg".to_string(),
        preference: 2,
        id: "b".to_string(),
        height: None,
        width: None,
        resolution: None,
    };
    let hash1 = {
        let mut h = DefaultHasher::new();
        t1.hash(&mut h);
        h.finish()
    };
    let hash2 = {
        let mut h = DefaultHasher::new();
        t2.hash(&mut h);
        h.finish()
    };
    assert_ne!(hash1, hash2);
}

// ============================== AutomaticCaption serde round-trip ==============================

#[test]
fn automatic_caption_serde_round_trip() {
    let caption = AutomaticCaption {
        extension: CaptionExtension::Vtt,
        url: "https://example.com/captions.vtt".to_string(),
        name: Some("English".to_string()),
    };
    let json = serde_json::to_string(&caption).unwrap();
    let back: AutomaticCaption = serde_json::from_str(&json).unwrap();
    assert_eq!(caption, back);
}

#[test]
fn automatic_caption_display() {
    let caption = AutomaticCaption {
        extension: CaptionExtension::Srt,
        url: "https://example.com/captions.srt".to_string(),
        name: Some("French".to_string()),
    };
    let display = format!("{}", caption);
    assert!(display.contains("French"));
    assert!(display.contains("Srt"));
}

#[test]
fn automatic_caption_display_no_lang() {
    let caption = AutomaticCaption {
        extension: CaptionExtension::Vtt,
        url: "https://example.com/captions.vtt".to_string(),
        name: None,
    };
    let display = format!("{}", caption);
    assert!(display.contains("unknown"));
}

#[test]
fn caption_extension_as_str() {
    assert_eq!(CaptionExtension::Json.as_str(), "json");
    assert_eq!(CaptionExtension::Json3.as_str(), "json3");
    assert_eq!(CaptionExtension::Vtt.as_str(), "vtt");
    assert_eq!(CaptionExtension::Srt.as_str(), "srt");
    assert_eq!(CaptionExtension::Ttml.as_str(), "ttml");
    assert_eq!(CaptionExtension::Ass.as_str(), "ass");
    assert_eq!(CaptionExtension::Ssa.as_str(), "ssa");
    assert_eq!(CaptionExtension::Unknown.as_str(), "unknown");
}

#[test]
fn caption_extension_display() {
    assert_eq!(format!("{}", CaptionExtension::Vtt), "vtt");
    assert_eq!(format!("{}", CaptionExtension::Srt), "srt");
}

#[test]
fn caption_extension_default() {
    assert_eq!(CaptionExtension::default(), CaptionExtension::Vtt);
}

#[test]
fn caption_extension_serde_unknown() {
    let ext: CaptionExtension = serde_json::from_str("\"some_future_format\"").unwrap();
    assert_eq!(ext, CaptionExtension::Unknown);
}

// ============================== Subtitle ==============================

#[test]
fn subtitle_serde_round_trip() {
    let sub = Subtitle {
        language_code: Some("en".to_string()),
        language_name: Some("English".to_string()),
        url: "https://example.com/sub.vtt".to_string(),
        extension: CaptionExtension::Vtt,
        is_automatic: false,
    };
    let json = serde_json::to_string(&sub).unwrap();
    let back: Subtitle = serde_json::from_str(&json).unwrap();
    assert_eq!(sub, back);
}

#[test]
fn subtitle_from_automatic_caption() {
    let caption = AutomaticCaption {
        extension: CaptionExtension::Vtt,
        url: "https://example.com/auto.vtt".to_string(),
        name: Some("English".to_string()),
    };
    let sub = Subtitle::from_automatic_caption(&caption, "en".to_string());
    assert!(sub.is_automatic);
    assert_eq!(sub.language_code, Some("en".to_string()));
    assert_eq!(sub.url, caption.url);
}

#[test]
fn subtitle_is_format() {
    let sub = Subtitle {
        language_code: Some("en".to_string()),
        language_name: None,
        url: "https://example.com/sub.srt".to_string(),
        extension: CaptionExtension::Srt,
        is_automatic: false,
    };
    assert!(sub.is_format(&CaptionExtension::Srt));
    assert!(!sub.is_format(&CaptionExtension::Vtt));
}

#[test]
fn subtitle_file_extension() {
    let sub = Subtitle {
        language_code: None,
        language_name: None,
        url: "https://example.com/sub.ass".to_string(),
        extension: CaptionExtension::Ass,
        is_automatic: false,
    };
    assert_eq!(sub.file_extension(), "ass");
}

#[test]
fn subtitle_display() {
    let sub = Subtitle {
        language_code: Some("fr".to_string()),
        language_name: Some("French".to_string()),
        url: "https://example.com/sub.vtt".to_string(),
        extension: CaptionExtension::Vtt,
        is_automatic: true,
    };
    let display = format!("{}", sub);
    assert!(display.contains("French"));
    assert!(display.contains("vtt"));
    assert!(display.contains("true"));
}

// ============================== Heatmap serde round-trip ==============================

#[test]
fn heatmap_serde_round_trip() {
    let heatmap = Heatmap::new(vec![
        HeatmapPoint {
            start_time: 0.0,
            end_time: 10.0,
            value: 0.5,
        },
        HeatmapPoint {
            start_time: 10.0,
            end_time: 20.0,
            value: 0.9,
        },
    ]);
    let json = serde_json::to_string(&heatmap).unwrap();
    let back: Heatmap = serde_json::from_str(&json).unwrap();
    assert_eq!(heatmap, back);
}

#[test]
fn heatmap_empty() {
    let heatmap = Heatmap::new(vec![]);
    assert!(heatmap.is_empty());
    assert!(heatmap.most_engaged_segment().is_none());
    assert!(heatmap.get_point_at_time(5.0).is_none());
}

#[test]
fn heatmap_display() {
    let heatmap = Heatmap::new(vec![HeatmapPoint {
        start_time: 0.0,
        end_time: 10.0,
        value: 0.5,
    }]);
    let display = format!("{}", heatmap);
    assert!(display.contains("points=1"));
}

#[test]
fn heatmap_point_duration() {
    let point = HeatmapPoint {
        start_time: 5.0,
        end_time: 15.0,
        value: 0.7,
    };
    assert!((point.duration() - 10.0).abs() < f64::EPSILON);
}

#[test]
fn heatmap_point_contains_timestamp() {
    let point = HeatmapPoint {
        start_time: 10.0,
        end_time: 20.0,
        value: 0.5,
    };
    assert!(point.contains_timestamp(10.0));
    assert!(point.contains_timestamp(15.0));
    assert!(!point.contains_timestamp(20.0)); // exclusive end
    assert!(!point.contains_timestamp(9.0));
}

#[test]
fn heatmap_point_display() {
    let point = HeatmapPoint {
        start_time: 1.5,
        end_time: 3.5,
        value: 0.42,
    };
    let display = format!("{}", point);
    assert!(display.contains("1.50"));
    assert!(display.contains("3.50"));
    assert!(display.contains("0.42"));
}

#[test]
fn heatmap_get_point_at_time() {
    let heatmap = Heatmap::new(vec![
        HeatmapPoint {
            start_time: 0.0,
            end_time: 10.0,
            value: 0.3,
        },
        HeatmapPoint {
            start_time: 10.0,
            end_time: 20.0,
            value: 0.8,
        },
    ]);
    let point = heatmap.get_point_at_time(5.0).unwrap();
    assert!((point.value - 0.3).abs() < f64::EPSILON);

    let point = heatmap.get_point_at_time(15.0).unwrap();
    assert!((point.value - 0.8).abs() < f64::EPSILON);

    assert!(heatmap.get_point_at_time(25.0).is_none());
}

// ============================== ChapterList ==============================

// The chapters.json fixture has 5 chapters:
// Introduction (0-30s), Setup (30-60s), Implementation (60-90s), Testing (90-120s), Conclusion (120-150s)

#[test]
fn count_chapter_list_entries_from_fixture() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    assert_eq!(list.count(), 5);
}

#[test]
fn compute_chapter_list_total_duration_from_fixture() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    assert!((list.total_duration() - 150.0).abs() < f64::EPSILON);
}

#[test]
fn search_chapter_list_by_title_finds_match() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    let results = list.search_by_title("implementation");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title.as_deref(), Some("Implementation"));
}

#[test]
fn search_chapter_list_by_title_returns_empty_on_no_match() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    let results = list.search_by_title("nonexistent_xyz");
    assert!(results.is_empty());
}

#[test]
fn find_chapter_list_by_exact_title_returns_match() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    let found = list.find_by_exact_title("introduction");
    assert!(found.is_some());
    assert_eq!(found.unwrap().start_time, 0.0);
}

#[test]
fn find_chapter_list_by_exact_title_returns_none_on_no_match() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    let found = list.find_by_exact_title("nonexistent_xyz");
    assert!(found.is_none());
}

#[test]
fn find_chapter_list_by_title_prefix_returns_match() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    // "Conclusion" starts with "con"
    let results = list.find_by_title_prefix("con");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title.as_deref(), Some("Conclusion"));
}

#[test]
fn find_chapter_list_by_timestamp_returns_matching_chapter() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    // t=50 falls in Setup (30-60s)
    let found = list.find_by_timestamp(50.0);
    assert!(found.is_some());
    assert_eq!(found.unwrap().title.as_deref(), Some("Setup"));
}

#[test]
fn find_chapter_list_by_timestamp_returns_none_when_out_of_range() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    assert!(list.find_by_timestamp(200.0).is_none());
}

#[test]
fn filter_chapter_list_by_duration_returns_all_matching() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    // All 5 fixture chapters are exactly 30s → all match [25, 35]
    let results = list.filter_by_duration(25.0, 35.0);
    assert_eq!(results.len(), 5);
}

#[test]
fn filter_chapter_list_with_titles_returns_all_when_all_titled() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    // All fixture chapters have titles
    assert_eq!(list.with_titles().len(), chapters.len());
}

#[test]
fn filter_chapter_list_with_titles_returns_only_titled() {
    let chapters = vec![
        Chapter {
            start_time: 0.0,
            end_time: 10.0,
            title: Some("With Title".to_string()),
        },
        Chapter {
            start_time: 10.0,
            end_time: 20.0,
            title: None,
        },
    ];
    let list = ChapterList::new(&chapters);
    assert_eq!(list.with_titles().len(), 1);
}

#[test]
fn check_chapter_list_is_sorted_returns_true_for_fixture() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    assert!(list.is_sorted());
}

#[test]
fn check_chapter_list_is_sorted_returns_false_for_unordered() {
    let chapters = vec![
        Chapter {
            start_time: 30.0,
            end_time: 60.0,
            title: None,
        },
        Chapter {
            start_time: 0.0,
            end_time: 30.0,
            title: None,
        },
    ];
    let list = ChapterList::new(&chapters);
    assert!(!list.is_sorted());
}

#[test]
fn check_chapter_list_has_overlaps_returns_false_for_fixture() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    assert!(!list.has_overlaps());
}

#[test]
fn check_chapter_list_has_overlaps_returns_true_for_overlapping() {
    let chapters = vec![
        Chapter {
            start_time: 0.0,
            end_time: 40.0,
            title: None,
        },
        Chapter {
            start_time: 30.0,
            end_time: 60.0,
            title: None,
        },
    ];
    let list = ChapterList::new(&chapters);
    assert!(list.has_overlaps());
}

// ============================== ChapterList::validate ==============================

#[test]
fn validate_chapter_list_returns_valid_for_fixture() {
    let chapters = fixtures::load_chapters_fixture();
    let list = ChapterList::new(&chapters);
    let validation = list.validate();
    assert!(validation.is_valid);
    assert!(validation.errors.is_empty());
}

#[test]
fn validate_chapter_list_returns_valid_for_empty() {
    let chapters: Vec<Chapter> = vec![];
    let list = ChapterList::new(&chapters);
    let validation = list.validate();
    assert!(validation.is_valid);
}

#[test]
fn validate_chapter_list_returns_error_for_inverted_time_range() {
    let chapters = vec![Chapter {
        start_time: 30.0,
        end_time: 10.0, // end < start
        title: Some("Bad".to_string()),
    }];
    let list = ChapterList::new(&chapters);
    let validation = list.validate();
    assert!(!validation.is_valid);
    assert!(validation.errors.iter().any(|e| e.contains("invalid time range")));
}

#[test]
fn validate_chapter_list_produces_warning_for_time_gap() {
    let chapters = vec![
        Chapter {
            start_time: 0.0,
            end_time: 10.0,
            title: Some("First".to_string()),
        },
        Chapter {
            start_time: 20.0, // 10s gap
            end_time: 30.0,
            title: Some("Second".to_string()),
        },
    ];
    let list = ChapterList::new(&chapters);
    let validation = list.validate();
    assert!(validation.is_valid); // gaps are warnings, not errors
    assert!(validation.warnings.iter().any(|w| w.contains("Gap")));
}

#[test]
fn validate_chapter_list_produces_warning_for_missing_title() {
    let chapters = vec![Chapter {
        start_time: 0.0,
        end_time: 60.0,
        title: None,
    }];
    let list = ChapterList::new(&chapters);
    let validation = list.validate();
    assert!(validation.is_valid);
    assert!(validation.warnings.iter().any(|w| w.contains("no title")));
}

// ============================== ChapterValidation ==============================

#[test]
fn build_valid_chapter_validation_has_empty_errors() {
    let v = ChapterValidation::valid();
    assert!(v.is_valid);
    assert!(v.errors.is_empty());
    assert!(v.warnings.is_empty());
    assert!(!v.has_issues());
}

#[test]
fn build_invalid_chapter_validation_sets_errors() {
    let v = ChapterValidation::invalid(vec!["error one".to_string()]);
    assert!(!v.is_valid);
    assert_eq!(v.errors.len(), 1);
    assert!(v.has_issues());
}

#[test]
fn chapter_validation_with_warning() {
    let v = ChapterValidation::valid().with_warning("a warning".to_string());
    assert!(v.is_valid);
    assert_eq!(v.warnings.len(), 1);
    assert!(v.has_issues());
}

#[test]
fn add_warnings_to_chapter_validation_sets_all_warnings() {
    let v = ChapterValidation::valid().with_warnings(vec!["w1".to_string(), "w2".to_string()]);
    assert_eq!(v.warnings.len(), 2);
}

#[test]
fn display_chapter_validation_shows_key_fields() {
    let v = ChapterValidation::invalid(vec!["err".to_string()]).with_warning("warn".to_string());
    let display = format!("{}", v);
    assert!(display.contains("ChapterValidation"));
    assert!(display.contains("false"));
    assert!(display.contains("errors=1"));
    assert!(display.contains("warnings=1"));
}

#[test]
fn display_chapter_shows_times_and_title() {
    let ch = Chapter {
        start_time: 5.5,
        end_time: 15.5,
        title: Some("Test Chapter".to_string()),
    };
    let display = format!("{}", ch);
    assert!(display.contains("Chapter"));
    assert!(display.contains("5.50"));
    assert!(display.contains("15.50"));
    assert!(display.contains("Test Chapter"));
}
