use crate::error::Result;
use crate::executor::Executor;
use std::path::Path;
use std::time::Duration;

/// Type of extractor to use for a given URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractorType {
    /// YouTube extractor (highly optimized)
    Youtube,

    /// Generic extractor for all other sites
    Generic(String),
}

/// Detect which extractor type should handle a URL.
///
/// # Arguments
/// * `url` - The URL to analyze
/// * `executable_path` - Path to the yt-dlp executable
///
/// # Returns
/// ExtractorType indicating which extractor should be used
///
/// # Errors
/// Returns error if URL cannot be validated or no extractor is available
pub async fn detect_extractor_type(url: &str, executable_path: &Path) -> Result<ExtractorType> {
    // Fast path: Pattern matching for YouTube
    if is_youtube_url(url) {
        return Ok(ExtractorType::Youtube);
    }

    // Slow path: Query yt-dlp to detect extractor
    let extractor_name = detect_via_ytdlp(url, executable_path).await?;
    Ok(ExtractorType::Generic(extractor_name))
}

/// Fast check if URL matches YouTube patterns.
fn is_youtube_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    url_lower.contains("youtube.com")
        || url_lower.contains("youtu.be")
        || url_lower.contains("youtube-nocookie.com")
        || url_lower.starts_with("ytsearch")
        || url_lower.starts_with("ytplaylist")
}

/// Detect extractor via yt-dlp simulation.
async fn detect_via_ytdlp(url: &str, executable_path: &Path) -> Result<String> {
    let args = vec![
        "--dump-json".to_string(),
        "--simulate".to_string(),
        "--no-warnings".to_string(),
        url.to_string(),
    ];

    let executor = Executor {
        executable_path: executable_path.to_path_buf(),
        args,
        timeout: Duration::from_secs(10),
    };

    let output = executor.execute().await?;

    let json: serde_json::Value = serde_json::from_str(&output.stdout)?;

    let extractor = json["extractor"].as_str().ok_or_else(|| {
        crate::error::Error::Unknown("Missing extractor field in yt-dlp output".to_string())
    })?;

    Ok(extractor.to_string())
}
