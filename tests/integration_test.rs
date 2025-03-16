use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use yt_dlp::fetcher::deps::Libraries;
use yt_dlp::Youtube;

// Constants for tests
const TEST_VIDEO_URL: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ"; // Never Gonna Give You Up
const TEST_SHORT_VIDEO_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw"; // Me at the zoo (first YouTube video)
const TEST_TIMEOUT: u64 = 120; // 2 minutes

// Utility function to set up the test environment
async fn setup_test_environment() -> (PathBuf, PathBuf) {
    let test_dir = PathBuf::from("test_output");
    let libs_dir = test_dir.join("libs");

    // Create directories if they don't exist
    if !test_dir.exists() {
        fs::create_dir_all(&test_dir)
            .await
            .expect("Failed to create test directory");
    }

    if !libs_dir.exists() {
        fs::create_dir_all(&libs_dir)
            .await
            .expect("Failed to create libraries directory");
    }

    (test_dir, libs_dir)
}

// Utility function to clean up the test environment
async fn cleanup_test_environment(test_dir: &PathBuf) {
    if test_dir.exists() {
        let _ = fs::remove_dir_all(test_dir).await;
    }
}

#[tokio::test]
async fn test_install_binaries() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Test binary installation
    let result = Youtube::with_new_binaries(&libs_dir, &test_dir).await;
    assert!(
        result.is_ok(),
        "Binary installation failed: {:?}",
        result.err()
    );

    // Verify that binaries exist
    let youtube_bin = libs_dir.join(if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    });
    let ffmpeg_bin = libs_dir.join(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    });

    assert!(youtube_bin.exists(), "yt-dlp binary was not installed");
    assert!(ffmpeg_bin.exists(), "ffmpeg binary was not installed");

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_fetch_video_info() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries or use existing ones
    let youtube = match Youtube::with_new_binaries(&libs_dir, &test_dir).await {
        Ok(yt) => yt,
        Err(_) => {
            // Try to use system binaries
            let libraries = Libraries::new("yt-dlp".into(), "ffmpeg".into());
            Youtube::new(libraries, &test_dir).expect("Failed to create Youtube instance")
        }
    };

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Test video info retrieval
    let result = youtube
        .fetch_video_infos(TEST_SHORT_VIDEO_URL.to_string())
        .await;
    assert!(
        result.is_ok(),
        "Video info retrieval failed: {:?}",
        result.err()
    );

    let video = result.unwrap();
    assert_eq!(video.id, "jNQXAC9IVRw", "Video ID is incorrect");
    assert!(video.title.contains("zoo"), "Video title is incorrect");
    assert!(!video.formats.is_empty(), "No formats were found");

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_download_audio() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries or use existing ones
    let youtube = match Youtube::with_new_binaries(&libs_dir, &test_dir).await {
        Ok(yt) => yt,
        Err(_) => {
            // Try to use system binaries
            let libraries = Libraries::new("yt-dlp".into(), "ffmpeg".into());
            Youtube::new(libraries, &test_dir).expect("Failed to create Youtube instance")
        }
    };

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Test audio download
    let result = youtube
        .download_audio_stream_from_url(TEST_SHORT_VIDEO_URL.to_string(), "test_audio.mp3")
        .await;
    assert!(result.is_ok(), "Audio download failed: {:?}", result.err());

    let audio_path = result.unwrap();
    assert!(audio_path.exists(), "Audio file does not exist");
    assert!(
        audio_path.metadata().unwrap().len() > 0,
        "Audio file is empty"
    );

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_download_video() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries or use existing ones
    let youtube = match Youtube::with_new_binaries(&libs_dir, &test_dir).await {
        Ok(yt) => yt,
        Err(_) => {
            // Try to use system binaries
            let libraries = Libraries::new("yt-dlp".into(), "ffmpeg".into());
            Youtube::new(libraries, &test_dir).expect("Failed to create Youtube instance")
        }
    };

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Test video download
    let result = youtube
        .download_video_stream_from_url(TEST_SHORT_VIDEO_URL.to_string(), "test_video.mp4")
        .await;
    assert!(result.is_ok(), "Video download failed: {:?}", result.err());

    let video_path = result.unwrap();
    assert!(video_path.exists(), "Video file does not exist");
    assert!(
        video_path.metadata().unwrap().len() > 0,
        "Video file is empty"
    );

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_download_complete_video() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries or use existing ones
    let youtube = match Youtube::with_new_binaries(&libs_dir, &test_dir).await {
        Ok(yt) => yt,
        Err(_) => {
            // Try to use system binaries
            let libraries = Libraries::new("yt-dlp".into(), "ffmpeg".into());
            Youtube::new(libraries, &test_dir).expect("Failed to create Youtube instance")
        }
    };

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Test complete video download (audio + video)
    let result = youtube
        .download_video_from_url(TEST_VIDEO_URL.to_string(), "test_complete.mp4")
        .await;
    assert!(
        result.is_ok(),
        "Complete video download failed: {:?}",
        result.err()
    );

    let complete_path = result.unwrap();
    assert!(complete_path.exists(), "Complete file does not exist");
    assert!(
        complete_path.metadata().unwrap().len() > 0,
        "Complete file is empty"
    );

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_format_selection() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries or use existing ones
    let youtube = match Youtube::with_new_binaries(&libs_dir, &test_dir).await {
        Ok(yt) => yt,
        Err(_) => {
            // Try to use system binaries
            let libraries = Libraries::new("yt-dlp".into(), "ffmpeg".into());
            Youtube::new(libraries, &test_dir).expect("Failed to create Youtube instance")
        }
    };

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Get video info
    let video = youtube
        .fetch_video_infos(TEST_VIDEO_URL.to_string())
        .await
        .unwrap();

    // Test format selection
    let best_audio = video.best_audio_format();
    assert!(best_audio.is_some(), "No audio format was found");
    assert!(
        best_audio.unwrap().is_audio(),
        "Selected format is not audio"
    );

    let best_video = video.best_video_format();
    assert!(best_video.is_some(), "No video format was found");
    assert!(
        best_video.unwrap().is_video(),
        "Selected format is not video"
    );

    cleanup_test_environment(&test_dir).await;
}

#[tokio::test]
async fn test_update_downloader() {
    let (test_dir, libs_dir) = setup_test_environment().await;

    // Install binaries
    let youtube = Youtube::with_new_binaries(&libs_dir, &test_dir)
        .await
        .unwrap();

    // Configure a longer timeout for tests
    let mut youtube = youtube;
    youtube.with_timeout(Duration::from_secs(TEST_TIMEOUT));

    // Test downloader update
    let result = youtube.update_downloader().await;
    assert!(
        result.is_ok(),
        "Downloader update failed: {:?}",
        result.err()
    );

    cleanup_test_environment(&test_dir).await;
}
