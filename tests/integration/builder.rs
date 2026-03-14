use std::path::PathBuf;
use std::time::Duration;

use yt_dlp::DownloaderBuilder;
use yt_dlp::client::deps::Libraries;
use yt_dlp::download::SpeedProfile;

fn fake_libraries() -> Libraries {
    Libraries::new(PathBuf::from("/tmp/fake-yt-dlp"), PathBuf::from("/tmp/fake-ffmpeg"))
}

// ---------------------------------------------------------------------------
// Basic build
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_defaults() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .build()
        .await
        .expect("build failed");

    assert_eq!(downloader.output_dir(), dir.path());
    assert!(downloader.args().is_empty());
    assert!(downloader.proxy().is_none());
}

// ---------------------------------------------------------------------------
// with_args
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_args() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_args(vec!["--no-playlist".into(), "--flat-playlist".into()])
        .build()
        .await
        .expect("build failed");

    assert!(downloader.args().contains(&"--no-playlist".to_string()));
    assert!(downloader.args().contains(&"--flat-playlist".to_string()));
}

// ---------------------------------------------------------------------------
// add_arg
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_add_arg() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .add_arg("--no-check-certificates")
        .build()
        .await
        .expect("build failed");

    assert!(downloader.args().contains(&"--no-check-certificates".to_string()));
}

// ---------------------------------------------------------------------------
// with_timeout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_timeout() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let timeout = Duration::from_secs(120);

    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_timeout(timeout)
        .build()
        .await
        .expect("build failed");

    assert_eq!(downloader.timeout(), timeout);
}

// ---------------------------------------------------------------------------
// with_user_agent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_user_agent() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_user_agent("test-agent/1.0")
        .build()
        .await
        .expect("build failed");

    assert_eq!(downloader.user_agent(), Some("test-agent/1.0"));
}

// ---------------------------------------------------------------------------
// with_cookies adds arg
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_cookies() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_cookies("/tmp/cookies.txt")
        .build()
        .await
        .expect("build failed");

    assert!(
        downloader.args().iter().any(|a| a.contains("--cookies")),
        "args should contain --cookies, got: {:?}",
        downloader.args()
    );
}

// ---------------------------------------------------------------------------
// with_cookies_from_browser adds arg
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_cookies_from_browser() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_cookies_from_browser("chrome")
        .build()
        .await
        .expect("build failed");

    assert!(
        downloader.args().iter().any(|a| a.contains("--cookies-from-browser")),
        "args should contain --cookies-from-browser"
    );
}

// ---------------------------------------------------------------------------
// with_netrc adds arg
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_netrc() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_netrc()
        .build()
        .await
        .expect("build failed");

    assert!(
        downloader.args().contains(&"--netrc".to_string()),
        "args should contain --netrc"
    );
}

// ---------------------------------------------------------------------------
// with_speed_profile
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_speed_profile() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_speed_profile(SpeedProfile::Aggressive)
        .build()
        .await
        .expect("build failed");

    assert_eq!(downloader.download_manager().speed_profile(), SpeedProfile::Aggressive);
}

// ---------------------------------------------------------------------------
// with_max_concurrent_downloads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_max_concurrent_downloads() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_max_concurrent_downloads(2)
        .build()
        .await
        .expect("build failed");

    // The download manager should have the configured limit
    assert!(downloader.download_manager().parallel_segments() > 0);
}

// ---------------------------------------------------------------------------
// with_proxy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn build_with_proxy() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let proxy =
        yt_dlp::client::proxy::ProxyConfig::new(yt_dlp::client::proxy::ProxyType::Http, "http://127.0.0.1:8080");

    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_proxy(proxy)
        .build()
        .await
        .expect("build failed");

    assert!(downloader.proxy().is_some());
    assert!(
        downloader.args().iter().any(|a| a == "--proxy"),
        "args should contain --proxy"
    );
}

// ---------------------------------------------------------------------------
// with_cache_config (feature: cache)
// ---------------------------------------------------------------------------

#[cfg(persistent_cache)]
#[tokio::test]
async fn build_with_cache() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let cache_dir = dir.path().join("cache");

    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .with_cache_config(
            yt_dlp::cache::config::CacheConfig::builder()
                .cache_dir(cache_dir.clone())
                .persistent_backend(Some(yt_dlp::cache::PersistentBackendKind::Json))
                .build(),
        )
        .build()
        .await
        .expect("build failed");

    assert!(downloader.cache().is_some());
}

// ---------------------------------------------------------------------------
// Event bus is always created
// ---------------------------------------------------------------------------

#[tokio::test]
async fn event_bus_created() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .build()
        .await
        .expect("build failed");

    // Bus should exist and be functional
    let bus = downloader.event_bus();
    // Subscriber count can be > 0 if statistics or other features create subscribers
    let _count = bus.subscriber_count();
}

// ---------------------------------------------------------------------------
// Output directory created
// ---------------------------------------------------------------------------

#[tokio::test]
async fn output_directory_created() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let output = dir.path().join("new_output");

    let _downloader = DownloaderBuilder::new(fake_libraries(), &output)
        .build()
        .await
        .expect("build failed");

    assert!(output.exists(), "output directory should be created");
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

#[test]
fn display() {
    let builder = DownloaderBuilder::new(fake_libraries(), "/tmp/output");
    let display = format!("{builder}");
    assert!(display.contains("DownloaderBuilder"));
    assert!(display.contains("/tmp/output"));
}

// ---------------------------------------------------------------------------
// Setters post-build
// ---------------------------------------------------------------------------

#[tokio::test]
async fn post_build_setters() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let mut downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .build()
        .await
        .expect("build failed");

    downloader.set_user_agent("new-agent");
    assert_eq!(downloader.user_agent(), Some("new-agent"));

    downloader.set_timeout(Duration::from_secs(999));
    assert_eq!(downloader.timeout(), Duration::from_secs(999));

    downloader.set_args(vec!["--arg1".into()]);
    assert_eq!(downloader.args(), &["--arg1"]);

    downloader.add_arg("--arg2");
    assert!(downloader.args().contains(&"--arg2".to_string()));
}

// ---------------------------------------------------------------------------
// Shutdown
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shutdown_and_check() {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let downloader = DownloaderBuilder::new(fake_libraries(), dir.path())
        .build()
        .await
        .expect("build failed");

    assert!(!downloader.is_shutdown_requested());
    downloader.shutdown();
    assert!(downloader.is_shutdown_requested());
}
