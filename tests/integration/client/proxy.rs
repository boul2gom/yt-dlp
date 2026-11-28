//! `ProxyType::Http` routes HTTPS requests through the proxy too.
//!
//! A raw TCP listener stands in for the proxy because wiremock cannot speak
//! CONNECT: the test asserts the first request line the proxy receives.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use yt_dlp::client::proxy::{ProxyConfig, ProxyType};
use yt_dlp::utils::http::create_http_client;

/// Budget for the request through the stand-in proxy.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Budget for the stand-in proxy to receive the request line.
const LINE_TIMEOUT: Duration = Duration::from_secs(5);
/// Host requested through the proxy; it never resolves, so only CONNECT proves routing.
const TARGET_URL: &str = "https://example.invalid/";
/// Request line an HTTPS request through an HTTP proxy must open with.
const EXPECTED_LINE: &str = "CONNECT example.invalid:443 HTTP/1.1";
/// Response that makes the stand-in reject the tunnel.
const REJECT_RESPONSE: &[u8] = b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n";

#[tokio::test]
async fn http_proxy_type_routes_https_through_connect() {
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

    let proxy = ProxyConfig::new(ProxyType::Http, format!("http://{addr}"));
    let client = create_http_client(Some(&proxy)).expect("client builds");
    // The request fails by design (the stand-in answers 502); what matters is
    // that it reached the proxy as a CONNECT rather than going direct.
    let _ = client.get(TARGET_URL).timeout(REQUEST_TIMEOUT).send().await;

    let first = tokio::time::timeout(LINE_TIMEOUT, line_rx)
        .await
        .expect("the proxy received a request")
        .expect("request line");
    assert_eq!(first, EXPECTED_LINE);
}
