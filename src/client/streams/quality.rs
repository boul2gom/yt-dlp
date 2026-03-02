use std::path::PathBuf;

use crate::Downloader;
use crate::client::streams::selection::VideoSelection;
use crate::error::{Error, Result};
use crate::model::Video;
use crate::model::format::FormatType;
use crate::model::selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};

impl Downloader {
    /// Downloads a video (video + audio combined) with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `video_quality` - The desired video quality.
    /// * `video_codec` - The preferred video codec.
    /// * `audio_quality` - The desired audio quality.
    /// * `audio_codec` - The preferred audio codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference, AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// let path = downloader.download_video_with_quality(
    ///     &video,
    ///     "my-video.mp4",
    ///     VideoQuality::High,
    ///     VideoCodecPreference::VP9,
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_with_quality_to_path(
            video,
            output_path,
            video_quality,
            video_codec,
            audio_quality,
            audio_codec,
        )
        .await
    }

    /// Downloads a video with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_with_quality`](Self::download_video_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `video_quality` - The desired video quality.
    /// * `video_codec` - The preferred video codec.
    /// * `audio_quality` - The desired audio quality.
    /// * `audio_codec` - The preferred audio codec.
    pub async fn download_video_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        self.download_video_with_quality_to_path_inner(
            video,
            output,
            video_quality,
            video_codec,
            audio_quality,
            audio_codec,
        )
        .await
    }

    /// Internal implementation for download_video_with_quality_to_path.
    /// Separated to allow retry on 403 wrapping without duplicating the complex body.
    async fn download_video_with_quality_to_path_inner(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        video_quality: VideoQuality,
        video_codec: VideoCodecPreference,
        audio_quality: AudioQuality,
        audio_codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        tracing::debug!(
            video_id = video.id,
            video_title = video.title,
            video_quality = ?video_quality,
            video_codec = ?video_codec,
            audio_quality = ?audio_quality,
            audio_codec = ?audio_codec,
            "🧩 Selecting formats based on quality preferences"
        );

        // Select video format based on quality and codec preferences
        let video_format = video
            .select_video_format(video_quality, video_codec.clone())
            .ok_or_else(|| Error::FormatNotAvailable {
                video_id: video.id.clone(),
                format_type: FormatType::Video,
                available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
            })?;

        tracing::debug!(
            video_id = video.id,
            format_id = video_format.format_id,
            width = ?video_format.video_resolution.width,
            height = ?video_format.video_resolution.height,
            codec = ?video_format.codec_info.video_codec,
            "🧩 Selected video format"
        );

        let output_path: PathBuf = output.into();

        // When no explicit codec preference, prefer a codec that is natively compatible
        // with the output container to avoid re-encoding during muxing.
        // For example, AAC audio can be stream-copied into MP4 without re-encoding,
        // while Opus requires an expensive software transcode to AAC (~50x real-time).
        let preferred_audio_codec = if audio_codec == AudioCodecPreference::Any {
            let ext = output_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            match ext.as_str() {
                "mp4" | "m4a" | "mov" => Some(AudioCodecPreference::AAC),
                "webm" => Some(AudioCodecPreference::Opus),
                _ => None,
            }
        } else {
            None
        };

        // Select audio format: try the container-compatible codec first, fall back to any
        let audio_format = if let Some(pref) = preferred_audio_codec {
            tracing::debug!(
                video_id = video.id,
                preferred_codec = ?pref,
                output_ext = ?output_path.extension(),
                "🧩 Trying container-compatible audio codec to avoid re-encoding"
            );
            video.select_audio_format(audio_quality, pref).or_else(|| {
                tracing::debug!(
                    video_id = video.id,
                    "🧩 Container-compatible audio not available, falling back to any codec"
                );
                video.select_audio_format(audio_quality, AudioCodecPreference::Any)
            })
        } else {
            video.select_audio_format(audio_quality, audio_codec)
        }
        .ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type: FormatType::Audio,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        tracing::debug!(
            video_id = video.id,
            format_id = audio_format.format_id,
            bitrate = ?audio_format.rates_info.audio_rate,
            codec = ?audio_format.codec_info.audio_codec,
            "🧩 Selected audio format"
        );

        // Download and combine formats, embedding metadata in a single ffmpeg pass
        self.download_and_combine_with_meta(video, video_format, audio_format, &output_path)
            .await
    }

    /// Downloads a video stream (video-only) with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `quality` - The desired video quality.
    /// * `codec` - The preferred video codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded video file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{VideoQuality, VideoCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// // Download a medium quality video with AVC1 codec
    /// let video_path = downloader.download_video_stream_with_quality(
    ///     &video,
    ///     "video-only.mp4",
    ///     VideoQuality::Medium,
    ///     VideoCodecPreference::AVC1
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video_stream_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_video_stream_with_quality_to_path(video, output_path, quality, codec)
            .await
    }

    /// Helper function to download a specific type of stream with quality preferences
    async fn download_stream_with_quality<F, Q, C>(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        quality: Q,
        codec: C,
        format_type: FormatType,
        select_format: F,
    ) -> Result<PathBuf>
    where
        F: FnOnce(&Video, Q, C) -> Option<crate::model::format::Format>,
        Q: std::fmt::Debug,
        C: std::fmt::Debug,
    {
        let output: PathBuf = output.into();

        tracing::debug!(
            video_id = video.id,
            output = ?output,
            quality = ?quality,
            codec = ?codec,
            stream_type = %format_type,
            "📥 Downloading stream with quality preferences"
        );

        let format = select_format(video, quality, codec).ok_or_else(|| Error::FormatNotAvailable {
            video_id: video.id.clone(),
            format_type,
            available_formats: video.formats.iter().map(|f| f.format_id.clone()).collect(),
        })?;

        self.download_format_to_path(&format, &output).await
    }

    /// Downloads a video stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_video_stream_with_quality`](Self::download_video_stream_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `quality` - The desired video quality.
    /// * `codec` - The preferred video codec.
    pub async fn download_video_stream_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Result<PathBuf> {
        self.download_stream_with_quality(video, output, quality, codec, FormatType::Video, |v, q, c| {
            v.select_video_format(q, c).cloned()
        })
        .await
    }

    /// Downloads an audio stream with the specified quality preferences.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The name of the output file.
    /// * `quality` - The desired audio quality.
    /// * `codec` - The preferred audio codec.
    ///
    /// # Returns
    ///
    /// The path to the downloaded audio file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use yt_dlp::Downloader;
    /// # use std::path::PathBuf;
    /// # use yt_dlp::client::deps::Libraries;
    /// # use yt_dlp::model::{AudioQuality, AudioCodecPreference};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let libraries = Libraries::new(PathBuf::from("libs/yt-dlp"), PathBuf::from("libs/ffmpeg"));
    /// # let downloader = Downloader::builder(libraries, "output").build().await?;
    /// let video = downloader.fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo").await?;
    ///
    /// let audio_path = downloader.download_audio_stream_with_quality(
    ///     &video,
    ///     "audio-only.mp3",
    ///     AudioQuality::High,
    ///     AudioCodecPreference::Opus
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_audio_stream_with_quality(
        &self,
        video: &Video,
        output: impl AsRef<str>,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        let output_path = self.output_dir.join(output.as_ref());
        self.download_audio_stream_with_quality_to_path(video, output_path, quality, codec)
            .await
    }

    /// Downloads an audio stream with quality preferences to a specific path.
    ///
    /// Unlike [`download_audio_stream_with_quality`](Self::download_audio_stream_with_quality),
    /// this method writes the file to the exact path specified, ignoring the configured `output_dir`.
    ///
    /// # Arguments
    ///
    /// * `video` - The pre-fetched `Video` metadata.
    /// * `output` - The full path where the file will be saved.
    /// * `quality` - The desired audio quality.
    /// * `codec` - The preferred audio codec.
    pub async fn download_audio_stream_with_quality_to_path(
        &self,
        video: &Video,
        output: impl Into<PathBuf>,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Result<PathBuf> {
        self.download_stream_with_quality(video, output, quality, codec, FormatType::Audio, |v, q, c| {
            v.select_audio_format(q, c).cloned()
        })
        .await
    }
}
