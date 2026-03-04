use std::cmp::Ordering;

use ordered_float::OrderedFloat;
use pretty_assertions::assert_eq;
use yt_dlp::VideoSelection;
use yt_dlp::model::Video;
use yt_dlp::model::format::{
    CodecInfo, DownloadInfo, Extension, FileInfo, Format, HttpHeaders, Protocol, QualityInfo, RatesInfo,
    StoryboardInfo, VideoResolution,
};
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, StoryboardQuality, ThumbnailQuality, VideoCodecPreference, VideoQuality,
};
use yt_dlp::model::thumbnail::Thumbnail;

// ============================== Helpers ==============================

fn make_video_format(id: &str, vcodec: &str, height: u32, fps: f64, vbr: f64, quality: f64) -> Format {
    Format {
        format: format!("{} - {}p", id, height),
        format_id: id.to_string(),
        format_note: Some(format!("{}p", height)),
        protocol: Protocol::Https,
        language: None,
        has_drm: None,
        container: None,
        available_at: None,
        language_preference: None,
        source_preference: None,
        codec_info: CodecInfo {
            audio_codec: None,
            video_codec: Some(vcodec.to_string()),
            audio_ext: Extension::default(),
            video_ext: Extension::Mp4,
            audio_channels: None,
            asr: None,
        },
        video_resolution: VideoResolution {
            width: Some(height * 16 / 9),
            height: Some(height),
            resolution: Some(format!("{}x{}", height * 16 / 9, height)),
            fps: Some(OrderedFloat(fps)),
            aspect_ratio: Some(OrderedFloat(16.0 / 9.0)),
        },
        download_info: DownloadInfo {
            url: Some(format!("https://example.com/{}.mp4", id)),
            ext: Extension::Mp4,
            http_headers: HttpHeaders::browser_defaults("test-agent".to_string()),
            manifest_url: None,
            downloader_options: None,
        },
        quality_info: QualityInfo {
            quality: Some(OrderedFloat(quality)),
            dynamic_range: None,
        },
        file_info: FileInfo {
            filesize_approx: None,
            filesize: None,
        },
        storyboard_info: StoryboardInfo {
            rows: None,
            columns: None,
            fragments: None,
        },
        rates_info: RatesInfo {
            video_rate: Some(OrderedFloat(vbr)),
            audio_rate: None,
            total_rate: None,
        },
        video_id: None,
    }
}

fn make_audio_format(id: &str, acodec: &str, abr: f64, asr: i64, channels: i64, quality: f64) -> Format {
    Format {
        format: format!("{} - audio only", id),
        format_id: id.to_string(),
        format_note: Some("audio only".to_string()),
        protocol: Protocol::Https,
        language: None,
        has_drm: None,
        container: None,
        available_at: None,
        language_preference: None,
        source_preference: None,
        codec_info: CodecInfo {
            audio_codec: Some(acodec.to_string()),
            video_codec: None,
            audio_ext: Extension::M4A,
            video_ext: Extension::default(),
            audio_channels: Some(channels),
            asr: Some(asr),
        },
        video_resolution: VideoResolution {
            width: None,
            height: None,
            resolution: Some("audio only".to_string()),
            fps: None,
            aspect_ratio: None,
        },
        download_info: DownloadInfo {
            url: Some(format!("https://example.com/{}.m4a", id)),
            ext: Extension::M4A,
            http_headers: HttpHeaders::browser_defaults("test-agent".to_string()),
            manifest_url: None,
            downloader_options: None,
        },
        quality_info: QualityInfo {
            quality: Some(OrderedFloat(quality)),
            dynamic_range: None,
        },
        file_info: FileInfo {
            filesize_approx: None,
            filesize: None,
        },
        storyboard_info: StoryboardInfo {
            rows: None,
            columns: None,
            fragments: None,
        },
        rates_info: RatesInfo {
            video_rate: None,
            audio_rate: Some(OrderedFloat(abr)),
            total_rate: None,
        },
        video_id: None,
    }
}

fn make_test_video(formats: Vec<Format>) -> Video {
    Video {
        id: "test_selection_video".to_string(),
        title: "Selection Test Video".to_string(),
        formats,
        ..serde_json::from_str(&minimal_video_json()).unwrap()
    }
}

fn minimal_video_json() -> String {
    r#"{
        "id": "test_selection_video",
        "title": "Selection Test Video",
        "formats": [],
        "thumbnails": [],
        "tags": [],
        "categories": [],
        "chapters": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "test",
        "extractor_key": "Test",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#
    .to_string()
}

fn make_video_with_thumbnails(thumbnails: Vec<Thumbnail>) -> Video {
    let json = r#"{
        "id": "thumb_video",
        "title": "Thumbnail Test Video",
        "formats": [],
        "thumbnails": [],
        "tags": [],
        "categories": [],
        "chapters": [],
        "age_limit": 0,
        "live_status": "not_live",
        "playable_in_embed": true,
        "extractor": "test",
        "extractor_key": "Test",
        "_version": {
            "version": "2024.01.01",
            "release_git_head": "abc123",
            "repository": "yt-dlp/yt-dlp"
        }
    }"#;
    let mut video: Video = serde_json::from_str(json).unwrap();
    video.thumbnails = thumbnails;
    video
}

// ============================== best_video_format ==============================

#[test]
fn best_video_format_selects_highest_quality() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v3", "avc1", 720, 30.0, 2500.0, 7.0),
    ]);

    let best = video.best_video_format().unwrap();
    assert_eq!(best.format_id, "v2");
}

#[test]
fn best_video_format_tiebreak_by_height() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 720, 30.0, 2500.0, 7.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 7.0),
    ]);

    let best = video.best_video_format().unwrap();
    assert_eq!(best.format_id, "v2");
}

#[test]
fn best_video_format_tiebreak_by_fps() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v2", "avc1", 1080, 60.0, 5000.0, 9.0),
    ]);

    let best = video.best_video_format().unwrap();
    assert_eq!(best.format_id, "v2");
}

#[test]
fn best_video_format_tiebreak_by_vbr() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 1080, 60.0, 5000.0, 9.0),
        make_video_format("v2", "avc1", 1080, 60.0, 8000.0, 9.0),
    ]);

    let best = video.best_video_format().unwrap();
    assert_eq!(best.format_id, "v2");
}

#[test]
fn best_video_format_empty_formats() {
    let video = make_test_video(vec![]);
    assert!(video.best_video_format().is_none());
}

#[test]
fn best_video_format_no_video_formats() {
    let video = make_test_video(vec![make_audio_format("a1", "opus", 128.0, 48000, 2, 5.0)]);
    assert!(video.best_video_format().is_none());
}

// ============================== worst_video_format ==============================

#[test]
fn worst_video_format_selects_lowest_quality() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v3", "avc1", 720, 30.0, 2500.0, 7.0),
    ]);

    let worst = video.worst_video_format().unwrap();
    assert_eq!(worst.format_id, "v1");
}

// ============================== best_audio_format ==============================

#[test]
fn best_audio_format_selects_highest_quality() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a3", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let best = video.best_audio_format().unwrap();
    assert_eq!(best.format_id, "a3");
}

#[test]
fn best_audio_format_tiebreak_by_abr() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a2", "opus", 256.0, 48000, 2, 7.0),
    ]);

    let best = video.best_audio_format().unwrap();
    assert_eq!(best.format_id, "a2");
}

#[test]
fn best_audio_format_tiebreak_by_sample_rate() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 44100, 2, 7.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
    ]);

    let best = video.best_audio_format().unwrap();
    assert_eq!(best.format_id, "a2");
}

#[test]
fn best_audio_format_tiebreak_by_channels() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a2", "opus", 128.0, 48000, 6, 7.0),
    ]);

    let best = video.best_audio_format().unwrap();
    assert_eq!(best.format_id, "a2");
}

#[test]
fn best_audio_format_empty() {
    let video = make_test_video(vec![]);
    assert!(video.best_audio_format().is_none());
}

#[test]
fn best_audio_format_no_audio() {
    let video = make_test_video(vec![make_video_format("v1", "avc1", 1080, 30.0, 5000.0, 9.0)]);
    assert!(video.best_audio_format().is_none());
}

// ============================== worst_audio_format ==============================

#[test]
fn worst_audio_format_selects_lowest() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a3", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let worst = video.worst_audio_format().unwrap();
    assert_eq!(worst.format_id, "a1");
}

// ============================== compare_video_formats ==============================

#[test]
fn compare_video_formats_ordering() {
    let video = make_test_video(vec![]);
    let low = make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0);
    let high = make_video_format("v2", "avc1", 1080, 60.0, 5000.0, 9.0);

    assert_eq!(video.compare_video_formats(&low, &high), Ordering::Less);
    assert_eq!(video.compare_video_formats(&high, &low), Ordering::Greater);
    assert_eq!(video.compare_video_formats(&low, &low), Ordering::Equal);
}

// ============================== compare_audio_formats ==============================

#[test]
fn compare_audio_formats_ordering() {
    let video = make_test_video(vec![]);
    let low = make_audio_format("a1", "opus", 48.0, 44100, 2, 3.0);
    let high = make_audio_format("a2", "opus", 256.0, 48000, 6, 10.0);

    assert_eq!(video.compare_audio_formats(&low, &high), Ordering::Less);
    assert_eq!(video.compare_audio_formats(&high, &low), Ordering::Greater);
    assert_eq!(video.compare_audio_formats(&low, &low), Ordering::Equal);
}

// ============================== select_video_format ==============================

#[test]
fn select_video_format_best() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v3", "vp9", 720, 30.0, 2500.0, 7.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Best, VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v2");
}

#[test]
fn select_video_format_worst() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 9.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Worst, VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v1");
}

#[test]
fn select_video_format_high_targets_1080p() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 720, 30.0, 2500.0, 7.0),
        make_video_format("v2", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v3", "avc1", 1440, 30.0, 8000.0, 10.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::High, VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v2");
}

#[test]
fn select_video_format_medium_targets_720p() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 720, 30.0, 2500.0, 7.0),
        make_video_format("v3", "avc1", 1080, 30.0, 5000.0, 9.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Medium, VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v2");
}

#[test]
fn select_video_format_low_targets_480p() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 360, 30.0, 500.0, 3.0),
        make_video_format("v2", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v3", "avc1", 720, 30.0, 2500.0, 7.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Low, VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v2");
}

#[test]
fn select_video_format_custom_height() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 720, 30.0, 2500.0, 7.0),
        make_video_format("v3", "avc1", 1440, 30.0, 8000.0, 10.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::CustomHeight(1440), VideoCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "v3");
}

#[test]
fn select_video_format_codec_filter_vp9() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1.64001f", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v2", "vp9", 1080, 30.0, 4000.0, 8.0),
        make_video_format("v3", "vp9", 720, 30.0, 2500.0, 7.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Best, VideoCodecPreference::VP9)
        .unwrap();
    assert_eq!(selected.format_id, "v2");
}

#[test]
fn select_video_format_codec_fallback() {
    // No AV1 formats — should fall back to all formats
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_video_format("v2", "vp9", 720, 30.0, 2500.0, 7.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::Best, VideoCodecPreference::AV1)
        .unwrap();
    assert_eq!(selected.format_id, "v1");
}

#[test]
fn select_video_format_empty() {
    let video = make_test_video(vec![]);
    assert!(
        video
            .select_video_format(VideoQuality::Best, VideoCodecPreference::Any)
            .is_none()
    );
}

// ============================== select_audio_format ==============================

#[test]
fn select_audio_format_best() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a3", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Best, AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a3");
}

#[test]
fn select_audio_format_worst() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Worst, AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a1");
}

#[test]
fn select_audio_format_high_targets_192kbps() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a2", "opus", 192.0, 48000, 2, 8.0),
        make_audio_format("a3", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::High, AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_medium_targets_128kbps() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a3", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Medium, AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_low_targets_96kbps() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 48.0, 48000, 2, 3.0),
        make_audio_format("a2", "opus", 96.0, 48000, 2, 5.0),
        make_audio_format("a3", "opus", 128.0, 48000, 2, 7.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Low, AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_custom_bitrate() {
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a2", "opus", 320.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::CustomBitrate(320), AudioCodecPreference::Any)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_codec_filter_opus() {
    let video = make_test_video(vec![
        make_audio_format("a1", "mp4a.40.2", 128.0, 44100, 2, 7.0),
        make_audio_format("a2", "opus", 128.0, 48000, 2, 7.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Best, AudioCodecPreference::Opus)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_codec_filter_aac() {
    let video = make_test_video(vec![
        make_audio_format("a1", "mp4a.40.2", 128.0, 44100, 2, 7.0),
        make_audio_format("a2", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Best, AudioCodecPreference::AAC)
        .unwrap();
    assert_eq!(selected.format_id, "a1");
}

#[test]
fn select_audio_format_codec_fallback() {
    // No MP3 formats — should fall back to all
    let video = make_test_video(vec![
        make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0),
        make_audio_format("a2", "mp4a.40.2", 256.0, 44100, 2, 10.0),
    ]);

    let selected = video
        .select_audio_format(AudioQuality::Best, AudioCodecPreference::MP3)
        .unwrap();
    assert_eq!(selected.format_id, "a2");
}

#[test]
fn select_audio_format_empty() {
    let video = make_test_video(vec![]);
    assert!(
        video
            .select_audio_format(AudioQuality::Best, AudioCodecPreference::Any)
            .is_none()
    );
}

// ============================== storyboard_formats ==============================

#[test]
fn storyboard_formats_returns_sorted() {
    let mut sb1 = make_video_format("sb1", "none", 90, 0.0, 0.0, -1.0);
    sb1.codec_info.video_codec = None;
    sb1.storyboard_info = StoryboardInfo {
        rows: Some(5),
        columns: Some(5),
        fragments: Some(vec![yt_dlp::model::format::Fragment {
            url: "https://example.com/sb1".to_string(),
            duration: OrderedFloat(10.0),
        }]),
    };
    sb1.video_resolution.width = Some(160);
    sb1.video_resolution.height = Some(90);

    let mut sb2 = make_video_format("sb2", "none", 180, 0.0, 0.0, -1.0);
    sb2.codec_info.video_codec = None;
    sb2.storyboard_info = StoryboardInfo {
        rows: Some(10),
        columns: Some(10),
        fragments: Some(vec![
            yt_dlp::model::format::Fragment {
                url: "https://example.com/sb2a".to_string(),
                duration: OrderedFloat(5.0),
            },
            yt_dlp::model::format::Fragment {
                url: "https://example.com/sb2b".to_string(),
                duration: OrderedFloat(5.0),
            },
        ]),
    };
    sb2.video_resolution.width = Some(320);
    sb2.video_resolution.height = Some(180);

    let video = make_test_video(vec![sb1, sb2.clone()]);
    let storyboards = video.storyboard_formats();

    assert_eq!(storyboards.len(), 2);
    // sb2 has more fragments, so it should be first
    assert_eq!(storyboards[0].format_id, "sb2");
    assert_eq!(storyboards[1].format_id, "sb1");
}

#[test]
fn storyboard_formats_empty_when_none() {
    let video = make_test_video(vec![make_video_format("v1", "avc1", 1080, 30.0, 5000.0, 9.0)]);
    assert!(video.storyboard_formats().is_empty());
}

// ============================== select_storyboard_format ==============================

#[test]
fn select_storyboard_best() {
    let mut sb1 = make_video_format("sb1", "none", 90, 0.0, 0.0, -1.0);
    sb1.codec_info.video_codec = None;
    sb1.storyboard_info.fragments = Some(vec![yt_dlp::model::format::Fragment {
        url: "https://example.com/sb1".to_string(),
        duration: OrderedFloat(10.0),
    }]);
    sb1.video_resolution = VideoResolution {
        width: Some(160),
        height: Some(90),
        resolution: None,
        fps: None,
        aspect_ratio: None,
    };

    let video = make_test_video(vec![sb1]);
    let selected = video.select_storyboard_format(StoryboardQuality::Best);
    assert!(selected.is_some());
}

#[test]
fn select_storyboard_worst() {
    let mut sb = make_video_format("sb1", "none", 90, 0.0, 0.0, -1.0);
    sb.codec_info.video_codec = None;
    sb.storyboard_info.fragments = Some(vec![yt_dlp::model::format::Fragment {
        url: "https://example.com/sb1".to_string(),
        duration: OrderedFloat(10.0),
    }]);
    sb.video_resolution = VideoResolution {
        width: Some(160),
        height: Some(90),
        resolution: None,
        fps: None,
        aspect_ratio: None,
    };

    let video = make_test_video(vec![sb]);
    let selected = video.select_storyboard_format(StoryboardQuality::Worst);
    assert!(selected.is_some());
}

// ============================== select_thumbnail ==============================

#[test]
fn select_thumbnail_best() {
    let video = make_video_with_thumbnails(vec![
        Thumbnail {
            url: "https://example.com/sm.jpg".to_string(),
            preference: -5,
            id: "0".to_string(),
            height: Some(90),
            width: Some(120),
            resolution: Some("120x90".to_string()),
        },
        Thumbnail {
            url: "https://example.com/lg.jpg".to_string(),
            preference: 5,
            id: "1".to_string(),
            height: Some(720),
            width: Some(1280),
            resolution: Some("1280x720".to_string()),
        },
    ]);

    let selected = video.select_thumbnail(ThumbnailQuality::Best).unwrap();
    assert_eq!(selected.id, "1");
}

#[test]
fn select_thumbnail_worst() {
    let video = make_video_with_thumbnails(vec![
        Thumbnail {
            url: "https://example.com/sm.jpg".to_string(),
            preference: -5,
            id: "0".to_string(),
            height: Some(90),
            width: Some(120),
            resolution: Some("120x90".to_string()),
        },
        Thumbnail {
            url: "https://example.com/lg.jpg".to_string(),
            preference: 5,
            id: "1".to_string(),
            height: Some(720),
            width: Some(1280),
            resolution: Some("1280x720".to_string()),
        },
    ]);

    let selected = video.select_thumbnail(ThumbnailQuality::Worst).unwrap();
    assert_eq!(selected.id, "0");
}

#[test]
fn select_thumbnail_minimum_resolution() {
    let video = make_video_with_thumbnails(vec![
        Thumbnail {
            url: "https://example.com/sm.jpg".to_string(),
            preference: -5,
            id: "0".to_string(),
            height: Some(90),
            width: Some(120),
            resolution: Some("120x90".to_string()),
        },
        Thumbnail {
            url: "https://example.com/md.jpg".to_string(),
            preference: 0,
            id: "1".to_string(),
            height: Some(360),
            width: Some(480),
            resolution: Some("480x360".to_string()),
        },
        Thumbnail {
            url: "https://example.com/lg.jpg".to_string(),
            preference: 5,
            id: "2".to_string(),
            height: Some(720),
            width: Some(1280),
            resolution: Some("1280x720".to_string()),
        },
    ]);

    let selected = video
        .select_thumbnail(ThumbnailQuality::MinimumResolution(400, 300))
        .unwrap();
    assert_eq!(selected.id, "1");
}

#[test]
fn select_thumbnail_empty() {
    let video = make_video_with_thumbnails(vec![]);
    assert!(video.select_thumbnail(ThumbnailQuality::Best).is_none());
}

// ============================== Edge cases ==============================

#[test]
fn single_video_format_is_both_best_and_worst() {
    let video = make_test_video(vec![make_video_format("v1", "avc1", 720, 30.0, 2500.0, 7.0)]);

    let best = video.best_video_format().unwrap();
    let worst = video.worst_video_format().unwrap();
    assert_eq!(best.format_id, worst.format_id);
}

#[test]
fn single_audio_format_is_both_best_and_worst() {
    let video = make_test_video(vec![make_audio_format("a1", "opus", 128.0, 48000, 2, 7.0)]);

    let best = video.best_audio_format().unwrap();
    let worst = video.worst_audio_format().unwrap();
    assert_eq!(best.format_id, worst.format_id);
}

#[test]
fn mixed_formats_only_selects_correct_type() {
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 1080, 30.0, 5000.0, 9.0),
        make_audio_format("a1", "opus", 256.0, 48000, 2, 10.0),
    ]);

    let best_video = video.best_video_format().unwrap();
    assert_eq!(best_video.format_id, "v1");

    let best_audio = video.best_audio_format().unwrap();
    assert_eq!(best_audio.format_id, "a1");
}

#[test]
fn select_video_format_fallback_when_no_exact_height() {
    // No 1080p format — should pick closest above or highest available
    let video = make_test_video(vec![
        make_video_format("v1", "avc1", 480, 30.0, 1000.0, 5.0),
        make_video_format("v2", "avc1", 720, 30.0, 2500.0, 7.0),
    ]);

    let selected = video
        .select_video_format(VideoQuality::High, VideoCodecPreference::Any)
        .unwrap();
    // No 1080p, should get the highest available since nothing is >= 1080
    assert_eq!(selected.format_id, "v2");
}
