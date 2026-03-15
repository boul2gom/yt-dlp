use wiremock::matchers::{header_exists, method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::fixtures;

/// Sets up a minimal wiremock server serving HLS master + media playlists and segments.
///
/// Mounts three routes:
/// - `GET /hls/master.m3u8` — HLS master playlist
/// - `GET /hls/*.m3u8` — HLS media playlist
/// - `GET /hls/segment_N.ts` — TS segment bytes
pub async fn setup_hls_server() -> MockServer {
    let server = MockServer::start().await;

    let master_content = fixtures::load_hls_fixture("master.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path("/hls/master.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(master_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let media_content = fixtures::load_hls_fixture("media.m3u8", &server.uri());
    Mock::given(method("GET"))
        .and(path_regex(r"^/hls/.*\.m3u8$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(media_content)
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    let segment_bytes = fixtures::load_media_bytes("small.ts");
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

/// Sets up a mock HTTP server that serves media files, HLS playlists, and thumbnails.
pub async fn setup_media_server() -> MockServer {
    let server = MockServer::start().await;
    let media_bytes = fixtures::load_media_bytes("small.bin");

    // Serve a generic media file for all /media/* requests
    Mock::given(method("GET"))
        .and(path_regex(r"^/media/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(media_bytes.clone())
                .insert_header("Content-Type", "application/octet-stream")
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&server)
        .await;

    // Range request support for partial downloads
    Mock::given(method("GET"))
        .and(path_regex(r"^/media/.*"))
        .and(header_exists("Range"))
        .respond_with(
            ResponseTemplate::new(206)
                .set_body_bytes(media_bytes[..1024.min(media_bytes.len())].to_vec())
                .insert_header("Content-Type", "application/octet-stream")
                .insert_header(
                    "Content-Range",
                    format!("bytes 0-{}/{}", 1023.min(media_bytes.len() - 1), media_bytes.len()),
                )
                .insert_header("Accept-Ranges", "bytes"),
        )
        .mount(&server)
        .await;

    // Serve thumbnail images
    Mock::given(method("GET"))
        .and(path_regex(r"^/thumbnails/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0xFF, 0xD8, 0xFF, 0xE0]) // minimal JPEG header
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&server)
        .await;

    // Serve storyboard images
    Mock::given(method("GET"))
        .and(path_regex(r"^/storyboard/.*"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0xFF, 0xD8, 0xFF, 0xE0])
                .insert_header("Content-Type", "image/jpeg"),
        )
        .mount(&server)
        .await;

    // Serve HLS master playlist
    Mock::given(method("GET"))
        .and(path("/hls/master.m3u8"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::load_hls_fixture("master.m3u8", &server.uri()))
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    // Serve HLS media playlist
    Mock::given(method("GET"))
        .and(path_regex(r"^/hls/.*\.m3u8$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::load_hls_fixture("media.m3u8", &server.uri()))
                .insert_header("Content-Type", "application/vnd.apple.mpegurl"),
        )
        .mount(&server)
        .await;

    // Serve HLS segments
    Mock::given(method("GET"))
        .and(path_regex(r"^/hls/segment_\d+\.ts$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(media_bytes)
                .insert_header("Content-Type", "video/mp2t"),
        )
        .mount(&server)
        .await;

    // Serve SRT subtitle files (under /captions/ and /subtitles/)
    Mock::given(method("GET"))
        .and(path_regex(r"^/(captions|subtitles)/.*\.srt$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::load_subtitle_string("sample.srt"))
                .insert_header("Content-Type", "text/plain; charset=utf-8"),
        )
        .mount(&server)
        .await;

    // Serve VTT subtitle files (under /captions/ and /subtitles/)
    Mock::given(method("GET"))
        .and(path_regex(r"^/(captions|subtitles)/.*\.vtt$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixtures::load_subtitle_string("sample.vtt"))
                .insert_header("Content-Type", "text/vtt; charset=utf-8"),
        )
        .mount(&server)
        .await;

    // Serve JSON captions (live chat etc.)
    Mock::given(method("GET"))
        .and(path_regex(r"^/(captions|subtitles)/.*\.json[3]?$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{\"events\":[]}")
                .insert_header("Content-Type", "application/json"),
        )
        .mount(&server)
        .await;

    // Serve other caption formats (srv1, srv2, srv3, ttml, ass, ssa)
    Mock::given(method("GET"))
        .and(path_regex(r"^/(captions|subtitles)/.*\.(srv[123]|ttml|ass|ssa)$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("")
                .insert_header("Content-Type", "text/plain; charset=utf-8"),
        )
        .mount(&server)
        .await;

    server
}
