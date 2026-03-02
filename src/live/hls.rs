//! HLS manifest parsing for live stream recording.
//!
//! Wraps `m3u8-rs` to provide a simplified API for fetching and parsing
//! HLS master and media playlists used by live streams.

use std::fmt;

use crate::error::{Error, Result};

/// A single HLS segment from a media playlist.
#[derive(Debug, Clone)]
pub struct HlsSegment {
    /// The absolute URL of the segment.
    pub url: String,
    /// The duration of the segment in seconds.
    pub duration: f64,
    /// The media sequence number of this segment.
    pub sequence: u64,
}

impl fmt::Display for HlsSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HlsSegment(seq={}, duration={:.2}s)", self.sequence, self.duration)
    }
}

/// A parsed HLS media playlist.
#[derive(Debug, Clone)]
pub struct HlsPlaylist {
    /// The target duration declared in the playlist.
    pub target_duration: f64,
    /// The media sequence number of the first segment.
    pub media_sequence: u64,
    /// The parsed segments.
    pub segments: Vec<HlsSegment>,
    /// Whether the playlist has the `#EXT-X-ENDLIST` tag (stream ended).
    pub is_endlist: bool,
}

impl fmt::Display for HlsPlaylist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HlsPlaylist(segments={}, target_duration={:.1}s, media_sequence={}, endlist={})",
            self.segments.len(),
            self.target_duration,
            self.media_sequence,
            self.is_endlist
        )
    }
}

/// A variant stream from an HLS master playlist.
#[derive(Debug, Clone)]
pub struct HlsVariant {
    /// The absolute URL of the variant media playlist.
    pub url: String,
    /// The declared bandwidth in bits per second.
    pub bandwidth: u64,
    /// The resolution as "WIDTHxHEIGHT", if present.
    pub resolution: Option<String>,
    /// The codecs string, if present.
    pub codecs: Option<String>,
}

impl fmt::Display for HlsVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HlsVariant(bandwidth={}, resolution={})",
            self.bandwidth,
            self.resolution.as_deref().unwrap_or("unknown")
        )
    }
}

/// Resolves a possibly relative URI against a base URL.
fn resolve_url(base: &str, uri: &str) -> String {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return uri.to_string();
    }

    url::Url::parse(base)
        .and_then(|base_url| base_url.join(uri))
        .map(|resolved| resolved.to_string())
        .unwrap_or_else(|_| {
            // Fallback to simple path concatenation
            if let Some(pos) = base.rfind('/') {
                format!("{}/{}", &base[..pos], uri)
            } else {
                uri.to_string()
            }
        })
}

/// Fetches and parses an HLS master playlist.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for fetching.
/// * `url` - The URL of the master playlist.
///
/// # Errors
///
/// Returns an error if the fetch or parsing fails.
///
/// # Returns
///
/// A vector of variant streams found in the master playlist.
pub async fn parse_master(client: &reqwest::Client, url: &str) -> Result<Vec<HlsVariant>> {
    tracing::debug!(url = url, "📡 Fetching HLS master playlist");

    let body = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::http(url, "fetching master playlist", e))?
        .bytes()
        .await
        .map_err(|e| Error::http(url, "reading master playlist body", e))?;

    let parsed = m3u8_rs::parse_playlist_res(&body).map_err(|e| Error::hls_parsing(url, format!("{e:?}")))?;

    match parsed {
        m3u8_rs::Playlist::MasterPlaylist(master) => {
            let variants: Vec<HlsVariant> = master
                .variants
                .into_iter()
                .map(|v| {
                    let resolution = v.resolution.map(|r| format!("{}x{}", r.width, r.height));
                    HlsVariant {
                        url: resolve_url(url, &v.uri),
                        bandwidth: v.bandwidth,
                        resolution,
                        codecs: v.codecs,
                    }
                })
                .collect();

            tracing::debug!(url = url, variant_count = variants.len(), "✅ Parsed master playlist");
            Ok(variants)
        }
        m3u8_rs::Playlist::MediaPlaylist(_) => {
            Err(Error::hls_parsing(url, "expected master playlist, got media playlist"))
        }
    }
}

/// Fetches and parses an HLS media playlist.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for fetching.
/// * `url` - The URL of the media playlist.
///
/// # Errors
///
/// Returns an error if the fetch or parsing fails.
///
/// # Returns
///
/// A parsed [`HlsPlaylist`] with all segments.
pub async fn parse_media(client: &reqwest::Client, url: &str) -> Result<HlsPlaylist> {
    tracing::debug!(url = url, "📡 Fetching HLS media playlist");

    let body = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::http(url, "fetching media playlist", e))?
        .bytes()
        .await
        .map_err(|e| Error::http(url, "reading media playlist body", e))?;

    let parsed = m3u8_rs::parse_playlist_res(&body).map_err(|e| Error::hls_parsing(url, format!("{e:?}")))?;

    match parsed {
        m3u8_rs::Playlist::MediaPlaylist(media) => {
            let media_sequence = media.media_sequence;
            let target_duration = media.target_duration as f64;
            let is_endlist = media.end_list;

            let segments: Vec<HlsSegment> = media
                .segments
                .into_iter()
                .enumerate()
                .map(|(i, seg)| HlsSegment {
                    url: resolve_url(url, &seg.uri),
                    duration: seg.duration as f64,
                    sequence: media_sequence + i as u64,
                })
                .collect();

            let playlist = HlsPlaylist {
                target_duration,
                media_sequence,
                segments,
                is_endlist,
            };

            tracing::debug!(
                url = url,
                segment_count = playlist.segments.len(),
                media_sequence = playlist.media_sequence,
                is_endlist = playlist.is_endlist,
                "✅ Parsed media playlist"
            );

            Ok(playlist)
        }
        m3u8_rs::Playlist::MasterPlaylist(_) => {
            Err(Error::hls_parsing(url, "expected media playlist, got master playlist"))
        }
    }
}

/// Selects the best variant from a list based on target bandwidth.
///
/// If `target_bandwidth` is `None`, selects the highest bandwidth variant.
/// Otherwise, selects the variant closest to the target without exceeding it.
///
/// # Arguments
///
/// * `variants` - The available HLS variants.
/// * `target_bandwidth` - Optional maximum bandwidth in bits per second.
///
/// # Returns
///
/// A reference to the best matching variant, or `None` if the list is empty.
pub fn select_variant(variants: &[HlsVariant], target_bandwidth: Option<u64>) -> Option<&HlsVariant> {
    if variants.is_empty() {
        return None;
    }

    match target_bandwidth {
        Some(max_bw) => {
            // Pick the highest bandwidth that does not exceed the target
            let matching = variants
                .iter()
                .filter(|v| v.bandwidth <= max_bw)
                .max_by_key(|v| v.bandwidth);
            // Fallback to lowest bandwidth if none match
            matching.or_else(|| variants.iter().min_by_key(|v| v.bandwidth))
        }
        None => variants.iter().max_by_key(|v| v.bandwidth),
    }
}
