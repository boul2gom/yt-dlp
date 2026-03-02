use std::path::{Path, PathBuf};

use crate::Downloader;
use crate::download::Fetcher;
use crate::error::Error;
use crate::model::Video;
use crate::model::caption::Extension as CaptionExtension;

impl Downloader {
    /// Downloads a subtitle file for a specific language.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `language_code` - The language code (e.g., "en", "es").
    /// * `output` - The output filename/path relative to the download directory.
    /// * `fallback_to_automatic` - Whether to fall back to automatic captions.
    ///
    /// # Returns
    ///
    /// The path to the downloaded subtitle file.
    pub async fn download_subtitle(
        &self,
        video: &Video,
        language_code: impl AsRef<str>,
        output: impl AsRef<str>,
        fallback_to_automatic: bool,
    ) -> crate::error::Result<PathBuf> {
        let language_code = language_code.as_ref();

        tracing::debug!(video_id = video.id, language = language_code, "💬 Downloading subtitle");

        let output_path = self.output_dir.join(output.as_ref());

        // Check if subtitle is in the cache
        #[cfg(cache)]
        if let Some(cache) = &self.cache
            && let Ok(Some((_, cached_path))) = cache.downloads.get_subtitle_by_language(&video.id, language_code).await
        {
            tracing::debug!(
                video_id = video.id,
                language = language_code,
                "🔍 Using cached subtitle"
            );

            // Hard link if possible, fall back to copy for cross-filesystem
            if tokio::fs::hard_link(&cached_path, &output_path).await.is_err() {
                tokio::fs::copy(&cached_path, &output_path).await?;
            }
            return Ok(output_path);
        }

        // Resolve subtitles for the language: prefer user-uploaded subtitles,
        // then fall back to automatic captions (e.g. YouTube auto-generated).
        let owned_fallback: Vec<crate::model::caption::Subtitle>;
        let subtitles: &[crate::model::caption::Subtitle] = if let Some(subs) = video.subtitles.get(language_code) {
            subs.as_slice()
        } else if fallback_to_automatic {
            if let Some(captions) = video.automatic_captions.get(language_code) {
                owned_fallback = captions
                    .iter()
                    .map(|c| crate::model::caption::Subtitle::from_automatic_caption(c, language_code.to_string()))
                    .collect();
                owned_fallback.as_slice()
            } else {
                return Err(Error::SubtitleNotAvailable {
                    video_id: video.id.clone(),
                    language: language_code.to_string(),
                });
            }
        } else {
            return Err(Error::SubtitleNotAvailable {
                video_id: video.id.clone(),
                language: language_code.to_string(),
            });
        };

        // Prefer SRT format, then VTT, then any available format
        let subtitle = subtitles
            .iter()
            .find(|s| s.is_format(&CaptionExtension::Srt))
            .or_else(|| subtitles.iter().find(|s| s.is_format(&CaptionExtension::Vtt)))
            .or_else(|| subtitles.first())
            .ok_or_else(|| Error::SubtitleNotAvailable {
                video_id: video.id.clone(),
                language: language_code.to_string(),
            })?;

        tracing::debug!(url = subtitle.url, path = ?output_path, "💬 Downloading subtitle file");

        // Download the subtitle file
        let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
        fetcher.fetch_asset(&output_path).await?;

        // Cache the downloaded subtitle
        #[cfg(cache)]
        if let Some(cache) = &self.cache {
            tracing::debug!(video_id = video.id, language = language_code, "🔍 Caching subtitle");

            if let Err(_e) = cache
                .downloads
                .put_subtitle_file(
                    &output_path,
                    output.as_ref(),
                    video.id.clone(),
                    language_code.to_string(),
                )
                .await
            {
                tracing::warn!(error = %_e, "Failed to cache subtitle");
            }
        }

        tracing::info!(language = language_code, path = ?output_path, "✅ Subtitle downloaded");

        Ok(output_path)
    }

    /// Downloads all available subtitles and automatic captions for a video.
    ///
    /// Iterates over user-uploaded subtitles first, then merges automatic captions
    /// for any language not already covered. Prefers SRT, then VTT, then any format.
    ///
    /// # Arguments
    ///
    /// * `video` - The `Video` metadata struct.
    /// * `output_dir` - The directory to save the subtitle files to.
    /// * `fallback_to_automatic` - Whether to include automatic captions.
    ///
    /// # Returns
    ///
    /// A vector of paths to the downloaded subtitle files.
    pub async fn download_all_subtitles(
        &self,
        video: &Video,
        output_dir: impl AsRef<Path>,
        fallback_to_automatic: bool,
    ) -> crate::error::Result<Vec<PathBuf>> {
        tracing::debug!(
            video_id = %video.id,
            subtitle_langs = video.subtitles.len(),
            caption_langs = video.automatic_captions.len(),
            "💬 Downloading all subtitles and automatic captions"
        );

        let output_dir = output_dir.as_ref();
        let mut downloaded_files = Vec::new();

        // Merge language sources: manual subtitles take priority over automatic captions
        let mut all_languages: std::collections::HashMap<&str, Vec<crate::model::caption::Subtitle>> =
            std::collections::HashMap::new();

        if fallback_to_automatic {
            for (lang, captions) in &video.automatic_captions {
                let subs: Vec<_> = captions
                    .iter()
                    .map(|c| crate::model::caption::Subtitle::from_automatic_caption(c, lang.clone()))
                    .collect();
                all_languages.entry(lang.as_str()).or_insert(subs);
            }
        }
        for (lang, subs) in &video.subtitles {
            // Manual subtitles override automatic captions for the same language
            all_languages.insert(lang.as_str(), subs.clone());
        }

        for (language_code, subtitles) in &all_languages {
            // Prefer SRT → VTT → first available
            let Some(subtitle) = subtitles
                .iter()
                .find(|s| s.is_format(&CaptionExtension::Srt))
                .or_else(|| subtitles.iter().find(|s| s.is_format(&CaptionExtension::Vtt)))
                .or_else(|| subtitles.first())
            else {
                continue;
            };

            let filename = format!("{}.{}.{}", video.id, language_code, subtitle.file_extension());
            let output_path = output_dir.join(&filename);

            tracing::debug!(
                video_id = %video.id,
                language_code = language_code,
                url = %subtitle.url,
                "💬 Downloading subtitle/caption"
            );

            let fetcher = Fetcher::new(&subtitle.url, self.proxy.as_ref(), None)?;
            fetcher.fetch_asset(&output_path).await?;
            downloaded_files.push(output_path);
        }

        tracing::info!(
            video_id = %video.id,
            count = downloaded_files.len(),
            "✅ Subtitle/caption files downloaded"
        );

        Ok(downloaded_files)
    }
}
