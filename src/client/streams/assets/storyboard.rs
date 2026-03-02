use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::Video;
use crate::model::format::{Format, FormatType};
use crate::model::selector::StoryboardQuality;
use crate::{DownloadStatus, Downloader};

impl Downloader {
    /// Downloads all MHTML fragments of a storyboard format.
    ///
    /// Each fragment is a grid of preview images for a contiguous time range.
    /// Files are named `{video_id}_sb_{format_id}_{index}.mhtml` where `video_id` comes
    /// from `format.video_id` when set (populated automatically when fetching via this library).
    ///
    /// # Arguments
    ///
    /// * `format` - A storyboard `Format` obtained via [`VideoSelection::best_storyboard_format`].
    /// * `output_dir` - Directory where fragment files will be written.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded MHTML fragment files.
    ///
    /// # Errors
    ///
    /// Returns an error if the format is not a storyboard, if fragments are missing, or if any
    /// fragment download fails.
    pub async fn download_storyboard_format(
        &self,
        format: &Format,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        let output_dir = output_dir.as_ref();

        if format.format_type() != FormatType::Storyboard {
            return Err(Error::FormatNotAvailable {
                video_id: format.video_id.clone().unwrap_or_default(),
                format_type: FormatType::Storyboard,
                available_formats: vec![format.format_id.clone()],
            });
        }

        let fragments = format.storyboard_info.fragments.as_deref().unwrap_or_default();

        // Use video_id when available (set by the library), fall back to format_id
        let prefix = format.video_id.as_deref().unwrap_or(format.format_id.as_str());

        tracing::debug!(
            video_id = prefix,
            format_id = %format.format_id,
            fragment_count = fragments.len(),
            resolution = ?format.video_resolution.resolution,
            "🖼️ Downloading storyboard fragments"
        );

        let mut paths = Vec::with_capacity(fragments.len());
        let mut download_ids = Vec::with_capacity(fragments.len());

        for (index, fragment) in fragments.iter().enumerate() {
            let filename = format!("{}_sb_{}_{:04}.mhtml", prefix, format.format_id, index);
            let output_path = output_dir.join(&filename);

            tracing::debug!(
                index = index,
                url = %fragment.url,
                path = ?output_path,
                "🖼️ Enqueuing storyboard fragment for download"
            );

            let id = self
                .download_manager
                .enqueue(
                    &fragment.url,
                    output_path.clone(),
                    Some(crate::download::DownloadPriority::Normal),
                )
                .await;

            paths.push(output_path);
            download_ids.push(id);
        }

        for id in download_ids {
            match self.wait_for_download(id).await {
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

        tracing::info!(
            video_id = prefix,
            format_id = %format.format_id,
            downloaded = paths.len(),
            "✅ Storyboard fragments downloaded"
        );

        Ok(paths)
    }

    /// Downloads the storyboard of the requested quality for a video.
    ///
    /// Selects the best or worst storyboard format via [`VideoSelection`] and delegates
    /// to [`Downloader::download_storyboard_format`].
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `quality` - [`StoryboardQuality::Best`] for highest resolution, [`StoryboardQuality::Worst`] for lowest.
    /// * `output_dir` - Directory where fragment files will be written.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded MHTML fragment files.
    ///
    /// # Errors
    ///
    /// Returns an error if no storyboard formats are available or if a download fails.
    pub async fn download_storyboard(
        &self,
        video: &Video,
        quality: StoryboardQuality,
        output_dir: impl AsRef<Path>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            video_id = %video.id,
            quality = ?quality,
            "🖼️ Selecting storyboard format for download"
        );

        let format = match quality {
            StoryboardQuality::Best => video.best_storyboard_format(),
            StoryboardQuality::Worst => video.worst_storyboard_format(),
        }
        .ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Storyboard,
            available_formats: vec![],
        })?;

        self.download_storyboard_format(format, output_dir).await
    }
}
