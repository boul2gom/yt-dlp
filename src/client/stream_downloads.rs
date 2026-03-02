//! Stream-specific download execution methods for [`DownloadBuilder`].
//!
//! Contains the individual stream download methods (video, audio, storyboard, thumbnail),
//! the shared `enqueue_download` and `execute_stream_internal` helpers, and the
//! `clip_stream` free function for partial/clip downloads via `media_seek`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use media_seek::RangeFetcher;
use tokio::io::AsyncWriteExt;

use crate::client::Downloader;
use crate::client::download_builder::DownloadBuilder;
use crate::client::streams::selection::VideoSelection;
use crate::download::DownloadStatus;
use crate::download::engine::range_fetcher::HttpRangeFetcher;
use crate::error::Result;
use crate::model::Video;
use crate::model::format::FormatType;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, StoryboardQuality, ThumbnailQuality, VideoCodecPreference, VideoQuality,
};

impl<'a> DownloadBuilder<'a> {
    /// Executes the download for the video stream only.
    ///
    /// # Errors
    ///
    /// Returns an error if the video stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video stream file.
    pub async fn execute_video_stream(self) -> Result<PathBuf> {
        let video_quality = self.video_quality.unwrap_or(VideoQuality::Best);
        let video_codec = self.video_codec.unwrap_or(VideoCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            video_quality = ?video_quality,
            video_codec = ?video_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "📥 Executing video stream download"
        );

        let video_format = self
            .video
            .select_video_format(video_quality, video_codec)
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Video))?;

        let video_url = video_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &video_format.format_id))?;

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Video",
            video_url,
            Some(video_format.download_info.http_headers.clone()),
        )
        .await
    }

    /// Executes the download for the audio stream only.
    ///
    /// # Errors
    ///
    /// Returns an error if the audio stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded audio stream file.
    pub async fn execute_audio_stream(self) -> Result<PathBuf> {
        let audio_quality = self.audio_quality.unwrap_or(AudioQuality::Best);
        let audio_codec = self.audio_codec.unwrap_or(AudioCodecPreference::Any);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            audio_quality = ?audio_quality,
            audio_codec = ?audio_codec,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            has_partial_range = self.partial_range.is_some(),
            "📥 Executing audio stream download"
        );

        let audio_format = self
            .video
            .select_audio_format(audio_quality, audio_codec)
            .ok_or_else(|| Self::format_not_available(self.video, FormatType::Audio))?;

        let audio_url = audio_format
            .download_info
            .url
            .as_ref()
            .ok_or_else(|| Self::format_no_url(&self.video.id, &audio_format.format_id))?;

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Audio",
            audio_url,
            Some(audio_format.download_info.http_headers.clone()),
        )
        .await
    }

    /// Executes the download for the storyboard.
    ///
    /// # Errors
    ///
    /// Returns an error if a fragment download fails.
    ///
    /// # Returns
    ///
    /// Returns a vector of paths for the downloaded fragments.
    pub async fn execute_storyboard(self) -> Result<Vec<PathBuf>> {
        let quality = self.storyboard_quality.unwrap_or(StoryboardQuality::Best);

        tracing::debug!(
            video_id = %self.video.id,
            output_dir = ?self.output,
            quality = ?quality,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            "📥 Executing storyboard download"
        );

        let format =
            self.video
                .select_storyboard_format(quality)
                .ok_or_else(|| crate::error::Error::FormatNotAvailable {
                    video_id: self.video.id.clone(),
                    format_type: FormatType::Storyboard,
                    available_formats: vec![],
                })?;

        let fragments = format.storyboard_info.fragments.as_deref().unwrap_or_default();

        let prefix = format.video_id.as_deref().unwrap_or(format.format_id.as_str());

        let output_dir_path = if self.output.is_absolute() {
            self.output.clone()
        } else {
            self.downloader.output_dir.join(&self.output)
        };

        let mut paths = Vec::with_capacity(fragments.len());
        let mut download_ids = Vec::with_capacity(fragments.len());

        // Progress tracking for fragments
        let fragment_count = fragments.len() as f64;
        let progress_callback = self.progress_callback.map(Arc::new);

        for (index, fragment) in fragments.iter().enumerate() {
            let filename = format!("{}_sb_{}_{:04}.mhtml", prefix, format.format_id, index);
            let output_path = output_dir_path.join(&filename);
            paths.push(output_path.clone());

            let callback = progress_callback.clone();

            let raw_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>> = callback.map(|cb| {
                Box::new(move |downloaded: u64, total: u64| {
                    let fragment_base = index as f64 / fragment_count;
                    let fragment_progress = if total > 0 {
                        (downloaded as f64 / total as f64) / fragment_count
                    } else {
                        0.0
                    };
                    cb(fragment_base + fragment_progress);
                }) as Box<dyn Fn(u64, u64) + Send + Sync>
            });

            let id = Self::enqueue_download(
                self.downloader,
                &fragment.url,
                output_path,
                self.priority,
                None, // storyboard doesn't use custom headers from format
                raw_callback,
            )
            .await;

            download_ids.push(id);
        }

        for id in download_ids {
            match self.downloader.wait_for_download(id).await {
                Some(DownloadStatus::Completed) => continue,
                Some(DownloadStatus::Failed { reason }) => {
                    return Err(crate::error::Error::download_failed(
                        id,
                        format!("Storyboard fragment download failed: {}", reason),
                    ));
                }
                Some(DownloadStatus::Canceled) => {
                    return Err(crate::error::Error::DownloadCancelled { download_id: id });
                }
                _ => {
                    return Err(crate::error::Error::download_failed(id, "Unexpected download status"));
                }
            }
        }

        Ok(paths)
    }

    /// Executes the download for the thumbnail.
    ///
    /// # Errors
    ///
    /// Returns an error if the thumbnail stream fetch fails.
    ///
    /// # Returns
    ///
    /// The path to the downloaded thumbnail file.
    pub async fn execute_thumbnail(self) -> Result<PathBuf> {
        let quality = self.thumbnail_quality.unwrap_or(ThumbnailQuality::Best);

        tracing::debug!(
            video_id = %self.video.id,
            output = ?self.output,
            quality = ?quality,
            priority = ?self.priority,
            has_progress_callback = self.progress_callback.is_some(),
            "📥 Executing thumbnail download"
        );

        let thumbnail = self
            .video
            .select_thumbnail(quality)
            .ok_or_else(|| crate::error::Error::NoThumbnail {
                video_id: self.video.id.clone(),
            })?;

        let http_headers = self
            .downloader
            .user_agent
            .clone()
            .map(|ua| crate::model::format::HttpHeaders {
                user_agent: ua,
                accept: "*/*".to_string(),
                accept_language: "en-US,en".to_string(),
                sec_fetch_mode: "navigate".to_string(),
            });

        Self::execute_stream_internal(
            self.downloader,
            &self.output,
            self.priority,
            self.progress_callback,
            "Thumbnail",
            &thumbnail.url,
            http_headers,
        )
        .await
    }

    async fn enqueue_download(
        downloader: &crate::Downloader,
        url: &str,
        output_path: PathBuf,
        priority: crate::download::DownloadPriority,
        http_headers: Option<crate::model::format::HttpHeaders>,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>>,
    ) -> u64 {
        tracing::debug!(
            output_path = ?output_path,
            priority = ?priority,
            has_headers = http_headers.is_some(),
            has_progress = progress_callback.is_some(),
            "📥 Enqueueing download"
        );

        if let Some(cb) = progress_callback {
            downloader
                .download_manager
                .enqueue_with_progress_and_headers(url, output_path, Some(priority), cb, http_headers)
                .await
        } else {
            downloader
                .download_manager
                .enqueue_with_headers(url, output_path, Some(priority), http_headers)
                .await
        }
    }

    async fn execute_stream_internal(
        downloader: &crate::Downloader,
        output: &std::path::Path,
        priority: crate::download::DownloadPriority,
        progress_callback: Option<Box<dyn Fn(f64) + Send + Sync>>,
        format_type_name: &str,
        url: &str,
        http_headers: Option<crate::model::format::HttpHeaders>,
    ) -> Result<PathBuf> {
        tracing::debug!(
            output = ?output,
            priority = ?priority,
            format_type = format_type_name,
            "📥 Executing stream download"
        );

        let path = if output.is_absolute() {
            output.to_path_buf()
        } else {
            downloader.output_dir.join(output)
        };

        let raw_callback: Option<Box<dyn Fn(u64, u64) + Send + Sync>> = progress_callback.map(|cb| {
            Box::new(move |downloaded: u64, total: u64| {
                if total > 0 {
                    cb(downloaded as f64 / total as f64);
                }
            }) as Box<dyn Fn(u64, u64) + Send + Sync>
        });

        let download_id =
            Self::enqueue_download(downloader, url, path.clone(), priority, http_headers, raw_callback).await;

        match downloader.wait_for_download(download_id).await {
            Some(DownloadStatus::Completed) => Ok(path),
            Some(DownloadStatus::Failed { reason }) => Err(crate::error::Error::download_failed(
                download_id,
                format!("{} download failed: {}", format_type_name, reason),
            )),
            Some(DownloadStatus::Canceled) => Err(crate::error::Error::DownloadCancelled { download_id }),
            _ => Err(crate::error::Error::download_failed(
                download_id,
                "Unexpected download status",
            )),
        }
    }

    pub(super) fn format_not_available(video: &Video, format_type: FormatType) -> crate::error::Error {
        crate::error::Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        }
    }

    pub(super) fn format_no_url(video_id: &str, format_id: &str) -> crate::error::Error {
        crate::error::Error::FormatNoUrl {
            video_id: video_id.to_string(),
            format_id: format_id.to_string(),
        }
    }
}

/// Downloads only the bytes covering `[start_secs, end_secs]` from `url` using `media_seek`.
///
/// Uses `media_seek` to parse the container index and resolve timestamps to byte offsets.
/// The init segment is fetched with a single request (typically a few KB). The clip data is
/// routed through `DownloadManager.enqueue_range` so it benefits from parallel segments,
/// speed profiles, retry logic, and progress tracking. Both are concatenated to `output`.
///
/// The caller should follow up with an FFmpeg `-c copy` trim pass to sharpen
/// keyframe-aligned boundaries to the exact requested timestamps.
///
/// Returns `Err(media_seek::Error::UnsupportedFormat)` or `Err(media_seek::Error::ParseFailed)`
/// when the container format cannot be sought via byte ranges — callers should fall back to a
/// full download in those cases. `Err(media_seek::Error::FetchFailed)` indicates an
/// unrecoverable I/O or network failure.
///
/// # Arguments
///
/// * `downloader` - The downloader instance, used for the shared HTTP client and download manager.
/// * `url` - The stream URL to fetch from.
/// * `http_headers` - Format-specific HTTP headers (e.g. signed CDN cookies) sent on every request.
/// * `total_size` - Total byte length of the stream, used by `media_seek` for bisection-based formats.
/// * `start_secs` - Start of the requested clip in seconds.
/// * `end_secs` - End of the requested clip in seconds.
/// * `output` - Destination path where the init + clip bytes are written.
///
/// # Errors
///
/// Returns `media_seek::Error::UnsupportedFormat` or `ParseFailed` for unsupported containers.
/// Returns `media_seek::Error::FetchFailed` on network or I/O failures.
pub(super) async fn clip_stream(
    downloader: &Downloader,
    url: &str,
    http_headers: &crate::model::format::HttpHeaders,
    total_size: Option<u64>,
    start_secs: f64,
    end_secs: f64,
    output: &Path,
) -> std::result::Result<(), media_seek::Error> {
    // Probe (512 KB) + parse container index
    let headers = http_headers.to_header_map();
    let rf = HttpRangeFetcher::new(Arc::clone(downloader.download_manager.client()), url, headers);

    const PROBE_SIZE: u64 = 512 * 1024;
    let probe = rf
        .fetch(0, PROBE_SIZE - 1)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    let index = media_seek::parse(&probe, total_size, &rf).await?;

    let range = index
        .find_byte_range(start_secs, end_secs)
        .ok_or_else(|| media_seek::Error::ParseFailed {
            reason: "time range not covered by container index".into(),
        })?;

    tracing::debug!(
        start_secs,
        end_secs,
        init_end = index.init_end_byte,
        content_start = range.start,
        content_end = range.end,
        "⚙️ media-seek byte range resolved"
    );

    // Init segment — single request (typically < 100 KB)
    let init_bytes = rf
        .fetch(0, index.init_end_byte)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    // Clip data — routed through DownloadManager (parallel segments, retry, progress)
    let clip_tmp = output.with_file_name(format!("clip_tmp_{}.bin", crate::utils::fs::random_filename(8)));

    let clip_id = downloader
        .download_manager
        .enqueue_range(url, &clip_tmp, range.start, range.end, None, Some(http_headers.clone()))
        .await;

    match downloader.download_manager.wait_for_completion(clip_id).await {
        Some(DownloadStatus::Completed) => {}
        Some(DownloadStatus::Failed { reason }) => {
            return Err(media_seek::Error::FetchFailed(Box::new(std::io::Error::other(
                format!("clip segment download failed: {reason}"),
            ))));
        }
        _ => {
            return Err(media_seek::Error::FetchFailed(Box::new(std::io::Error::other(
                "clip segment download did not complete",
            ))));
        }
    }

    // Concatenate: init_bytes + clip_tmp → output
    let mut out_file = tokio::fs::File::create(output)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    out_file
        .write_all(&init_bytes)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    let mut clip_file = tokio::fs::File::open(&clip_tmp)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    tokio::io::copy(&mut clip_file, &mut out_file)
        .await
        .map_err(|e| media_seek::Error::FetchFailed(Box::new(e)))?;

    drop(clip_file);
    tokio::fs::remove_file(&clip_tmp).await.ok();

    tracing::debug!(
        init_bytes = init_bytes.len(),
        clip_bytes = range.end - range.start + 1,
        "✅ media-seek clip stream written via DownloadManager"
    );

    Ok(())
}
