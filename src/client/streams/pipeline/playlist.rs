use std::path::PathBuf;
use std::sync::Arc;

use futures_util::stream::{FuturesUnordered, StreamExt};

use crate::Downloader;
use crate::error::Error;
use crate::model::playlist::{Playlist, PlaylistDownloadProgress};

impl Downloader {
    /// Fetches playlist information from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the playlist.
    ///
    /// # Returns
    ///
    /// A `Playlist` struct containing metadata about the playlist and its videos.
    ///
    /// # Errors
    ///
    /// Returns an error if the playlist cannot be fetched or parsed.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let playlist = downloader.fetch_playlist_infos("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf").await?;
    /// println!("Playlist title: {}", playlist.title);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_playlist_infos(&self, url: impl AsRef<str>) -> crate::error::Result<Playlist> {
        let url_str = url.as_ref();
        tracing::info!(url = url_str, "📋 Fetching playlist information");

        // Check if the playlist is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache
            && let Some(playlist) = cache.playlists.get(url_str).await?
        {
            tracing::debug!(url = url_str, "🔍 Using cached playlist information");
            return Ok(playlist);
        }

        // Delegate to the extractor
        let extractor = self.get_extractor(url_str);
        tracing::debug!(extractor = %extractor.name(), "📡 Fetching playlist information from extractor");

        let start = std::time::Instant::now();
        let result = extractor.fetch_playlist(url_str).await;
        let duration = start.elapsed();

        let mut playlist = match result {
            Ok(p) => {
                tracing::debug!(
                    url = url_str,
                    playlist_id = %p.id,
                    entry_count = p.entry_count(),
                    duration = ?duration,
                    "✅ Playlist information fetched"
                );

                self.emit_event(crate::events::DownloadEvent::PlaylistFetched {
                    url: url_str.to_string(),
                    playlist: p.clone(),
                    duration,
                })
                .await;

                p
            }
            Err(e) => {
                tracing::debug!(
                    url = url_str,
                    error = %e,
                    duration = ?duration,
                    "📋 Playlist information fetch failed"
                );

                self.emit_event(crate::events::DownloadEvent::PlaylistFetchFailed {
                    url: url_str.to_string(),
                    error: e.to_string(),
                    duration,
                })
                .await;

                return Err(e);
            }
        };

        // Store the URL in the playlist for caching purposes
        playlist.url = Some(url_str.to_string());

        // Cache the playlist if caching is enabled
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(url = url_str, "🔍 Caching playlist information");

            if let Err(_e) = cache.playlists.put(url_str.to_string(), playlist.clone()).await {
                tracing::warn!(error = %_e, "Failed to cache playlist information");
            }
        }

        tracing::info!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            "✅ Playlist fetched"
        );

        Ok(playlist)
    }

    /// Downloads all videos from a playlist.
    ///
    /// This method uses the `download_playlist_parallel` method internally with default concurrency settings.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames (e.g., "%(title)s.%(ext)s").
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let playlist = downloader.fetch_playlist_infos("https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf.").await?;
    ///
    /// // Download all videos in the playlist
    /// let paths = downloader.download_playlist(&playlist, "%(title)s.%(ext)s").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_playlist(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::info!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            "📋 Downloading playlist"
        );

        // Use None to let download_playlist_parallel use its default concurrent limit
        // The limit is already configured in the download manager based on the speed profile
        let max_concurrent = None;

        // Use parallel download mode by default for better performance
        let results = self
            .download_playlist_parallel(playlist, output_pattern, max_concurrent)
            .await?;

        // Convert results to a simple Vec<PathBuf>, filtering out errors
        // and collecting only successful downloads
        let mut downloaded_files = Vec::new();
        let mut errors = Vec::new();

        for (video_idx, result) in results.into_iter().enumerate() {
            match result {
                Ok(path) => downloaded_files.push(path),
                Err(e) => {
                    tracing::error!(index = video_idx, error = %e, "Failed to download video");
                    errors.push(e);
                }
            }
        }

        // If there were any errors, return the first one
        // (to maintain backward compatibility with the previous sequential behavior)
        if !errors.is_empty()
            && downloaded_files.is_empty()
            && let Some(e) = errors.into_iter().next()
        {
            return Err(e);
        }

        tracing::info!(
            downloaded = downloaded_files.len(),
            total = playlist.entry_count(),
            playlist_id = playlist.id,
            "✅ Playlist download completed"
        );

        Ok(downloaded_files)
    }

    /// Downloads all videos from a playlist in parallel.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames.
    /// * `max_concurrent` - Optional limit on concurrent downloads (defaults to 3).
    ///
    /// # Returns
    ///
    /// A vector of Results, each containing the path to a downloaded video or an error.
    pub async fn download_playlist_parallel(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>> {
        self.download_playlist_parallel_with_progress::<fn(PlaylistDownloadProgress)>(
            playlist,
            output_pattern,
            max_concurrent,
            None,
        )
        .await
    }

    /// Downloads all videos from a playlist in parallel with progress tracking.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `output_pattern` - pattern for output filenames.
    /// * `max_concurrent` - Optional limit on concurrent downloads.
    /// * `progress_callback` - Optional closure called with `PlaylistDownloadProgress` updates.
    ///
    /// # Returns
    ///
    /// A vector of Results, each containing the path to a downloaded video or an error.
    pub async fn download_playlist_parallel_with_progress<F>(
        &self,
        playlist: &Playlist,
        output_pattern: impl AsRef<str>,
        max_concurrent: Option<usize>,
        progress_callback: Option<F>,
    ) -> crate::error::Result<Vec<crate::error::Result<PathBuf>>>
    where
        F: Fn(PlaylistDownloadProgress) + Send + Sync + 'static,
    {
        tracing::debug!(
            playlist_id = playlist.id,
            count = playlist.entry_count(),
            max_concurrent = max_concurrent.unwrap_or(3),
            "📋 Downloading playlist in parallel"
        );

        let max_concurrent = max_concurrent.unwrap_or(3);
        let total_videos = playlist.entry_count();
        let mut completed = 0usize;
        let mut results = Vec::new();
        let mut tasks = FuturesUnordered::new();
        let mut entry_iter = playlist.entries.iter().peekable();

        let output_pattern = output_pattern.as_ref().to_string();
        let progress_callback = progress_callback.map(Arc::new);
        let playlist_start = std::time::Instant::now();

        loop {
            // Check for cancellation before spawning new tasks
            if self.cancellation_token.is_cancelled() {
                tracing::info!(playlist_id = %playlist.id, "🛑 Playlist download cancelled");
                break;
            }

            // Spawn tasks up to max_concurrent limit
            while tasks.len() < max_concurrent {
                let Some(entry) = entry_iter.next() else {
                    break;
                };

                if !entry.is_available() {
                    completed += 1;
                    self.handle_unavailable_playlist_entry(
                        entry,
                        playlist,
                        total_videos,
                        completed,
                        &progress_callback,
                        &mut results,
                    )
                    .await;
                    continue;
                }

                let task = self
                    .spawn_playlist_download_task(entry, &output_pattern, playlist, total_videos)
                    .await;
                tasks.push(task);
            }

            if tasks.is_empty() {
                break;
            }

            if let Some(result) = tasks.next().await {
                completed += 1;
                self.handle_playlist_task_result(
                    result,
                    playlist,
                    total_videos,
                    completed,
                    &progress_callback,
                    &mut results,
                )
                .await;
            }
        }

        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - successful;
        let playlist_duration = playlist_start.elapsed();

        self.emit_event(crate::events::DownloadEvent::PlaylistCompleted {
            playlist_id: playlist.id.clone(),
            total_items: total_videos,
            successful,
            failed,
            duration: playlist_duration,
        })
        .await;

        tracing::info!(
            successful = successful,
            total = playlist.entry_count(),
            playlist_id = playlist.id,
            "✅ Parallel playlist download completed"
        );

        Ok(results)
    }

    async fn handle_unavailable_playlist_entry<F>(
        &self,
        entry: &crate::model::playlist::PlaylistEntry,
        playlist: &Playlist,
        total_videos: usize,
        completed: usize,
        progress_callback: &Option<Arc<F>>,
        results: &mut Vec<crate::error::Result<PathBuf>>,
    ) where
        F: Fn(PlaylistDownloadProgress) + Send + Sync + 'static,
    {
        tracing::warn!(title = entry.title, id = entry.id, "Skipping unavailable video");

        self.emit_event(crate::events::DownloadEvent::PlaylistItemFailed {
            playlist_id: playlist.id.clone(),
            index: entry.index.unwrap_or(0),
            total: total_videos,
            video_id: entry.id.clone(),
            error: format!("Video {} is not available", entry.id),
        })
        .await;

        if let Some(callback) = progress_callback {
            callback(PlaylistDownloadProgress {
                entry: entry.clone(),
                result: Err(format!("Video {} is not available", entry.id)),
                completed,
                total: total_videos,
            });
        }

        results.push(Err(Error::video_fetch(
            &entry.url,
            format!("Video {} is not available", entry.id),
        )));
    }

    async fn spawn_playlist_download_task(
        &self,
        entry: &crate::model::playlist::PlaylistEntry,
        output_pattern: &str,
        playlist: &Playlist,
        total_videos: usize,
    ) -> tokio::task::JoinHandle<(crate::model::playlist::PlaylistEntry, crate::error::Result<PathBuf>)> {
        let entry = entry.clone();
        let output_pattern = output_pattern.to_string();
        let youtube = self.clone();
        let playlist_id = playlist.id.clone();

        self.emit_event(crate::events::DownloadEvent::PlaylistItemStarted {
            playlist_id,
            index: entry.index.unwrap_or(0),
            total: total_videos,
            video_id: entry.id.clone(),
        })
        .await;

        tokio::spawn(async move {
            tracing::debug!(
                video_id = entry.id,
                index = entry.index.unwrap_or(0),
                "📥 Downloading video from playlist"
            );

            let video = match youtube.fetch_video_infos(entry.url.clone()).await {
                Ok(v) => v,
                Err(e) => return (entry, Err(e)),
            };

            let filename = output_pattern
                .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                .replace("%(title)s", &crate::utils::validation::sanitize_filename(&entry.title))
                .replace("%(id)s", &entry.id);

            let download_result = youtube.download_video(&video, &filename).await;

            if download_result.is_ok() {
                tracing::info!(
                    title = entry.title,
                    index = entry.index.unwrap_or(0),
                    "✅ Downloaded video from playlist"
                );
            }

            (entry, download_result)
        })
    }

    async fn handle_playlist_task_result<F>(
        &self,
        result: std::result::Result<
            (crate::model::playlist::PlaylistEntry, crate::error::Result<PathBuf>),
            tokio::task::JoinError,
        >,
        playlist: &Playlist,
        total_videos: usize,
        completed: usize,
        progress_callback: &Option<Arc<F>>,
        results: &mut Vec<crate::error::Result<PathBuf>>,
    ) where
        F: Fn(PlaylistDownloadProgress) + Send + Sync + 'static,
    {
        match result {
            Ok((entry, download_result)) => {
                self.emit_playlist_item_event(&entry, &download_result, playlist, total_videos)
                    .await;

                if let Some(callback) = progress_callback {
                    let result_for_progress = download_result.as_ref().map(|p| p.clone()).map_err(|e| e.to_string());
                    callback(PlaylistDownloadProgress {
                        entry,
                        result: result_for_progress,
                        completed,
                        total: total_videos,
                    });
                }

                results.push(download_result);
            }
            Err(e) => {
                results.push(Err(Error::runtime("playlist download task", e)));
            }
        }
    }

    async fn emit_playlist_item_event(
        &self,
        entry: &crate::model::playlist::PlaylistEntry,
        download_result: &crate::error::Result<PathBuf>,
        playlist: &Playlist,
        total_videos: usize,
    ) {
        match download_result {
            Ok(path) => {
                self.emit_event(crate::events::DownloadEvent::PlaylistItemCompleted {
                    playlist_id: playlist.id.clone(),
                    index: entry.index.unwrap_or(0),
                    total: total_videos,
                    video_id: entry.id.clone(),
                    output_path: path.clone(),
                })
                .await;
            }
            Err(e) => {
                self.emit_event(crate::events::DownloadEvent::PlaylistItemFailed {
                    playlist_id: playlist.id.clone(),
                    index: entry.index.unwrap_or(0),
                    total: total_videos,
                    video_id: entry.id.clone(),
                    error: e.to_string(),
                })
                .await;
            }
        }
    }

    /// Downloads specific videos from a playlist by their indices.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `indices` - A slice of indices (0-based) of videos to download.
    /// * `output_pattern` - pattern for output filenames.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
    pub async fn download_playlist_items(
        &self,
        playlist: &Playlist,
        indices: &[usize],
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            count = indices.len(),
            playlist_id = playlist.id,
            "📋 Downloading specific videos from playlist"
        );

        let mut downloaded_files = Vec::new();

        for &index in indices {
            if let Some(entry) = playlist.get_entry_by_index(index) {
                if !entry.is_available() {
                    tracing::warn!(index = index, title = entry.title, "Skipping unavailable video");
                    continue;
                }

                // Fetch full video info
                let video = self.fetch_video_infos(entry.url.clone()).await?;

                // Generate filename from pattern
                let filename = output_pattern
                    .as_ref()
                    .replace("%(playlist_index)s", &index.to_string())
                    .replace("%(title)s", &crate::utils::validation::sanitize_filename(&entry.title))
                    .replace("%(id)s", &entry.id);

                // Download the video
                let video_path = self.download_video(&video, &filename).await?;
                downloaded_files.push(video_path);

                tracing::info!(index = index, title = entry.title, "✅ Downloaded video");
            } else {
                tracing::warn!(index = index, "Index out of bounds for playlist");
            }
        }

        Ok(downloaded_files)
    }

    /// Downloads a range of videos from a playlist.
    ///
    /// # Arguments
    ///
    /// * `playlist` - The `Playlist` metadata struct.
    /// * `start` - The start index (inclusive).
    /// * `end` - The end index (inclusive).
    /// * `output_pattern` - pattern for output filenames.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded video files.
    pub async fn download_playlist_range(
        &self,
        playlist: &Playlist,
        start: usize,
        end: usize,
        output_pattern: impl AsRef<str>,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            start = start,
            end = end,
            playlist_id = playlist.id,
            "📋 Downloading playlist range"
        );

        let entries = playlist.get_entries_in_range(start, end);
        let mut downloaded_files = Vec::new();

        for entry in entries {
            if !entry.is_available() {
                tracing::warn!(title = entry.title, id = entry.id, "Skipping unavailable video");
                continue;
            }

            // Fetch full video info
            let video = self.fetch_video_infos(entry.url.clone()).await?;

            // Generate filename from pattern
            let filename = output_pattern
                .as_ref()
                .replace("%(playlist_index)s", &entry.index.unwrap_or(0).to_string())
                .replace("%(title)s", &crate::utils::validation::sanitize_filename(&entry.title))
                .replace("%(id)s", &entry.id);

            // Download the video
            let video_path = self.download_video(&video, &filename).await?;
            downloaded_files.push(video_path);

            tracing::info!(title = entry.title, "✅ Downloaded video");
        }

        Ok(downloaded_files)
    }
}
