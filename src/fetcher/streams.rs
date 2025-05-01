//! Tools for fetching video streams from YouTube.

use crate::error::Error;
use crate::executor::Executor;
use crate::fetcher::Fetcher;
use crate::model::Video;
use crate::model::format::Format;
#[cfg(feature = "cache")]
use crate::model::format_selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use crate::{Youtube, utils};
use std::fmt::Display;
use std::path::Path;
use std::path::PathBuf;

impl Youtube {
    /// Fetch the video information from the given URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to fetch.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video information could not be fetched.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_video_infos(&self, url: String) -> crate::error::Result<Video> {
        // Check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            if let Some(video) = cache.get(&url) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached video information for {}", url);
                return Ok(video);
            }
        }

        // If the video is not in the cache, retrieve it from YouTube
        let download_args = vec!["--no-progress", "--dump-json", &url];

        let mut final_args = self.args.clone();
        final_args.append(&mut utils::to_owned(download_args));

        let executor = Executor {
            executable_path: self.libraries.youtube.clone(),
            timeout: self.timeout,
            args: final_args,
        };

        let output = executor.execute().await?;
        let mut video: Video = serde_json::from_str(&output.stdout).map_err(Error::Serde)?;

        // Set the video ID on each format for caching purposes
        for format in &mut video.formats {
            format.video_id = Some(video.id.clone());
        }

        // Put the video in the cache if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching video information for {}", url);

            if let Err(_e) = cache.put(url.clone(), video.clone()) {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache video information: {}", _e);
            }
        }

        Ok(video)
    }

    /// Fetch the video from the given URL, download it (video with audio) and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video_path = fetcher.download_video_from_url(url, "my-video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;
        self.download_video(&video, output).await
    }

    /// Fetch the video, download it (video with audio) and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let video_path = fetcher.download_video(&video, "my-video.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video {}", video.title);

        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                let output_str = output.as_ref();
                let path = self.output_dir.join(output_str);
            }
        }

        // Check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            // Try to find the video in the cache by its ID
            if let Some((_, cached_path)) = download_cache.get_by_hash(&video.id) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Caching downloaded video with ID: {}", video.id);

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let best_video = video
            .best_video_format()
            .ok_or(Error::MissingFormat("video".to_string()))?;

        let best_audio = video
            .best_audio_format()
            .ok_or(Error::MissingFormat("audio".to_string()))?;

        // Create temporary names for audio and video files
        let audio_name = format!("temp_audio_{}.m4a", video.id);
        let video_name = format!("temp_video_{}.mp4", video.id);

        // Download audio and video streams in parallel
        let (audio_result, video_result) = tokio::join!(
            self.download_format(best_audio, &audio_name),
            self.download_format(best_video, &video_name)
        );

        // Check the results
        let _audio_path = audio_result?;
        let _video_path = video_result?;

        // Combine audio and video streams
        let output_path = self
            .combine_audio_and_video(&audio_name, &video_name, output.as_ref())
            .await?;

        // Clean up temporary files
        if let Err(_e) = tokio::fs::remove_file(&_video_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary video file: {}", _e);
        }
        if let Err(_e) = tokio::fs::remove_file(&_audio_path).await {
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to remove temporary audio file: {}", _e);
        }

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching downloaded video with ID: {}", video.id);

            if let Err(_e) = download_cache
                .put_file(&path, output_str, Some(video.id.clone()), None)
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache downloaded video: {}", _e);
            }
        }

        Ok(output_path)
    }

    /// Fetch the video from the given URL, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video_path = fetcher.download_video_stream_from_url(url, "my-video-stream.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;

        self.download_video_stream(&video, output).await
    }

    /// Download the video only, and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download.
    /// * `output` - The name of the file to save the video to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let video_path = fetcher.download_video_stream(&video, "my-video-stream.mp4").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading video stream {}", video.title);

        let best_video = video
            .best_video_format()
            .ok_or(Error::MissingFormat("video".to_string()))?;

        self.download_format(best_video, output).await
    }

    /// Fetch the audio stream from the given URL, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the video to download.
    /// * `output` - The name of the file to save the audio to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be fetched or downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let audio_path = fetcher.download_audio_stream_from_url(url, "my-audio.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream_from_url(
        &self,
        url: String,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream from URL: {}", url);

        let video = self.fetch_video_infos(url).await?;
        self.download_audio_stream(&video, output).await
    }

    /// Fetch the audio stream, download it and returns its path.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to download the audio from.
    /// * `output` - The name of the file to save the audio to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the audio could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let audio_path = fetcher.download_audio_stream(&video, "my-audio.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream(
        &self,
        video: &Video,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading audio stream {}", video.title);

        let output_str = output.as_ref();

        // Check if we have a cached audio file for this video
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            let path = self.output_dir.join(output_str);

            // Try to find an audio format in the cache by video ID
            let best_audio = video
                .best_audio_format()
                .ok_or(Error::MissingFormat("audio".to_string()))?;

            if let Some((_, cached_path)) =
                download_cache.get_by_video_and_format(&video.id, &best_audio.format_id)
            {
                #[cfg(feature = "tracing")]
                tracing::debug!(
                    "Using cached audio: {} (format: {})",
                    video.id,
                    best_audio.format_id
                );

                // Copy the file from the cache to the output directory
                tokio::fs::copy(&cached_path, &path).await?;
                return Ok(path);
            }
        }

        let best_audio = video
            .best_audio_format()
            .ok_or(Error::MissingFormat("audio".to_string()))?;

        let temp_output = format!("temp_{}", output_str);
        let temp_path = self.download_format(best_audio, &temp_output).await?;

        // Post-process the audio file with ffmpeg to ensure compatibility with players
        let output_path = self.output_dir.join(output_str);

        let temp = temp_path
            .to_str()
            .ok_or(Error::Path("Invalid temp path".to_string()))?;
        let output_str_path = output_path
            .to_str()
            .ok_or(Error::Path("Invalid output path".to_string()))?;

        let args = vec!["-i", temp, "-c:a", "aac", "-b:a", "192k", output_str_path];

        let executor = Executor {
            executable_path: self.libraries.ffmpeg.clone(),
            timeout: self.timeout,
            args: utils::to_owned(args),
        };

        executor.execute().await?;

        // Clean up temporary file (logs error internally if tracing is enabled)
        let _ = utils::file_system::remove_temp_file(temp_path).await;

        // Cache the processed audio file
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            #[cfg(feature = "tracing")]
            tracing::debug!("Caching format with ID: {}", best_audio.format_id);

            if let Err(_e) = download_cache
                .put_file(
                    &output_path,
                    output_str,
                    Some(video.id.clone()),
                    Some(best_audio),
                )
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(output_path)
    }

    /// Downloads a format.
    /// Be careful, this function may take a while to execute.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download.
    /// * `output` - The name of the file to save the format to.
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// # use yt_dlp::Youtube;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::fetcher::deps::Libraries;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries_dir = PathBuf::from("libs");
    /// # let output_dir = PathBuf::from("output");
    /// # let youtube = libraries_dir.join("yt-dlp");
    /// # let ffmpeg = libraries_dir.join("ffmpeg");
    /// # let libraries = Libraries::new(youtube, ffmpeg);
    /// let fetcher = Youtube::new(libraries, output_dir)?;
    ///
    /// let url = String::from("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
    /// let video = fetcher.fetch_video_infos(url).await?;
    ///
    /// let video_format = video.best_video_format().unwrap();
    /// let format_path = fetcher.download_format(&video_format, "my-video-stream.mp4").await?;
    ///
    /// let audio_format = video.worst_audio_format().unwrap();
    /// let audio_path = fetcher.download_format(&audio_format, "my-audio-stream.mp3").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_format(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
    ) -> crate::error::Result<PathBuf> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Downloading format {}", format.format_id);

        let output_path = self.output_dir.join(output.as_ref());

        // Use the internal function to download the format without preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(format, &output_path, None, None, None, None).await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Downloads a format with specific quality and codec preferences.
    ///
    /// This method allows fine-grained control over the download process by specifying
    /// quality and codec preferences for both video and audio components of the format.
    ///
    /// # Arguments
    ///
    /// * `format` - The format to download
    /// * `output` - The name of the output file
    /// * `video_quality` - Optional video quality preference
    /// * `audio_quality` - Optional audio quality preference
    /// * `video_codec` - Optional video codec preference
    /// * `audio_codec` - Optional audio codec preference
    ///
    /// # Returns
    ///
    /// * `PathBuf` - The path to the downloaded format
    ///
    /// # Errors
    ///
    /// This function will return an error if the video could not be downloaded.
    pub async fn download_format_with_preferences(
        &self,
        format: &Format,
        output: impl AsRef<str> + std::fmt::Debug + Display,
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());

        // Use the internal function to download the format with preferences
        cfg_if::cfg_if! {
            if #[cfg(feature = "cache")] {
                self.download_format_internal(
                    format,
                    &output_path,
                    video_quality,
                    audio_quality,
                    video_codec,
                    audio_codec,
                )
                .await
            } else {
                self.download_format_internal(format, &output_path).await
            }
        }
    }

    /// Internal function that handles downloading a format with or without preferences
    ///
    /// This function avoids code duplication between download_format and download_format_with_preferences
    async fn download_format_internal(
        &self,
        format: &Format,
        path: &PathBuf,
        #[cfg(feature = "cache")] video_quality: Option<VideoQuality>,
        #[cfg(feature = "cache")] audio_quality: Option<AudioQuality>,
        #[cfg(feature = "cache")] video_codec: Option<VideoCodecPreference>,
        #[cfg(feature = "cache")] audio_codec: Option<AudioCodecPreference>,
    ) -> crate::error::Result<PathBuf> {
        // Check if we have specific preferences
        #[cfg(feature = "cache")]
        let has_preferences = video_quality.is_some()
            || audio_quality.is_some()
            || video_codec.is_some()
            || audio_codec.is_some();

        // Check if the format is in the cache
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            if let Some(video_id) = format.video_id.as_ref() {
                // First try to find by exact format ID
                if let Some((_, cached_path)) =
                    download_cache.get_by_video_and_format(video_id, &format.format_id)
                {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Using cached format by ID: {}", format.format_id);

                    // Copy the file from the cache to the output directory
                    tokio::fs::copy(&cached_path, path).await?;
                    return Ok(path.clone());
                }

                // Then try to find by preferences if they exist
                if has_preferences {
                    if let Some((_, cached_path)) = download_cache.get_by_video_and_preferences(
                        video_id,
                        video_quality,
                        audio_quality,
                        video_codec.clone(),
                        audio_codec.clone(),
                    ) {
                        #[cfg(feature = "tracing")]
                        tracing::debug!("Using cached format by preferences");

                        // Copy the file from the cache to the output directory
                        tokio::fs::copy(&cached_path, path).await?;
                        return Ok(path.clone());
                    }
                }
            }
        }

        // Check if URL is available
        let url = format
            .download_info
            .url
            .clone()
            .ok_or(Error::MissingUrl(format.format_id.clone()))?;

        // Create an optimized fetcher with parallel downloading
        let fetcher = Fetcher::new(&url)
            .with_parallel_segments(8) // Use 8 parallel segments
            .with_segment_size(1024 * 1024 * 5) // 5 MB per segment
            .with_retry_attempts(3); // 3 attempts in case of failure

        fetcher.fetch_asset(path.clone()).await?;

        // Don't add metadata for video or audio streams that will be combined later
        // Only add metadata for standalone formats that contain both
        // audio and video, or for audio-only formats intended for direct use
        self.add_metadata_if_needed(path, format).await?;

        // Cache the downloaded file if caching is enabled
        #[cfg(feature = "cache")]
        if let Some(download_cache) = &self.download_cache {
            let output_str = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_string();

            #[cfg(feature = "tracing")]
            tracing::debug!("Caching format with ID: {}", format.format_id);

            // Use the appropriate function depending on whether we have preferences or not
            if has_preferences {
                if let Some(video_id) = format.video_id.as_ref() {
                    if let Err(_e) = download_cache
                        .put_file_with_preferences(
                            path,
                            output_str,
                            Some(video_id.clone()),
                            Some(format),
                            video_quality,
                            audio_quality,
                            video_codec,
                            audio_codec,
                        )
                        .await
                    {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to cache format with preferences: {}", _e);
                    }
                }
            } else if let Err(_e) = download_cache
                .put_file(path, output_str, format.video_id.clone(), Some(format))
                .await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to cache format: {}", _e);
            }
        }

        Ok(path.clone())
    }

    /// Adds format metadata based on the format type (audio-only, video-only, or both)
    /// This function is extracted to avoid code duplication
    async fn add_metadata_if_needed(
        &self,
        path: impl AsRef<Path>,
        format: &Format,
    ) -> crate::error::Result<()> {
        let format_type = format.format_type();
        let is_standalone_format = format_type.is_audio_and_video() || format_type.is_audio();

        if is_standalone_format {
            if let Some(video_id) = format.video_id.as_ref() {
                // Get the video metadata from the cache
                if let Some(video) = self.get_video_by_id(video_id).await {
                    #[cfg(feature = "tracing")]
                    tracing::debug!("Adding metadata to standalone file with format preferences");

                    // Use the method with format information for richer metadata
                    // Add metadata, log error on failure, then propagate
                    crate::metadata::MetadataManager::add_metadata_with_format(
                        path,
                        &video,
                        Some(format),
                        None,
                    )
                    .await
                    .inspect_err(|_e| {
                        #[cfg(feature = "tracing")]
                        tracing::warn!("Failed to add metadata to file: {}", _e);
                    })?;
                } else {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("Failed to get video metadata for ID: {}", video_id);
                }
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::debug!(
                "Skipping metadata for non-standalone format: will be added after combining"
            );
        }

        Ok(())
    }

    /// Retrieve a video by its ID, checking the cache first if available
    ///
    /// # Arguments
    ///
    /// * `video_id` - The ID of the video to find
    ///
    /// # Returns
    ///
    /// * `Option<Video>` - The video if found, None otherwise
    pub async fn get_video_by_id(&self, video_id: &str) -> Option<Video> {
        // First check if the video is in the cache
        #[cfg(feature = "cache")]
        if let Some(cache) = &self.cache {
            if let Ok(cached_video) = cache.get_by_id(video_id) {
                #[cfg(feature = "tracing")]
                tracing::debug!("Using cached video data for ID: {}", video_id);
                return Some(cached_video.video);
            }
        }

        // If not in cache, try to fetch it using the ID-based URL
        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Video not found in cache, trying to fetch it using ID: {}",
            video_id
        );

        let url = format!("https://www.youtube.com/watch?v={}", video_id);

        self.fetch_video_infos(url).await.ok().or({
            #[cfg(feature = "tracing")]
            tracing::warn!("Failed to fetch video by ID: {}", video_id);
            None
        })
    }
}
