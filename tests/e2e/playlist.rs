use yt_dlp::DownloadStatus;

use crate::common::fixtures;
use crate::helpers;

/// Verify the playlist fixture loads correctly and has the expected structure.
#[tokio::test]
async fn playlist_fixture_loads() {
    let playlist = helpers::load_e2e_playlist();

    assert_eq!(playlist.id, "PLhixgUqwRTjwvBI-hmbZ2rpkAl4lutnJG");
    assert_eq!(playlist.entries.len(), 5);
    assert_eq!(playlist.entry_count(), 5);
    assert_eq!(playlist.title, "Minecraft:HACKED");
}

/// Verify each playlist entry has an ID and URL.
#[tokio::test]
async fn playlist_entries_have_metadata() {
    let playlist = helpers::load_e2e_playlist();

    for (i, entry) in playlist.entries.iter().enumerate() {
        assert!(!entry.id.is_empty(), "Entry {} should have a non-empty ID", i);
        assert!(!entry.url.is_empty(), "Entry {} should have a non-empty URL", i);
    }
}

/// download_playlist requires yt-dlp to fetch each entry's video info.
/// With fake binaries, it should fail with an appropriate error.
#[tokio::test]
async fn download_playlist_fails_without_ytdlp() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let playlist = helpers::load_e2e_playlist();

    let result = downloader.download_playlist(&playlist, "%(title)s.%(ext)s").await;

    assert!(result.is_err(), "download_playlist should fail without yt-dlp binary");
}

/// download_playlist_parallel also requires yt-dlp for each entry.
#[tokio::test]
async fn download_playlist_parallel_fails_without_ytdlp() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let playlist = helpers::load_e2e_playlist();

    let result = downloader
        .download_playlist_parallel(&playlist, "%(title)s.%(ext)s", Some(2))
        .await;

    // The outer Result may succeed while inner results contain errors.
    // Either scenario indicates the pipeline cannot complete without yt-dlp.
    match result {
        Err(_) => {} // Outer error — expected
        Ok(results) => {
            let all_failed = results.iter().all(|r| r.is_err());
            assert!(
                all_failed,
                "All playlist entries should fail without yt-dlp, but some succeeded"
            );
        }
    }
}

/// download_playlist_range also requires yt-dlp.
#[tokio::test]
async fn download_playlist_range_fails_without_ytdlp() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let playlist = helpers::load_e2e_playlist();

    let result = downloader
        .download_playlist_range(&playlist, 0, 1, "%(title)s.%(ext)s")
        .await;

    assert!(
        result.is_err(),
        "download_playlist_range should fail without yt-dlp binary"
    );
}

/// download_playlist_items also requires yt-dlp.
#[tokio::test]
async fn download_playlist_items_fails_without_ytdlp() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;
    let playlist = helpers::load_e2e_playlist();

    let result = downloader
        .download_playlist_items(&playlist, &[0, 2], "%(title)s.%(ext)s")
        .await;

    assert!(
        result.is_err(),
        "download_playlist_items should fail without yt-dlp binary"
    );
}

/// Multiple formats can be downloaded independently via the download manager
/// to simulate what a playlist download would do internally.
#[tokio::test]
async fn simulate_playlist_via_manager() {
    let server = helpers::setup_e2e_server().await;
    let tmp = fixtures::temp_test_dir();
    let downloader = helpers::build_e2e_downloader(&server.uri(), tmp.path()).await;

    // Simulate downloading three "playlist items" directly through the manager
    let urls: Vec<String> = (1..=3)
        .map(|i| format!("{}/media/item_{}.bin", server.uri(), i))
        .collect();

    let mut ids = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        let output = tmp.path().join(format!("item_{}.bin", i));
        let id = downloader.download_manager().enqueue(url, &output, None).await;
        ids.push(id);
    }

    // Wait for all downloads
    let mut all_completed = true;
    for id in &ids {
        let status = downloader.wait_for_download(*id).await;
        if !matches!(status, Some(DownloadStatus::Completed)) {
            all_completed = false;
        }
    }

    assert!(all_completed, "All playlist-simulated downloads should complete");
}
