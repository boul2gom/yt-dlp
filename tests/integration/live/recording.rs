use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};
use yt_dlp::live::hls::{HlsPlaylist, HlsSegment, HlsVariant, select_variant};
use yt_dlp::live::{LiveRecorder, RecordingConfig, RecordingResult};

use crate::common;
use crate::common::server::setup_hls_server;

// ============================== parse_master ==============================

#[tokio::test]
async fn parse_master_playlist() {
    let server = setup_hls_server().await;
    let client = reqwest::Client::new();

    let url = format!("{}/hls/master.m3u8", server.uri());
    let variants = yt_dlp::live::hls::parse_master(&client, &url)
        .await
        .expect("parse_master should succeed");

    assert!(!variants.is_empty(), "should have at least one variant");

    // Our fixture has 3 variants (360p, 480p, 720p)
    assert_eq!(variants.len(), 3);

    // Verify they have bandwidth info
    for v in &variants {
        assert!(v.bandwidth > 0);
        assert!(!v.url.is_empty());
    }

    // Verify bandwidths are different
    let bandwidths: Vec<u64> = variants.iter().map(|v| v.bandwidth).collect();
    let unique: std::collections::HashSet<u64> = bandwidths.iter().copied().collect();
    assert_eq!(unique.len(), 3, "all bandwidths should be unique");
}

// ============================== parse_media ==============================

#[tokio::test]
async fn parse_media_playlist() {
    let server = setup_hls_server().await;
    let client = reqwest::Client::new();

    let url = format!("{}/hls/720p.m3u8", server.uri());
    let playlist = yt_dlp::live::hls::parse_media(&client, &url)
        .await
        .expect("parse_media should succeed");

    assert!(!playlist.segments.is_empty(), "should have segments");
    assert_eq!(playlist.segments.len(), 3);
    assert!(playlist.is_endlist, "fixture has EXT-X-ENDLIST");
    assert_eq!(playlist.media_sequence, 0);
    assert!((playlist.target_duration - 10.0).abs() < 0.1);

    // Verify segments have valid URLs
    for seg in &playlist.segments {
        assert!(!seg.url.is_empty());
        assert!(seg.duration > 0.0);
    }
}

#[tokio::test]
async fn parse_media_segments_have_correct_sequence() {
    let server = setup_hls_server().await;
    let client = reqwest::Client::new();

    let url = format!("{}/hls/720p.m3u8", server.uri());
    let playlist = yt_dlp::live::hls::parse_media(&client, &url).await.unwrap();

    for (i, seg) in playlist.segments.iter().enumerate() {
        assert_eq!(seg.sequence, i as u64);
    }
}

// ============================== select_variant ==============================

#[tokio::test]
async fn select_best_variant_from_parsed() {
    let server = setup_hls_server().await;
    let client = reqwest::Client::new();

    let url = format!("{}/hls/master.m3u8", server.uri());
    let variants = yt_dlp::live::hls::parse_master(&client, &url).await.unwrap();

    let best = select_variant(&variants, None).expect("should select best variant");
    assert_eq!(best.bandwidth, 2_800_000);
    assert!(best.resolution.as_deref().unwrap().contains("1280"));
}

#[tokio::test]
async fn select_variant_with_bandwidth_limit() {
    let server = setup_hls_server().await;
    let client = reqwest::Client::new();

    let url = format!("{}/hls/master.m3u8", server.uri());
    let variants = yt_dlp::live::hls::parse_master(&client, &url).await.unwrap();

    // Limit to 1Mbps should select 360p (800kbps)
    let selected = select_variant(&variants, Some(1_000_000)).expect("should select variant");
    assert_eq!(selected.bandwidth, 800_000);
}

// ============================== HLS type Display ==============================

#[test]
fn hls_segment_display_format() {
    let seg = HlsSegment {
        url: "https://example.com/seg0.ts".to_string(),
        duration: 10.0,
        sequence: 5,
    };
    let display = format!("{}", seg);
    assert!(display.contains("seq=5"));
    assert!(display.contains("10.00s"));
}

#[test]
fn hls_playlist_display_format() {
    let playlist = HlsPlaylist {
        target_duration: 10.0,
        media_sequence: 42,
        segments: vec![HlsSegment {
            url: "https://example.com/seg0.ts".to_string(),
            duration: 10.0,
            sequence: 42,
        }],
        is_endlist: true,
    };
    let display = format!("{}", playlist);
    assert!(display.contains("segments=1"));
    assert!(display.contains("endlist=true"));
    assert!(display.contains("media_sequence=42"));
}

#[test]
fn hls_variant_display_format() {
    let variant = HlsVariant {
        url: "https://example.com/720p.m3u8".to_string(),
        bandwidth: 3_000_000,
        resolution: Some("1280x720".to_string()),
        codecs: Some("avc1.4d401f".to_string()),
    };
    let display = format!("{}", variant);
    assert!(display.contains("3000000"));
    assert!(display.contains("1280x720"));
}

#[test]
fn recording_result_display() {
    let result = RecordingResult {
        output_path: std::path::PathBuf::from("/tmp/recording.ts"),
        total_bytes: 1_048_576,
        total_duration: Duration::from_secs(120),
        segments_downloaded: 12,
    };
    let display = format!("{}", result);
    assert!(display.contains("1048576"));
    assert!(display.contains("120.0s"));
    assert!(display.contains("segments=12"));
}

// ============================== parse_master error handling ==============================

#[tokio::test]
async fn parse_master_invalid_url_fails() {
    let client = reqwest::Client::new();
    let result = yt_dlp::live::hls::parse_master(&client, "http://127.0.0.1:1/invalid").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn parse_media_invalid_url_fails() {
    let client = reqwest::Client::new();
    let result = yt_dlp::live::hls::parse_media(&client, "http://127.0.0.1:1/invalid").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn parse_master_wrong_content() {
    let server = MockServer::start().await;

    // Serve a media playlist where master is expected
    let media_content = common::fixtures::load_hls_fixture("media.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path("/wrong.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(media_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let url = format!("{}/wrong.m3u8", server.uri());
    let result = yt_dlp::live::hls::parse_master(&client, &url).await;
    assert!(result.is_err(), "media playlist should not parse as master");
}

#[tokio::test]
async fn parse_media_wrong_content() {
    let server = MockServer::start().await;

    // Serve a master playlist where media is expected
    let master_content = common::fixtures::load_hls_fixture("master.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path("/wrong.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(master_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let url = format!("{}/wrong.m3u8", server.uri());
    let result = yt_dlp::live::hls::parse_media(&client, &url).await;
    assert!(result.is_err(), "master playlist should not parse as media");
}

// ============================== LiveRecorder::record() ==============================

/// Serves a single-segment ENDLIST playlist with target_duration=1 (poll_interval=500ms).
async fn setup_fast_endlist_server() -> MockServer {
    let server = MockServer::start().await;

    let media_content = format!(
        "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:1\n#EXT-X-MEDIA-SEQUENCE:0\n\
         #EXTINF:0.5,\n{}/hls/segment_0.ts\n#EXT-X-ENDLIST\n",
        server.uri()
    );

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
async fn record_stops_when_stream_ends_via_endlist() {
    let server = setup_fast_endlist_server().await;
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("recording.ts");

    let config = RecordingConfig {
        stream_url: format!("{}/hls/media.m3u8", server.uri()),
        output_path: output.clone(),
        video_id: "test_endlist".to_string(),
        quality: "720p".to_string(),
        max_duration: None,
        cancellation_token: CancellationToken::new(),
        event_bus: yt_dlp::events::EventBus::with_default_capacity(),
    };

    let client = Arc::new(reqwest::Client::new());
    let recorder = LiveRecorder::new(config, client);
    let result = recorder.record().await.expect("recording should succeed");

    assert!(result.total_bytes > 0, "should have written bytes");
    assert!(
        result.segments_downloaded >= 1,
        "should have downloaded at least one segment"
    );
    assert!(output.exists(), "output file should exist");
}

#[tokio::test]
async fn record_stops_via_cancellation_token() {
    let server = setup_hls_server().await;
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("cancelled_recording.ts");

    let token = CancellationToken::new();
    let token_clone = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        token_clone.cancel();
    });

    let config = RecordingConfig {
        stream_url: format!("{}/hls/720p.m3u8", server.uri()),
        output_path: output.clone(),
        video_id: "test_cancel".to_string(),
        quality: "720p".to_string(),
        max_duration: None,
        cancellation_token: token,
        event_bus: yt_dlp::events::EventBus::with_default_capacity(),
    };

    let client = Arc::new(reqwest::Client::new());
    let recorder = LiveRecorder::new(config, client);
    let result = recorder.record().await.expect("recording should succeed after cancel");

    assert!(result.segments_downloaded >= 1, "initial segments should be downloaded");
    assert!(output.exists(), "output file should be created");
}
