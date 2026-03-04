use crate::common::fixtures;
use crate::helpers;

/// Verify download_subtitle downloads a subtitle file using automatic captions.
#[tokio::test]
async fn download_subtitle_with_automatic_captions() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_short_video(&server.uri());

    let result = downloader
        .download_subtitle(&video, "fr", "subtitle_fr.srt", true)
        .await;

    assert!(result.is_ok(), "download_subtitle should succeed: {:?}", result.err());
    let path = result.unwrap();
    assert!(path.exists(), "Subtitle file should exist at {:?}", path);

    let content = std::fs::read_to_string(&path).expect("read subtitle");
    assert!(!content.is_empty(), "Subtitle file should not be empty");
}

/// Verify download_subtitle fails when language is not available and fallback is disabled.
#[tokio::test]
async fn download_subtitle_unavailable_language_no_fallback() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_short_video(&server.uri());

    let result = downloader
        .download_subtitle(&video, "zh", "subtitle_zh.srt", false)
        .await;

    assert!(result.is_err(), "Should fail for unavailable language without fallback");
}

/// Verify download_subtitle uses user-uploaded subtitles from the main video fixture.
#[tokio::test]
async fn download_subtitle_from_user_subtitles() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_video(&server.uri());

    // video.json has user-uploaded subtitles for "en"
    let result = downloader
        .download_subtitle(&video, "en", "subtitle_en.srt", false)
        .await;

    assert!(result.is_ok(), "download_subtitle should succeed: {:?}", result.err());
    let path = result.unwrap();
    assert!(path.exists(), "Subtitle file should exist");
}

/// Verify download_all_subtitles downloads all available subtitle languages.
#[tokio::test]
async fn download_all_subtitles_with_fallback() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_short_video(&server.uri());

    let result = downloader.download_all_subtitles(&video, tmp.path(), true).await;

    assert!(
        result.is_ok(),
        "download_all_subtitles should succeed: {:?}",
        result.err()
    );
    let files = result.unwrap();
    assert_eq!(files.len(), 2, "Should download subtitles for fr and en");
    for file in &files {
        assert!(file.exists(), "Subtitle file should exist at {:?}", file);
    }
}

/// Verify download_all_subtitles returns empty when no subtitles and fallback disabled.
#[tokio::test]
async fn download_all_subtitles_no_fallback_empty() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let video = helpers::load_e2e_short_video(&server.uri());

    // short_video has no user-uploaded subtitles, only automatic_captions
    let result = downloader.download_all_subtitles(&video, tmp.path(), false).await;

    assert!(result.is_ok());
    let files = result.unwrap();
    assert!(
        files.is_empty(),
        "Should be empty without fallback since short_video has no user subs"
    );
}
