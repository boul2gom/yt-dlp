use std::future::Future;
use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::client::streams::selection::VideoSelection;
use crate::error::Error;
use crate::executor::Executor;
use crate::model::Video;
use crate::model::format::{FormatType, HttpHeaders};
use crate::{Downloader, utils};

/// Initial probe size for container index parsing (512 KB).
const PROBE_SIZE: u64 = 512 * 1024;

/// Private reqwest-based implementation of `media_seek::RangeFetcher`.
///
/// Wraps a reqwest client and issues `Range: bytes={start}-{end}` requests
/// against the given URL. Extra headers from the format's `http_headers` are
/// forwarded on every request.
struct ReqwestFetcher<'a> {
    client: &'a reqwest::Client,
    url: &'a str,
    headers: reqwest::header::HeaderMap,
}

impl<'a> ReqwestFetcher<'a> {
    fn new(client: &'a reqwest::Client, url: &'a str, http_headers: Option<&HttpHeaders>) -> Self {
        let headers = http_headers.map(|h| h.to_header_map()).unwrap_or_default();
        Self { client, url, headers }
    }
}

impl media_seek::RangeFetcher for ReqwestFetcher<'_> {
    type Error = crate::error::Error;

    fn fetch(&self, start: u64, end: u64) -> impl Future<Output = crate::error::Result<Vec<u8>>> + Send {
        let client = self.client.clone();
        let url = self.url.to_owned();
        let mut headers = self.headers.clone();
        let range = format!("bytes={}-{}", start, end);
        async move {
            headers.insert(
                reqwest::header::RANGE,
                reqwest::header::HeaderValue::from_str(&range)
                    .map_err(|e| Error::invalid_partial_range(e.to_string()))?,
            );
            let resp = client
                .get(&url)
                .headers(headers)
                .send()
                .await
                .map_err(|e| Error::http(&url, "range fetch for partial seek", e))?;
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| Error::http(&url, "reading range response body", e))?;
            Ok(bytes.to_vec())
        }
    }
}

/// Fetches the probe bytes and total size for `url` using a HEAD or Range GET.
///
/// Returns `(probe_bytes, total_size)`. `total_size` is `None` when the server
/// does not return a `Content-Range` or `Content-Length` header.
async fn fetch_probe(
    client: &reqwest::Client,
    url: &str,
    headers: reqwest::header::HeaderMap,
) -> crate::error::Result<(Vec<u8>, Option<u64>)> {
    let range = format!("bytes=0-{}", PROBE_SIZE - 1);
    let mut req_headers = headers;
    req_headers.insert(
        reqwest::header::RANGE,
        reqwest::header::HeaderValue::from_str(&range).map_err(|e| Error::invalid_partial_range(e.to_string()))?,
    );

    let resp = client
        .get(url)
        .headers(req_headers)
        .send()
        .await
        .map_err(|e| Error::http(url, "fetching probe for container index", e))?;

    // Total size from Content-Range: bytes 0-{end}/{total}
    let total_size = resp
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split('/').next_back())
        .and_then(|s| s.trim().parse::<u64>().ok());

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| Error::http(url, "reading probe body", e))?;

    Ok((bytes.to_vec(), total_size))
}

/// Downloads a byte range from `url` and appends the bytes to `dest`.
///
/// If `dest` does not yet exist it is created; if it does exist the bytes are appended.
async fn fetch_range_to_file(
    client: &reqwest::Client,
    url: &str,
    start: u64,
    end: u64,
    headers: reqwest::header::HeaderMap,
    dest: &Path,
) -> crate::error::Result<()> {
    let range = format!("bytes={}-{}", start, end);
    let mut req_headers = headers;
    req_headers.insert(
        reqwest::header::RANGE,
        reqwest::header::HeaderValue::from_str(&range).map_err(|e| Error::invalid_partial_range(e.to_string()))?,
    );
    let resp = client
        .get(url)
        .headers(req_headers)
        .send()
        .await
        .map_err(|e| Error::http(url, "fetching byte range", e))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| Error::http(url, "reading byte range body", e))?;

    // Append using OpenOptions instead of read-all + rewrite
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dest)
        .await
        .map_err(|e| Error::io_with_path("opening file for append", dest, e))?;
    file.write_all(&bytes)
        .await
        .map_err(|e| Error::io_with_path("appending to file", dest, e))?;
    file.flush()
        .await
        .map_err(|e| Error::io_with_path("flushing file", dest, e))?;

    Ok(())
}

/// Fetches the container index for `url` and downloads the init+content byte ranges to `dest`.
///
/// This replaces the old subprocess-based approach: it probes the first 512 KB,
/// parses the container index with `media-seek`, then fetches the two ranges
/// (init segment + content window) in sequence and concatenates them.
pub(crate) async fn fetch_partial_stream(
    client: &reqwest::Client,
    url: &str,
    start_secs: f64,
    end_secs: f64,
    http_headers: Option<&HttpHeaders>,
    dest: &Path,
) -> crate::error::Result<()> {
    let header_map = http_headers.map(|h| h.to_header_map()).unwrap_or_default();

    let (probe, total_size) = fetch_probe(client, url, header_map.clone()).await?;

    let fetcher = ReqwestFetcher::new(client, url, http_headers);
    let index = media_seek::parse(&probe, total_size, &fetcher)
        .await
        .map_err(crate::error::Error::from)?;

    let range = index
        .find_byte_range(start_secs, end_secs)
        .ok_or_else(|| Error::invalid_partial_range("container index is empty"))?;

    // Write init segment first (bytes 0..=init_end_byte)
    if index.init_end_byte > 0 {
        fetch_range_to_file(client, url, 0, index.init_end_byte, header_map.clone(), dest).await?;
    }

    // Append the content window
    fetch_range_to_file(client, url, range.start, range.end, header_map, dest).await?;

    Ok(())
}

impl Downloader {
    /// Downloads a specific time window of a video using HTTP Range requests and pure Rust parsing.
    ///
    /// Selects the best video and audio formats, fetches only the byte ranges
    /// corresponding to `range` via `media-seek` container index parsing, combines
    /// the streams with FFmpeg, and trims the result to exact boundaries.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `range` - The partial range (time or chapters) to download.
    /// * `output` - The output filename/path relative to the download directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the range is invalid, the container format is unsupported,
    /// HTTP Range requests fail, or FFmpeg combining/trimming fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded partial video file.
    pub async fn download_video_partial(
        &self,
        video: &Video,
        range: &crate::download::engine::partial::PartialRange,
        output: impl AsRef<str>,
    ) -> crate::error::Result<PathBuf> {
        let time_range = if range.needs_chapter_metadata() {
            if !video.chapters.is_empty() {
                range
                    .to_time_range(&video.chapters)
                    .ok_or_else(|| Error::VideoMissingField {
                        video_id: video.id.clone(),
                        field: "chapter at requested index".to_string(),
                    })?
            } else {
                return Err(Error::VideoMissingField {
                    video_id: video.id.clone(),
                    field: "chapters".to_string(),
                });
            }
        } else {
            range.clone()
        };

        let (start_secs, end_secs) = time_range
            .get_times()
            .ok_or_else(|| Error::invalid_partial_range("could not resolve time boundaries from partial range"))?;

        let output_path = self.output_dir.join(output.as_ref());
        utils::create_parent_dir(&output_path).await?;

        let best_video = video.best_video_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Video,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;
        let best_audio = video.best_audio_format().ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Audio,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        let client = crate::utils::http::build_http_client(crate::utils::http::HttpClientConfig {
            proxy: self.proxy.as_ref(),
            timeout: Some(self.timeout),
            user_agent: self.user_agent.clone(),
            ..Default::default()
        })?;

        let video_url = best_video
            .download_info
            .url
            .as_deref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: best_video.format_id.clone(),
            })?;
        let audio_url = best_audio
            .download_info
            .url
            .as_deref()
            .ok_or_else(|| Error::FormatNoUrl {
                video_id: video.id.clone(),
                format_id: best_audio.format_id.clone(),
            })?;

        let video_ext = best_video.download_info.ext.as_str();
        let audio_ext = best_audio.download_info.ext.as_str();
        let video_temp_name = format!("temp_pv_{}.{}", utils::fs::random_filename(8), video_ext);
        let audio_temp_name = format!("temp_pa_{}.{}", utils::fs::random_filename(8), audio_ext);
        let combined_temp_name = format!("temp_pc_{}.mp4", utils::fs::random_filename(8));

        let video_temp = self.output_dir.join(&video_temp_name);
        let audio_temp = self.output_dir.join(&audio_temp_name);
        let combined_temp = self.output_dir.join(&combined_temp_name);

        tracing::info!(
            video_id = %video.id,
            start_secs = start_secs,
            end_secs = end_secs,
            "✂️ Starting partial download via HTTP Range + media-seek"
        );

        let (video_result, audio_result) = tokio::join!(
            fetch_partial_stream(
                &client,
                video_url,
                start_secs,
                end_secs,
                Some(&best_video.download_info.http_headers),
                &video_temp,
            ),
            fetch_partial_stream(
                &client,
                audio_url,
                start_secs,
                end_secs,
                Some(&best_audio.download_info.http_headers),
                &audio_temp,
            ),
        );

        if let Err(e) = video_result {
            utils::remove_temp_file(&video_temp).await;
            utils::remove_temp_file(&audio_temp).await;
            return Err(e);
        }
        if let Err(e) = audio_result {
            utils::remove_temp_file(&video_temp).await;
            utils::remove_temp_file(&audio_temp).await;
            return Err(e);
        }

        let combine_result = self
            .execute_ffmpeg_combine(
                &audio_temp,
                &video_temp,
                &combined_temp,
                None,
                best_audio.codec_info.audio_codec.as_deref(),
            )
            .await;

        utils::remove_temp_file(&video_temp).await;
        utils::remove_temp_file(&audio_temp).await;

        combine_result?;

        // Trim to exact timestamps to remove any over-fetched frames at segment boundaries
        self.extract_time_range(&combined_temp, &output_path, start_secs, end_secs)
            .await?;
        utils::remove_temp_file(&combined_temp).await;

        tracing::info!(
            video_id = %video.id,
            output_path = ?output_path,
            "✅ Partial video downloaded and trimmed"
        );

        Ok(output_path)
    }

    /// Trims a media file to the exact `[start_time, end_time]` window using FFmpeg stream copy.
    ///
    /// Runs: `ffmpeg -i {source} -ss {start} -t {duration} -c copy -avoid_negative_ts 1 -y {output}`
    ///
    /// # Arguments
    ///
    /// * `source` - Path to the input file.
    /// * `output` - Path where the trimmed file is written.
    /// * `start_time` - Start time in seconds.
    /// * `end_time` - End time in seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if FFmpeg fails or path conversion fails.
    pub(crate) async fn extract_time_range(
        &self,
        source: &Path,
        output: &Path,
        start_time: f64,
        end_time: f64,
    ) -> crate::error::Result<()> {
        let source_str = source.to_str().ok_or_else(|| Error::PathValidation {
            path: source.to_path_buf(),
            reason: "Non-UTF8 source path".to_string(),
        })?;
        let output_str = output.to_str().ok_or_else(|| Error::PathValidation {
            path: output.to_path_buf(),
            reason: "Non-UTF8 output path".to_string(),
        })?;

        let start_str = format!("{:.3}", start_time);
        let duration = end_time - start_time;
        let duration_str = format!("{:.3}", duration);

        let args = crate::executor::FfmpegArgs::new()
            .input(source_str)
            .args(["-ss", &start_str, "-t", &duration_str])
            .codec_copy()
            .args(["-avoid_negative_ts", "1"])
            .output(output_str)
            .build();

        Executor::new(self.libraries.ffmpeg.clone(), args, self.timeout)
            .execute()
            .await?;
        Ok(())
    }
}
