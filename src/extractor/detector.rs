use crate::error::Result;
use crate::executor::Executor;
use std::path::Path;

use crate::extractor::ExtractorName;

/// Detect which extractor type should handle a URL.
///
/// # Arguments
/// * `url` - The URL to analyze
/// * `executable_path` - Path to the yt-dlp executable
///
/// # Returns
/// ExtractorName indicating which extractor should be used
///
/// # Errors
/// Returns error if URL cannot be validated or no extractor is available
pub async fn detect_extractor_type(url: &str, executable_path: &Path) -> Result<ExtractorName> {
    // Fast path: Pattern matching for YouTube
    if is_youtube_url(url) {
        return Ok(ExtractorName::Youtube);
    }

    // Slow path: Query yt-dlp to detect extractor
    let extractor_name = detect_via_ytdlp(url, executable_path).await?;
    Ok(ExtractorName::Generic(Some(extractor_name)))
}

use crate::extractor::youtube::Youtube;

/// Fast check if URL matches YouTube patterns.
fn is_youtube_url(url: &str) -> bool {
    Youtube::supports_url(url)
}

/// Detect extractor via yt-dlp simulation.
async fn detect_via_ytdlp(url: &str, executable_path: &Path) -> Result<String> {
    let args = vec![
        "--dump-json".to_string(),
        "--simulate".to_string(),
        "--no-warnings".to_string(),
        url.to_string(),
    ];

    let executor = Executor::new(
        executable_path.to_path_buf(),
        args,
        crate::client::DEFAULT_TIMEOUT,
    );

    let output = executor.execute().await?;

    let json: serde_json::Value = serde_json::from_str(&output.stdout)?;

    let extractor = json["extractor"].as_str().ok_or_else(|| {
        crate::error::Error::Unknown("Missing extractor field in yt-dlp output".to_string())
    })?;

    Ok(extractor.to_string())
}
