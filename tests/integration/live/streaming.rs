use std::sync::Arc;
use std::time::Duration;

use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use yt_dlp::live::{LiveFragmentStreamer, LiveStreamConfig};

use crate::common::server::setup_hls_server;

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
