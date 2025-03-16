use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use yt_dlp::fetcher::deps::Libraries;
use yt_dlp::Youtube;

// Constants for tests
const TEST_SHORT_VIDEO_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw"; // Me at the zoo (first YouTube video)
const TEST_TIMEOUT: u64 = 120; // 2 minutes

// Utility function to set up the test environment
async fn setup_test_environment() -> (PathBuf, PathBuf) {
    let test_dir = PathBuf::from("test_output_audio");
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
async fn test_audio_compatibility() {
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
        .download_audio_stream_from_url(TEST_SHORT_VIDEO_URL.to_string(), "test_audio_compat.mp3")
        .await;
    assert!(result.is_ok(), "Audio download failed: {:?}", result.err());

    let audio_path = result.unwrap();
    assert!(audio_path.exists(), "Audio file does not exist");
    assert!(
        audio_path.metadata().unwrap().len() > 0,
        "Audio file is empty"
    );

    // Check file extension - should be .mp3
    assert_eq!(
        audio_path.extension().unwrap(),
        "mp3",
        "Audio file does not have the correct extension"
    );

    // Verify the file was processed by checking its size and existence
    // We don't rely on ffprobe to check the codec
    assert!(
        audio_path.metadata().unwrap().len() > 1000,
        "Audio file is too small, suggesting it might not be properly processed"
    );

    cleanup_test_environment(&test_dir).await;
}
