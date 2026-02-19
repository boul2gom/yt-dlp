use crate::Downloader;
use crate::model::Video;
use crate::model::format::Format;
use crate::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality, matches_audio_codec,
    matches_video_codec,
};
use ordered_float::OrderedFloat;
use std::cmp::Ordering;

/// Trait for selecting video and audio formats from a Video.
pub trait VideoSelection {
    fn best_video_format(&self) -> Option<&Format>;
    fn best_audio_format(&self) -> Option<&Format>;
    fn worst_video_format(&self) -> Option<&Format>;
    fn worst_audio_format(&self) -> Option<&Format>;
    fn compare_video_formats(&self, a: &Format, b: &Format) -> Ordering;
    fn compare_audio_formats(&self, a: &Format, b: &Format) -> Ordering;
    fn select_video_format(
        &self,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Option<&Format>;
    fn select_audio_format(
        &self,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Option<&Format>;
}

impl VideoSelection for Video {
    /// Returns the best video format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    fn best_video_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            format_count = self.formats.len(),
            "Selecting best video format"
        );

        self.formats
            .iter()
            .filter(|f| f.is_video())
            .max_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the best audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    fn best_audio_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            format_count = self.formats.len(),
            "Selecting best audio format"
        );

        self.formats
            .iter()
            .filter(|f| f.is_audio())
            .max_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Returns the worst video format available.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    fn worst_video_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            format_count = self.formats.len(),
            "Selecting worst video format"
        );

        self.formats
            .iter()
            .filter(|f| f.is_video())
            .min_by(|a, b| self.compare_video_formats(a, b))
    }

    /// Returns the worst audio format available.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    fn worst_audio_format(&self) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            format_count = self.formats.len(),
            "Selecting worst audio format"
        );

        self.formats
            .iter()
            .filter(|f| f.is_audio())
            .min_by(|a, b| self.compare_audio_formats(a, b))
    }

    /// Compares two video formats.
    /// Formats sorting : "quality", "video resolution", "fps", "video bitrate"
    fn compare_video_formats(&self, a: &Format, b: &Format) -> Ordering {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Comparing video formats: {} and {}",
            a.format_id,
            b.format_id
        );

        let a_quality = a.quality_info.quality.unwrap_or(OrderedFloat(0.0));
        let b_quality = b.quality_info.quality.unwrap_or(OrderedFloat(0.0));

        let cmp_quality = a_quality.cmp(&b_quality);
        if cmp_quality != Ordering::Equal {
            return cmp_quality;
        }

        let a_height = a.video_resolution.height.unwrap_or(0);
        let b_height = b.video_resolution.height.unwrap_or(0);

        let cmp_height = a_height.cmp(&b_height);
        if cmp_height != Ordering::Equal {
            return cmp_height;
        }

        let a_fps = a.video_resolution.fps.map(|f| *f).unwrap_or(0.0);
        let b_fps = b.video_resolution.fps.map(|f| *f).unwrap_or(0.0);

        let cmp_fps = OrderedFloat(a_fps).cmp(&OrderedFloat(b_fps));
        if cmp_fps != Ordering::Equal {
            return cmp_fps;
        }

        let a_vbr = a.rates_info.video_rate.map(|vr| *vr).unwrap_or(0.0);
        let b_vbr = b.rates_info.video_rate.map(|vr| *vr).unwrap_or(0.0);

        OrderedFloat(a_vbr).cmp(&OrderedFloat(b_vbr))
    }

    /// Compares two audio formats.
    /// Formats sorting : "quality", "audio bitrate", "sample rate", "audio channels"
    fn compare_audio_formats(&self, a: &Format, b: &Format) -> Ordering {
        #[cfg(feature = "tracing")]
        tracing::trace!(
            "Comparing audio formats: {} and {}",
            a.format_id,
            b.format_id
        );

        let a_quality = a.quality_info.quality.unwrap_or(OrderedFloat(0.0));
        let b_quality = b.quality_info.quality.unwrap_or(OrderedFloat(0.0));

        let cmp_quality = a_quality.cmp(&b_quality);
        if cmp_quality != Ordering::Equal {
            return cmp_quality;
        }

        let a_abr = a.rates_info.audio_rate.map(|ar| *ar).unwrap_or(0.0);
        let b_abr = b.rates_info.audio_rate.map(|ar| *ar).unwrap_or(0.0);

        let cmp_abr = OrderedFloat(a_abr).cmp(&OrderedFloat(b_abr));
        if cmp_abr != Ordering::Equal {
            return cmp_abr;
        }

        let a_asr = a.codec_info.asr.unwrap_or(0);
        let b_asr = b.codec_info.asr.unwrap_or(0);

        let cmp_asr = a_asr.cmp(&b_asr);
        if cmp_asr != Ordering::Equal {
            return cmp_asr;
        }

        let a_channels = a.codec_info.audio_channels.unwrap_or(0);
        let b_channels = b.codec_info.audio_channels.unwrap_or(0);

        a_channels.cmp(&b_channels)
    }

    /// Selects a video format based on quality preference and codec preference.
    fn select_video_format(
        &self,
        quality: VideoQuality,
        codec: VideoCodecPreference,
    ) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            quality = ?quality,
            codec = ?codec,
            total_formats = self.formats.len(),
            "Selecting video format with preferences"
        );

        let video_formats: Vec<&Format> = self
            .formats
            .iter()
            .filter(|format| format.is_video())
            .collect();

        if video_formats.is_empty() {
            return None;
        }

        // Filter by codec if a specific one is requested
        let codec_filtered: Vec<&Format> = match codec {
            VideoCodecPreference::Any => video_formats,
            _ => {
                let filtered: Vec<&Format> = video_formats
                    .iter()
                    .filter(|format| {
                        if let Some(video_codec) = &format.codec_info.video_codec {
                            matches_video_codec(video_codec, &codec)
                        } else {
                            false
                        }
                    })
                    .copied()
                    .collect();

                if filtered.is_empty() {
                    video_formats
                } else {
                    filtered
                }
            }
        };

        // Select based on quality preference
        match quality {
            VideoQuality::Best => codec_filtered
                .into_iter()
                .max_by(|a, b| self.compare_video_formats(a, b)),

            VideoQuality::Worst => codec_filtered
                .into_iter()
                .min_by(|a, b| self.compare_video_formats(a, b)),

            VideoQuality::High => select_closest_video_height(codec_filtered, 1080, self),

            VideoQuality::Medium => select_closest_video_height(codec_filtered, 720, self),

            VideoQuality::Low => select_closest_video_height(codec_filtered, 480, self),

            VideoQuality::CustomHeight(height) => {
                select_closest_video_height(codec_filtered, height, self)
            }

            VideoQuality::CustomWidth(width) => {
                select_closest_video_width(codec_filtered, width, self)
            }
        }
    }

    /// Selects an audio format based on quality preference and codec preference.
    fn select_audio_format(
        &self,
        quality: AudioQuality,
        codec: AudioCodecPreference,
    ) -> Option<&Format> {
        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %self.id,
            quality = ?quality,
            codec = ?codec,
            total_formats = self.formats.len(),
            "Selecting audio format with preferences"
        );

        let audio_formats: Vec<&Format> = self
            .formats
            .iter()
            .filter(|format| format.is_audio())
            .collect();

        if audio_formats.is_empty() {
            return None;
        }

        // Filter by codec if a specific one is requested
        let codec_filtered: Vec<&Format> = match codec {
            AudioCodecPreference::Any => audio_formats,
            _ => {
                let filtered: Vec<&Format> = audio_formats
                    .iter()
                    .filter(|format| {
                        if let Some(audio_codec) = &format.codec_info.audio_codec {
                            matches_audio_codec(audio_codec, &codec)
                        } else {
                            false
                        }
                    })
                    .copied()
                    .collect();

                if filtered.is_empty() {
                    audio_formats
                } else {
                    filtered
                }
            }
        };

        // Select based on quality preference
        match quality {
            AudioQuality::Best => codec_filtered
                .into_iter()
                .max_by(|a, b| self.compare_audio_formats(a, b)),

            AudioQuality::Worst => codec_filtered
                .into_iter()
                .min_by(|a, b| self.compare_audio_formats(a, b)),

            AudioQuality::High => select_closest_audio_bitrate(codec_filtered, 192, self),

            AudioQuality::Medium => select_closest_audio_bitrate(codec_filtered, 128, self),

            AudioQuality::Low => select_closest_audio_bitrate(codec_filtered, 96, self),

            AudioQuality::CustomBitrate(bitrate) => {
                select_closest_audio_bitrate(codec_filtered, bitrate, self)
            }
        }
    }
}

/// Selects the video format with the closest height to the target
///
/// # Arguments
///
/// * `formats` - List of video formats to choose from
/// * `target_height` - Target height in pixels
/// * `video` - The video being processed (for quality comparisons)
///
/// # Returns
///
/// The format with the closest height to the target, or None if no formats available
fn select_closest_video_height<'a>(
    formats: Vec<&'a Format>,
    target_height: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        target_height = target_height,
        available_formats = formats.len(),
        video_id = %video.id,
        "Selecting video format closest to target height"
    );

    if formats.is_empty() {
        return None;
    }

    // First try to find formats with height >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .video_resolution
                .height
                .is_some_and(|h| h >= target_height)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest height to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_diff = a
                .video_resolution
                .height
                .unwrap_or(0)
                .saturating_sub(target_height);
            let b_diff = b
                .video_resolution
                .height
                .unwrap_or(0)
                .saturating_sub(target_height);

            // Compare difference then quality
            a_diff
                .cmp(&b_diff)
                .then_with(|| video.compare_video_formats(a, b))
        });
    }

    // If no format with height >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_height = a.video_resolution.height.unwrap_or(0);
        let b_height = b.video_resolution.height.unwrap_or(0);

        // Compare height then quality
        a_height
            .cmp(&b_height)
            .then_with(|| video.compare_video_formats(a, b))
    })
}

/// Selects the video format with the closest width to the target
///
/// # Arguments
///
/// * `formats` - List of video formats to choose from
/// * `target_width` - Target width in pixels
/// * `video` - The video being processed (for quality comparisons)
///
/// # Returns
///
/// The format with the closest width to the target, or None if no formats available
fn select_closest_video_width<'a>(
    formats: Vec<&'a Format>,
    target_width: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        target_width = target_width,
        available_formats = formats.len(),
        video_id = %video.id,
        "Selecting video format closest to target width"
    );

    if formats.is_empty() {
        return None;
    }

    // First try to find formats with width >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .video_resolution
                .width
                .is_some_and(|w| w >= target_width)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest width to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_diff = a
                .video_resolution
                .width
                .unwrap_or(0)
                .saturating_sub(target_width);
            let b_diff = b
                .video_resolution
                .width
                .unwrap_or(0)
                .saturating_sub(target_width);

            // Compare difference then quality
            a_diff
                .cmp(&b_diff)
                .then_with(|| video.compare_video_formats(a, b))
        });
    }

    // If no format with width >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_width = a.video_resolution.width.unwrap_or(0);
        let b_width = b.video_resolution.width.unwrap_or(0);

        // Compare width then quality
        a_width
            .cmp(&b_width)
            .then_with(|| video.compare_video_formats(a, b))
    })
}

/// Selects the audio format with the closest bitrate to the target
///
/// # Arguments
///
/// * `formats` - List of audio formats to choose from
/// * `target_bitrate` - Target bitrate in kbps
/// * `video` - The video being processed (for quality comparisons)
///
/// # Returns
///
/// The format with the closest bitrate to the target, or None if no formats available
fn select_closest_audio_bitrate<'a>(
    formats: Vec<&'a Format>,
    target_bitrate: u32,
    video: &Video,
) -> Option<&'a Format> {
    #[cfg(feature = "tracing")]
    tracing::debug!(
        target_bitrate = target_bitrate,
        available_formats = formats.len(),
        video_id = %video.id,
        "Selecting audio format closest to target bitrate"
    );

    if formats.is_empty() {
        return None;
    }

    let target_float = OrderedFloat(target_bitrate as f64);

    // First try to find formats with bitrate >= target
    let formats_above_target: Vec<&Format> = formats
        .iter()
        .filter(|format| {
            format
                .rates_info
                .audio_rate
                .is_some_and(|r| r >= target_float)
        })
        .copied()
        .collect();

    if !formats_above_target.is_empty() {
        // Find the one with the closest bitrate to target
        return formats_above_target.into_iter().min_by(|a, b| {
            let a_rate = a.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));
            let b_rate = b.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));

            let a_diff = (a_rate.0 - target_bitrate as f64).abs();
            let b_diff = (b_rate.0 - target_bitrate as f64).abs();

            // Compare bitrate difference then quality
            OrderedFloat(a_diff)
                .partial_cmp(&OrderedFloat(b_diff))
                .unwrap_or(Ordering::Equal)
                .then_with(|| video.compare_audio_formats(a, b))
        });
    }

    // If no format with bitrate >= target, get the highest available
    formats.into_iter().max_by(|a, b| {
        let a_rate = a.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));
        let b_rate = b.rates_info.audio_rate.unwrap_or(OrderedFloat(0.0));

        // Compare bitrate then quality
        a_rate
            .partial_cmp(&b_rate)
            .unwrap_or(Ordering::Equal)
            .then_with(|| video.compare_audio_formats(a, b))
    })
}

impl Downloader {
    /// Lists all available subtitle languages for a video.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to list subtitle languages for
    ///
    /// # Returns
    ///
    /// A vector of language codes
    pub fn list_subtitle_languages(&self, video: &Video) -> Vec<String> {
        let languages: Vec<String> = video.subtitles.keys().cloned().collect();

        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %video.id,
            language_count = languages.len(),
            languages = ?languages,
            "Listing subtitle languages"
        );

        languages
    }

    /// Checks if a video has subtitles in a specific language.
    ///
    /// # Arguments
    ///
    /// * `video` - The video to check
    /// * `language_code` - The language code to check for
    ///
    /// # Returns
    ///
    /// `true` if subtitles are available in the specified language
    pub fn has_subtitle_language(&self, video: &Video, language_code: &str) -> bool {
        let has_language = video.subtitles.contains_key(language_code);

        #[cfg(feature = "tracing")]
        tracing::debug!(
            video_id = %video.id,
            language_code = language_code,
            has_language = has_language,
            "Checking for subtitle language"
        );

        has_language
    }
}
