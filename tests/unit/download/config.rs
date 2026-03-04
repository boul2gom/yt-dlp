use pretty_assertions::assert_eq;
use yt_dlp::download::config::postprocess::{
    AudioCodec, EncodingPreset, FfmpegFilter, PostProcessConfig, Resolution, VideoCodec, WatermarkPosition,
};
use yt_dlp::download::config::progress::ProgressInfo;
use yt_dlp::download::config::speed_profile::SpeedProfile;
use yt_dlp::download::{DownloadPriority, DownloadStatus, ManagerConfig};

// ============================== SpeedProfile ==============================

#[test]
fn speed_profile_conservative_values() {
    let profile = SpeedProfile::Conservative;
    assert_eq!(profile.max_concurrent_downloads(), 3);
    assert_eq!(profile.segment_size(), 5 * 1024 * 1024);
    assert_eq!(profile.parallel_segments(), 4);
    assert_eq!(profile.max_buffer_size(), 10 * 1024 * 1024);
}

#[test]
fn speed_profile_balanced_values() {
    let profile = SpeedProfile::Balanced;
    assert_eq!(profile.max_concurrent_downloads(), 4);
    assert_eq!(profile.segment_size(), 8 * 1024 * 1024);
    assert_eq!(profile.parallel_segments(), 5);
    assert_eq!(profile.max_buffer_size(), 20 * 1024 * 1024);
}

#[test]
fn speed_profile_aggressive_values() {
    let profile = SpeedProfile::Aggressive;
    assert_eq!(profile.max_concurrent_downloads(), 6);
    assert_eq!(profile.segment_size(), 10 * 1024 * 1024);
    assert_eq!(profile.parallel_segments(), 6);
    assert_eq!(profile.max_buffer_size(), 30 * 1024 * 1024);
}

#[test]
fn speed_profile_default_is_balanced() {
    assert_eq!(SpeedProfile::default(), SpeedProfile::Balanced);
}

#[test]
fn speed_profile_display() {
    assert_eq!(format!("{}", SpeedProfile::Conservative), "Conservative");
    assert_eq!(format!("{}", SpeedProfile::Balanced), "Balanced");
    assert_eq!(format!("{}", SpeedProfile::Aggressive), "Aggressive");
}

#[test]
fn speed_profile_optimal_segments_small_file() {
    let profile = SpeedProfile::Balanced;
    let segment_size = 8 * 1024 * 1024;
    // 5MB file = 1 segment total
    let result = profile.calculate_optimal_segments(5 * 1024 * 1024, segment_size as u64);
    assert_eq!(result, 1);
}

#[test]
fn speed_profile_optimal_segments_medium_file() {
    let profile = SpeedProfile::Balanced;
    let segment_size = 8 * 1024 * 1024;
    // 200MB file
    let result = profile.calculate_optimal_segments(200 * 1024 * 1024, segment_size as u64);
    assert!(result > 1);
    assert!(result <= 20);
}

#[test]
fn speed_profile_optimal_segments_large_file() {
    let profile = SpeedProfile::Aggressive;
    let segment_size = 10 * 1024 * 1024;
    // 3GB file
    let result = profile.calculate_optimal_segments(3 * 1024 * 1024 * 1024, segment_size as u64);
    assert!(result <= 24);
}

#[test]
fn speed_profile_max_parallel_large_files() {
    assert_eq!(SpeedProfile::Conservative.max_parallel_segments_for_large_files(), 16);
    assert_eq!(SpeedProfile::Balanced.max_parallel_segments_for_large_files(), 20);
    assert_eq!(SpeedProfile::Aggressive.max_parallel_segments_for_large_files(), 24);
}

#[test]
fn speed_profile_playlist_concurrent() {
    assert_eq!(SpeedProfile::Conservative.max_playlist_concurrent_downloads(), 2);
    assert_eq!(SpeedProfile::Balanced.max_playlist_concurrent_downloads(), 3);
    assert_eq!(SpeedProfile::Aggressive.max_playlist_concurrent_downloads(), 5);
}

// ============================== ManagerConfig ==============================

#[test]
fn manager_config_default() {
    let config = ManagerConfig::default();
    assert_eq!(config.speed_profile, SpeedProfile::Balanced);
    assert_eq!(config.max_concurrent_downloads, 4);
    assert_eq!(config.retry_attempts, 3);
}

#[test]
fn manager_config_from_speed_profile() {
    let config = ManagerConfig::from_speed_profile(SpeedProfile::Aggressive);
    assert_eq!(config.speed_profile, SpeedProfile::Aggressive);
    assert_eq!(config.max_concurrent_downloads, 6);
}

#[test]
fn manager_config_with_speed_profile() {
    let config = ManagerConfig::default().with_speed_profile(SpeedProfile::Conservative);
    assert_eq!(config.speed_profile, SpeedProfile::Conservative);
    assert_eq!(config.max_concurrent_downloads, 3);
}

#[test]
fn manager_config_display() {
    let config = ManagerConfig::default();
    let display = format!("{}", config);
    assert!(display.contains("Balanced"));
    assert!(display.contains("ManagerConfig"));
}

// ============================== DownloadPriority ==============================

#[test]
fn download_priority_default() {
    assert_eq!(DownloadPriority::default(), DownloadPriority::Normal);
}

#[test]
fn download_priority_from_i32() {
    assert_eq!(DownloadPriority::from_i32(0), DownloadPriority::Low);
    assert_eq!(DownloadPriority::from_i32(1), DownloadPriority::Normal);
    assert_eq!(DownloadPriority::from_i32(2), DownloadPriority::High);
    assert_eq!(DownloadPriority::from_i32(3), DownloadPriority::Critical);
    assert_eq!(DownloadPriority::from_i32(99), DownloadPriority::Normal);
}

#[test]
fn download_priority_display() {
    assert_eq!(format!("{}", DownloadPriority::Low), "Low");
    assert_eq!(format!("{}", DownloadPriority::Normal), "Normal");
    assert_eq!(format!("{}", DownloadPriority::High), "High");
    assert_eq!(format!("{}", DownloadPriority::Critical), "Critical");
}

// ============================== DownloadStatus ==============================

#[test]
fn download_status_display() {
    assert_eq!(format!("{}", DownloadStatus::Queued), "Queued");
    assert_eq!(format!("{}", DownloadStatus::Completed), "Completed");
    assert_eq!(format!("{}", DownloadStatus::Canceled), "Canceled");
    let downloading = DownloadStatus::Downloading {
        downloaded_bytes: 500,
        total_bytes: 1000,
    };
    assert!(format!("{}", downloading).contains("500"));
    let failed = DownloadStatus::Failed {
        reason: "timeout".to_string(),
    };
    assert!(format!("{}", failed).contains("timeout"));
}

// ============================== ProgressInfo ==============================

#[test]
fn progress_info_percentage() {
    let info = ProgressInfo::new(500, 1000);
    assert!((info.percentage() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn progress_info_percentage_zero_total() {
    let info = ProgressInfo::new(0, 0);
    assert!((info.percentage()).abs() < f64::EPSILON);
}

#[test]
fn progress_info_display() {
    let info = ProgressInfo::new(750, 1000);
    let display = format!("{}", info);
    assert!(display.contains("750"));
    assert!(display.contains("1000"));
    assert!(display.contains("75.0%"));
}

// ============================== PostProcessConfig ==============================

#[test]
fn post_process_config_new_is_empty() {
    let config = PostProcessConfig::new();
    assert!(config.is_empty());
}

#[test]
fn post_process_config_builder() {
    let config = PostProcessConfig::new()
        .with_video_codec(VideoCodec::H264)
        .with_audio_codec(AudioCodec::AAC)
        .with_video_bitrate("2M")
        .with_audio_bitrate("128k")
        .with_resolution(Resolution::FullHD)
        .with_framerate(30)
        .with_preset(EncodingPreset::Medium)
        .add_filter(FfmpegFilter::FlipHorizontal);

    assert!(!config.is_empty());
    assert_eq!(config.video_codec, Some(VideoCodec::H264));
    assert_eq!(config.audio_codec, Some(AudioCodec::AAC));
    assert_eq!(config.video_bitrate.as_deref(), Some("2M"));
    assert_eq!(config.audio_bitrate.as_deref(), Some("128k"));
    assert_eq!(config.framerate, Some(30));
    assert_eq!(config.preset, Some(EncodingPreset::Medium));
    assert_eq!(config.filters.len(), 1);
}

#[test]
fn post_process_config_display() {
    let config = PostProcessConfig::new().with_video_codec(VideoCodec::H265);
    let display = format!("{}", config);
    assert!(display.contains("H265"));
}

// ============================== VideoCodec ==============================

#[test]
fn video_codec_ffmpeg_name() {
    assert_eq!(VideoCodec::H264.to_ffmpeg_name(), "libx264");
    assert_eq!(VideoCodec::H265.to_ffmpeg_name(), "libx265");
    assert_eq!(VideoCodec::VP9.to_ffmpeg_name(), "libvpx-vp9");
    assert_eq!(VideoCodec::AV1.to_ffmpeg_name(), "libaom-av1");
    assert_eq!(VideoCodec::Copy.to_ffmpeg_name(), "copy");
}

#[test]
fn video_codec_display() {
    assert_eq!(format!("{}", VideoCodec::H264), "H264");
    assert_eq!(format!("{}", VideoCodec::Copy), "Copy");
}

#[test]
fn video_codec_default_is_copy() {
    assert_eq!(VideoCodec::default(), VideoCodec::Copy);
}

// ============================== AudioCodec ==============================

#[test]
fn audio_codec_ffmpeg_name() {
    assert_eq!(AudioCodec::AAC.to_ffmpeg_name(), "aac");
    assert_eq!(AudioCodec::MP3.to_ffmpeg_name(), "libmp3lame");
    assert_eq!(AudioCodec::Opus.to_ffmpeg_name(), "libopus");
    assert_eq!(AudioCodec::Vorbis.to_ffmpeg_name(), "libvorbis");
    assert_eq!(AudioCodec::Copy.to_ffmpeg_name(), "copy");
}

#[test]
fn audio_codec_default_is_copy() {
    assert_eq!(AudioCodec::default(), AudioCodec::Copy);
}

// ============================== Resolution ==============================

#[test]
fn resolution_dimensions() {
    assert_eq!(Resolution::FullHD.dimensions(), (1920, 1080));
    assert_eq!(Resolution::HD.dimensions(), (1280, 720));
    assert_eq!(Resolution::SD.dimensions(), (854, 480));
    assert_eq!(Resolution::UHD4K.dimensions(), (3840, 2160));
    let custom = Resolution::Custom {
        width: 800,
        height: 600,
    };
    assert_eq!(custom.dimensions(), (800, 600));
}

#[test]
fn resolution_ffmpeg_scale() {
    assert_eq!(Resolution::FullHD.to_ffmpeg_scale(), "1920:1080");
}

#[test]
fn resolution_display() {
    assert_eq!(format!("{}", Resolution::FullHD), "FullHD");
    assert_eq!(format!("{}", Resolution::HD), "HD");
    let custom = Resolution::Custom {
        width: 800,
        height: 600,
    };
    assert!(format!("{}", custom).contains("800"));
}

// ============================== EncodingPreset ==============================

#[test]
fn encoding_preset_ffmpeg_name() {
    assert_eq!(EncodingPreset::UltraFast.to_ffmpeg_name(), "ultrafast");
    assert_eq!(EncodingPreset::Medium.to_ffmpeg_name(), "medium");
    assert_eq!(EncodingPreset::VerySlow.to_ffmpeg_name(), "veryslow");
}

#[test]
fn encoding_preset_default_is_medium() {
    assert_eq!(EncodingPreset::default(), EncodingPreset::Medium);
}

// ============================== FfmpegFilter ==============================

#[test]
fn ffmpeg_filter_to_string() {
    assert_eq!(FfmpegFilter::FlipHorizontal.to_ffmpeg_string(), "hflip");
    assert_eq!(FfmpegFilter::FlipVertical.to_ffmpeg_string(), "vflip");
    assert_eq!(FfmpegFilter::Denoise.to_ffmpeg_string(), "hqdn3d");

    let crop = FfmpegFilter::Crop {
        width: 640,
        height: 480,
        x: 0,
        y: 0,
    };
    assert_eq!(crop.to_ffmpeg_string(), "crop=640:480:0:0");

    let blur = FfmpegFilter::Blur { radius: 5 };
    assert_eq!(blur.to_ffmpeg_string(), "boxblur=5:5");

    let custom = FfmpegFilter::Custom {
        filter: "scale=1280:-1".to_string(),
    };
    assert_eq!(custom.to_ffmpeg_string(), "scale=1280:-1");
}

#[test]
fn ffmpeg_filter_display() {
    assert_eq!(format!("{}", FfmpegFilter::FlipHorizontal), "FlipHorizontal");
    assert_eq!(format!("{}", FfmpegFilter::Denoise), "Denoise");
    assert_eq!(format!("{}", FfmpegFilter::Sharpen), "Sharpen");
}

// ============================== WatermarkPosition ==============================

#[test]
fn watermark_position_ffmpeg() {
    assert!(WatermarkPosition::TopLeft.to_ffmpeg_position().contains("x=10"));
    assert!(WatermarkPosition::BottomRight.to_ffmpeg_position().contains("W-w"));
    assert!(WatermarkPosition::Center.to_ffmpeg_position().contains("(W-w)/2"));
    let custom = WatermarkPosition::Custom { x: 50, y: 100 };
    assert_eq!(custom.to_ffmpeg_position(), "x=50:y=100");
}
