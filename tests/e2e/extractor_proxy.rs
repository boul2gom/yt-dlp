//! The metadata extractors run their own yt-dlp command lines, so the configured
//! proxy must reach them. A raw TCP listener stands in for the proxy: the
//! extractor's yt-dlp process should open a CONNECT to YouTube through it.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use yt_dlp::Downloader;
use yt_dlp::client::Libraries;
use yt_dlp::client::proxy::{ProxyConfig, ProxyType};

/// Budget for the whole metadata fetch; the stand-in rejects it, so it ends early.
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Budget for the stand-in proxy to receive the request line.
const LINE_TIMEOUT: Duration = Duration::from_secs(5);
/// Video whose metadata fetch the extractor starts.
const VIDEO_URL: &str = "https://www.youtube.com/watch?v=aqz-KE-bpKQ";
/// Host the extractor must open a tunnel to.
const EXPECTED_HOST: &str = "youtube.com";
/// Response that makes the stand-in reject the tunnel.
const REJECT_RESPONSE: &[u8] = b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n";

#[tokio::test]
async fn extractor_metadata_fetch_goes_through_the_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind proxy stand-in");
    let addr = listener.local_addr().expect("local addr");
    let (line_tx, line_rx) = oneshot::channel::<String>();
    tokio::spawn(async move {
        if let Ok((socket, _)) = listener.accept().await {
            let mut reader = BufReader::new(socket);
            let mut first = String::new();
            let _ = reader.read_line(&mut first).await;
            let _ = line_tx.send(first.trim_end().to_owned());
            let _ = reader.get_mut().write_all(REJECT_RESPONSE).await;
        }
    });

    let scratch = tempfile::tempdir().expect("scratch dir");
    let libraries = Libraries::new(PathBuf::from("yt-dlp"), PathBuf::from("ffmpeg"));
    let downloader = Downloader::builder(libraries, scratch.path())
        .with_timeout(FETCH_TIMEOUT)
        .with_proxy(ProxyConfig::new(ProxyType::Http, format!("http://{addr}")))
        .build()
        .await
        .expect("downloader builds");

    let _ = tokio::time::timeout(FETCH_TIMEOUT, downloader.fetch_video_infos(VIDEO_URL)).await;

    let first = tokio::time::timeout(LINE_TIMEOUT, line_rx)
        .await
        .expect("the extractor's yt-dlp used the proxy")
        .expect("request line");
    assert!(first.starts_with("CONNECT "), "{first}");
    assert!(first.contains(EXPECTED_HOST), "{first}");
}
