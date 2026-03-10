use std::path::Path;

use crate::error::Result;
use crate::executor::Executor;
use crate::extractor::ExtractorName;

/// Detect which extractor type should handle a URL.
///
/// # Arguments
///
/// * `url` - The URL to analyze
/// * `executable_path` - Path to the yt-dlp executable
///
/// # Returns
///
/// ExtractorName indicating which extractor should be used
///
/// # Errors
///
/// Returns error if URL cannot be validated or no extractor is available
pub async fn detect_extractor_type(url: &str, executable_path: &Path) -> Result<ExtractorName> {
    tracing::debug!(
        url = %url,
        executable = ?executable_path,
        "📡 Detecting extractor type for URL"
    );

    // Fast path: Pattern matching for YouTube
    if is_youtube_url(url) {
        tracing::debug!(
            url = %url,
            extractor = "youtube",
            "✅ Detected YouTube URL via pattern matching"
        );

        return Ok(ExtractorName::Youtube);
    }

    tracing::debug!(
        url = %url,
        "📡 URL is not YouTube, querying yt-dlp for extractor detection"
    );

    // Slow path: Query yt-dlp to detect extractor
    let extractor_name = detect_via_ytdlp(url, executable_path).await?;

    tracing::debug!(
        url = %url,
        extractor = %extractor_name,
        "✅ Detected extractor via yt-dlp"
    );

    Ok(ExtractorName::Generic(Some(extractor_name)))
}

use crate::extractor::youtube::Youtube;

/// Fast check if URL matches YouTube patterns.
///
/// # Arguments
///
/// * `url` - The URL to check
///
/// # Returns
///
/// true if the URL matches YouTube patterns, false otherwise
fn is_youtube_url(url: &str) -> bool {
    Youtube::supports_url(url)
}

/// Detect extractor via yt-dlp simulation.
///
/// # Arguments
///
/// * `url` - The URL to detect extractor for
/// * `executable_path` - Path to the yt-dlp executable
///
/// # Returns
///
/// The name of the detected extractor
///
/// # Errors
///
/// Returns an error if yt-dlp fails, JSON parsing fails, or extractor field is missing
// LCOV_EXCL_START — requires real yt-dlp binary on PATH
async fn detect_via_ytdlp(url: &str, executable_path: &Path) -> Result<String> {
    tracing::debug!(
        url = %url,
        executable = ?executable_path,
        "📡 Starting yt-dlp extractor detection"
    );

    let args = vec![
        "--dump-single-json".to_string(),
        "--simulate".to_string(),
        "--no-warnings".to_string(),
        url.to_string(),
    ];

    let executor = Executor::new(executable_path.to_path_buf(), args, crate::client::DEFAULT_TIMEOUT);

    tracing::debug!(
        url = %url,
        "📡 Executing yt-dlp with --simulate to detect extractor"
    );

    let output = executor.execute().await?;

    tracing::debug!(
        url = %url,
        stdout_len = output.stdout.len(),
        "⚙️ yt-dlp execution completed, parsing JSON"
    );

    let json: serde_json::Value = serde_json::from_str(&output.stdout)?;

    let extractor = json["extractor"].as_str().ok_or_else(|| {
        tracing::error!(
            url = %url,
            "Missing extractor field in yt-dlp output"
        );

        crate::error::Error::VideoMissingField {
            video_id: url.to_string(),
            field: "extractor".to_string(),
        }
    })?;

    tracing::debug!(
        url = %url,
        extractor = extractor,
        "✅ Successfully detected extractor from yt-dlp"
    );

    Ok(extractor.to_string())
}
// LCOV_EXCL_STOP
