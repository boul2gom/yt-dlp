use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::client::deps::github::GitHubFetcher;
use yt_dlp::client::deps::{Asset, Libraries, LibraryInstaller, Release, WantedRelease};

// ---------------------------------------------------------------------------
// GitHubFetcher Display and construction
// ---------------------------------------------------------------------------

#[test]
fn display_github_fetcher_shows_repo_info() {
    let fetcher = GitHubFetcher::new("owner", "repo");
    let display = format!("{}", fetcher);
    assert!(display.contains("GitHubFetcher"));
    assert!(display.contains("owner"));
    assert!(display.contains("repo"));
}

#[test]
fn debug_github_fetcher_returns_nonempty_string() {
    let fetcher = GitHubFetcher::new("yt-dlp", "yt-dlp");
    let debug = format!("{:?}", fetcher);
    assert!(!debug.is_empty());
}

// ---------------------------------------------------------------------------
// Release / Asset / WantedRelease Display
// ---------------------------------------------------------------------------

#[test]
fn display_release_shows_tag_and_asset_count() {
    let release = Release {
        tag_name: "v1.2.3".to_string(),
        assets: vec![],
    };
    let display = format!("{}", release);
    assert!(display.contains("Release"));
    assert!(display.contains("v1.2.3"));
    assert!(display.contains("assets=0"));
}

#[test]
fn display_asset_shows_name() {
    let asset = Asset {
        name: "yt-dlp_linux".to_string(),
        download_url: "https://github.com/yt-dlp/yt-dlp/releases/download/v1/yt-dlp_linux".to_string(),
        digest: None,
    };
    let display = format!("{}", asset);
    assert!(display.contains("Asset"));
    assert!(display.contains("yt-dlp_linux"));
}

#[test]
fn display_wanted_release_without_checksum_shows_none() {
    let release = WantedRelease {
        name: "yt-dlp_linux".to_string(),
        url: "https://example.com/yt-dlp".to_string(),
        checksum: None,
    };
    let display = format!("{}", release);
    assert!(display.contains("WantedRelease"));
    assert!(display.contains("none")); // no checksum
}

#[test]
fn display_wanted_release_with_checksum_shows_hash() {
    let release = WantedRelease {
        name: "yt-dlp_linux".to_string(),
        url: "https://example.com/yt-dlp".to_string(),
        checksum: Some("abc123".to_string()),
    };
    let display = format!("{}", release);
    assert!(display.contains("abc123"));
}

// ---------------------------------------------------------------------------
// LibraryInstaller / Libraries Display
// ---------------------------------------------------------------------------

#[test]
fn display_library_installer_shows_path() {
    let installer = LibraryInstaller::new("/tmp/libs".into());
    let display = format!("{}", installer);
    assert!(display.contains("LibraryInstaller"));
    assert!(display.contains("libs"));
}

#[test]
fn display_libraries_shows_both_paths() {
    let libs = Libraries::new("/tmp/libs/yt-dlp".into(), "/tmp/libs/ffmpeg".into());
    let display = format!("{}", libs);
    assert!(display.contains("Libraries"));
    assert!(display.contains("yt-dlp"));
    assert!(display.contains("ffmpeg"));
}

// ---------------------------------------------------------------------------
// WantedRelease::download — wiremock server, with and without checksum
// ---------------------------------------------------------------------------

#[tokio::test]
async fn wanted_release_download_succeeds() {
    let server = MockServer::start().await;

    let content = b"fake binary content";
    Mock::given(method("GET"))
        .and(path("/release/asset"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(content.as_slice()))
        .mount(&server)
        .await;

    let release = WantedRelease {
        name: "asset".to_string(),
        url: format!("{}/release/asset", server.uri()),
        checksum: None,
    };

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("asset");
    release.download(dest.clone()).await.expect("download should succeed");

    assert!(dest.exists(), "downloaded file should exist");
    assert_eq!(std::fs::read(&dest).unwrap(), content);
}

#[tokio::test]
async fn wanted_release_download_checksum_mismatch_fails() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/release/asset"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake content".as_slice()))
        .mount(&server)
        .await;

    let release = WantedRelease {
        name: "asset".to_string(),
        url: format!("{}/release/asset", server.uri()),
        checksum: Some("0000000000000000000000000000000000000000000000000000000000000000".to_string()),
    };

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("asset");
    let result = release.download(dest.clone()).await;

    assert!(result.is_err(), "download with wrong checksum should fail");
    // File should be deleted on checksum mismatch
    assert!(!dest.exists(), "invalid file should be removed after checksum failure");
}
