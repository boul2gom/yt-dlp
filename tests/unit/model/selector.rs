use pretty_assertions::assert_eq;
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, FormatPreferences, StoryboardQuality, ThumbnailQuality, VideoCodecPreference,
    VideoQuality, matches_audio_codec, matches_video_codec,
};

// ============================== VideoQuality ==============================

#[test]
fn video_quality_display() {
    assert_eq!(format!("{}", VideoQuality::Best), "Best");
    assert_eq!(format!("{}", VideoQuality::High), "High");
    assert_eq!(format!("{}", VideoQuality::Medium), "Medium");
    assert_eq!(format!("{}", VideoQuality::Low), "Low");
    assert_eq!(format!("{}", VideoQuality::Worst), "Worst");
    assert_eq!(
        format!("{}", VideoQuality::CustomHeight(1080)),
        "CustomHeight(height=1080)"
    );
    assert_eq!(
        format!("{}", VideoQuality::CustomWidth(1920)),
        "CustomWidth(width=1920)"
    );
}

// ============================== AudioQuality ==============================

#[test]
fn audio_quality_display() {
    assert_eq!(format!("{}", AudioQuality::Best), "Best");
    assert_eq!(format!("{}", AudioQuality::High), "High");
    assert_eq!(format!("{}", AudioQuality::Medium), "Medium");
    assert_eq!(format!("{}", AudioQuality::Low), "Low");
    assert_eq!(format!("{}", AudioQuality::Worst), "Worst");
    assert_eq!(
        format!("{}", AudioQuality::CustomBitrate(320)),
        "CustomBitrate(bitrate=320)"
    );
}

// ============================== Codec preferences ==============================

#[test]
fn matches_video_codec_vp9() {
    assert!(matches_video_codec("vp9", &VideoCodecPreference::VP9));
    assert!(matches_video_codec("vp9.0", &VideoCodecPreference::VP9));
    assert!(matches_video_codec("VP9", &VideoCodecPreference::VP9));
    assert!(!matches_video_codec("avc1", &VideoCodecPreference::VP9));
}

#[test]
fn matches_video_codec_avc1() {
    assert!(matches_video_codec("avc1.64001f", &VideoCodecPreference::AVC1));
    assert!(matches_video_codec("h264", &VideoCodecPreference::AVC1));
    assert!(matches_video_codec("H.264", &VideoCodecPreference::AVC1));
    assert!(!matches_video_codec("vp9", &VideoCodecPreference::AVC1));
}

#[test]
fn matches_video_codec_av1() {
    assert!(matches_video_codec("av1", &VideoCodecPreference::AV1));
    assert!(matches_video_codec("av01", &VideoCodecPreference::AV1));
    assert!(!matches_video_codec("avc1", &VideoCodecPreference::AV1));
}

#[test]
fn matches_video_codec_custom() {
    let custom = VideoCodecPreference::Custom("hevc".to_string());
    assert!(matches_video_codec("hevc", &custom));
    assert!(matches_video_codec("HEVC", &custom));
    assert!(!matches_video_codec("vp9", &custom));
}

#[test]
fn matches_video_codec_any() {
    assert!(matches_video_codec("anything", &VideoCodecPreference::Any));
}

#[test]
fn matches_audio_codec_opus() {
    assert!(matches_audio_codec("opus", &AudioCodecPreference::Opus));
    assert!(matches_audio_codec("OPUS", &AudioCodecPreference::Opus));
    assert!(!matches_audio_codec("aac", &AudioCodecPreference::Opus));
}

#[test]
fn matches_audio_codec_aac() {
    assert!(matches_audio_codec("aac", &AudioCodecPreference::AAC));
    assert!(matches_audio_codec("mp4a.40.2", &AudioCodecPreference::AAC));
    assert!(!matches_audio_codec("opus", &AudioCodecPreference::AAC));
}

#[test]
fn matches_audio_codec_mp3() {
    assert!(matches_audio_codec("mp3", &AudioCodecPreference::MP3));
    assert!(!matches_audio_codec("opus", &AudioCodecPreference::MP3));
}

#[test]
fn matches_audio_codec_any() {
    assert!(matches_audio_codec("anything", &AudioCodecPreference::Any));
}

// ============================== FormatPreferences ==============================

#[test]
fn format_preferences_has_any() {
    let empty = FormatPreferences::default();
    assert!(!empty.has_any());

    let with_video = FormatPreferences {
        video_quality: Some(VideoQuality::High),
        ..Default::default()
    };
    assert!(with_video.has_any());

    let with_audio = FormatPreferences {
        audio_quality: Some(AudioQuality::High),
        ..Default::default()
    };
    assert!(with_audio.has_any());
}

#[test]
fn format_preferences_display() {
    let prefs = FormatPreferences {
        video_quality: Some(VideoQuality::High),
        audio_quality: Some(AudioQuality::High),
        ..Default::default()
    };
    let display = format!("{}", prefs);
    assert!(display.contains("High"));
}

// ============================== StoryboardQuality ==============================

#[test]
fn storyboard_quality_display() {
    assert_eq!(format!("{}", StoryboardQuality::Best), "Best");
    assert_eq!(format!("{}", StoryboardQuality::Worst), "Worst");
}

// ============================== ThumbnailQuality ==============================

#[test]
fn thumbnail_quality_display() {
    assert_eq!(format!("{}", ThumbnailQuality::Best), "Best");
    assert_eq!(format!("{}", ThumbnailQuality::Worst), "Worst");
    let min_res = ThumbnailQuality::MinimumResolution(640, 480);
    assert!(format!("{}", min_res).contains("640"));
    assert!(format!("{}", min_res).contains("480"));
}
