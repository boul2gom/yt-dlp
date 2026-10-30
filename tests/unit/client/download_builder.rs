use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use yt_dlp::Downloader;
use yt_dlp::client::deps::Libraries;
use yt_dlp::client::download_builder::{DownloadMode, is_audio_extension};

#[test]
fn is_audio_extension_detection() {
    assert!(is_audio_extension("mp3"));
    assert!(is_audio_extension(".mp3"));
    assert!(is_audio_extension("MP3"));
    assert!(is_audio_extension("m4a"));
    assert!(is_audio_extension("aac"));
    assert!(is_audio_extension("flac"));
    assert!(is_audio_extension("wav"));
    assert!(is_audio_extension("opus"));
    assert!(is_audio_extension("ogg"));
    assert!(is_audio_extension("alac"));

    assert!(!is_audio_extension("mp4"));
    assert!(!is_audio_extension("mkv"));
    assert!(!is_audio_extension("webm"));
    assert!(!is_audio_extension("avi"));
    assert!(!is_audio_extension("mov"));
    assert!(!is_audio_extension("txt"));
}

#[tokio::test]
async fn download_builder_mode_resolution() {
    let libs = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    let downloader = Downloader::builder(libs, "/tmp/downloads").build().await.unwrap();
    let video = crate::common::fixtures::load_video_fixture();

    // Auto mode with .mp3 -> AudioOnly
    let builder_mp3 = downloader.download(&video, "track.mp3");
    assert_eq!(builder_mp3.resolved_mode(), DownloadMode::AudioOnly);

    // Auto mode with .m4a -> AudioOnly
    let builder_m4a = downloader.download(&video, "track.m4a");
    assert_eq!(builder_m4a.resolved_mode(), DownloadMode::AudioOnly);

    // Auto mode with .flac -> AudioOnly
    let builder_flac = downloader.download(&video, "track.flac");
    assert_eq!(builder_flac.resolved_mode(), DownloadMode::AudioOnly);

    // Auto mode with .mp4 -> VideoAudio
    let builder_mp4 = downloader.download(&video, "movie.mp4");
    assert_eq!(builder_mp4.resolved_mode(), DownloadMode::VideoAudio);

    // Explicit audio_only() on .mp4 -> AudioOnly
    let builder_forced_audio = downloader.download(&video, "movie.mp4").audio_only();
    assert_eq!(builder_forced_audio.resolved_mode(), DownloadMode::AudioOnly);

    // Explicit mode(VideoAudio) on .mp3 -> VideoAudio
    let builder_forced_video = downloader.download(&video, "track.mp3").mode(DownloadMode::VideoAudio);
    assert_eq!(builder_forced_video.resolved_mode(), DownloadMode::VideoAudio);
}

#[test]
fn dual_progress_tracker_monotonicity_and_completion() {
    // Access internal tracker
    use yt_dlp::client::download_builder::DualProgressTracker;

    let progress_values = Arc::new(Mutex::new(Vec::new()));
    let values_clone = Arc::clone(&progress_values);

    let tracker = DualProgressTracker::new(Box::new(move |progress| {
        values_clone.lock().unwrap().push(progress);
    }));

    // Video starts downloading (total 100 bytes)
    tracker.on_video_progress(10, 100);
    // Audio starts downloading (total 20 bytes)
    tracker.on_audio_progress(5, 20);
    // Audio finishes fast (20 of 20 bytes)
    tracker.on_audio_progress(20, 20);
    // Video continues (50 of 100 bytes)
    tracker.on_video_progress(50, 100);
    // Video finishes (100 of 100 bytes)
    tracker.on_video_progress(100, 100);
    // Finished called
    tracker.finish();

    let values = progress_values.lock().unwrap();
    assert!(!values.is_empty(), "Progress values should not be empty");

    // Check that progress is strictly monotonic (never decreases)
    for window in values.windows(2) {
        assert!(
            window[1] >= window[0],
            "Progress decreased from {} to {}",
            window[0],
            window[1]
        );
    }

    // Check that final value is 1.0 (100%), never stuck at 0.5
    assert_eq!(*values.last().unwrap(), 1.0, "Final progress must be 1.0 (100%)");
}
