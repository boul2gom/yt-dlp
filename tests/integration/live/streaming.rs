use std::sync::Arc;
use std::time::Duration;

use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::live::{LiveFragmentStreamer, LiveStreamConfig};

use crate::common;

/// Sets up a wiremock server serving HLS master + media playlists and segments.
async fn setup_hls_server() -> MockServer {
    let server = MockServer::start().await;

    let master_content = common::fixtures::load_hls_fixture("master.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path("/hls/master.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(master_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let media_content = common::fixtures::load_hls_fixture("media.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path_regex(r"^/hls/.*\.m3u8$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(media_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let segment_bytes = common::fixtures::load_media_bytes("small.ts");
    Mock::given(method("GET"))
        .and(path_regex(r"^/hls/segment_\d+\.ts$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(segment_bytes)
                .insert_header("Content-Type", "video/mp2t"),
        )
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn stream_live_fragments_yields_segments() {
    let server = setup_hls_server().await;
    let client = Arc::new(reqwest::Client::new());
    let cancellation_token = CancellationToken::new();

    let config = LiveStreamConfig {
        stream_url: format!("{}/hls/720p.m3u8", server.uri()),
        video_id: "test_video".to_string(),
        quality: "720p".to_string(),
        max_duration: Some(Duration::from_secs(1)),
        cancellation_token: cancellation_token.clone(),
        event_bus: yt_dlp::events::EventBus::with_default_capacity(),
    };

    let streamer = LiveFragmentStreamer::new(config, client);
    let mut stream = streamer.stream().await.expect("stream should start");

    let first = stream.next().await.expect("expected first fragment");
    let fragment = first.expect("fragment should be ok");
    assert!(!fragment.data.is_empty(), "fragment data should not be empty");

    cancellation_token.cancel();
    let _ = stream.next().await;
}
