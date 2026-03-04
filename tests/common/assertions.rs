use std::path::Path;

use yt_dlp::model::Video;
use yt_dlp::model::format::FormatType;

/// Assert a video has the expected number of formats.
pub fn assert_format_count(video: &Video, expected: usize) {
    assert_eq!(
        video.formats.len(),
        expected,
        "Expected {} formats, got {}",
        expected,
        video.formats.len()
    );
}

/// Assert a video has at least one audio format.
pub fn assert_has_audio_format(video: &Video) {
    assert!(
        video.formats.iter().any(|f| f.format_type() == FormatType::Audio),
        "Expected at least one audio format"
    );
}

/// Assert a video has at least one video format.
pub fn assert_has_video_format(video: &Video) {
    assert!(
        video.formats.iter().any(|f| f.format_type() == FormatType::Video),
        "Expected at least one video format"
    );
}

/// Assert a video has chapters.
pub fn assert_has_chapters(video: &Video) {
    assert!(!video.chapters.is_empty(), "Expected video to have chapters");
}

/// Assert a video has a heatmap.
pub fn assert_has_heatmap(video: &Video) {
    assert!(
        video.heatmap.is_some() && !video.heatmap.as_ref().unwrap().is_empty(),
        "Expected video to have heatmap data"
    );
}

/// Assert a file exists at the given path.
pub fn assert_file_exists(path: &Path) {
    assert!(path.exists(), "Expected file to exist: {}", path.display());
}

/// Assert a file has a size strictly greater than `min_bytes`.
pub fn assert_file_size_gt(path: &Path, min_bytes: u64) {
    let metadata = std::fs::metadata(path).unwrap_or_else(|e| panic!("Cannot stat {}: {}", path.display(), e));
    assert!(
        metadata.len() > min_bytes,
        "Expected {} to be larger than {} bytes, got {} bytes",
        path.display(),
        min_bytes,
        metadata.len()
    );
}

/// Assert a Video has essential fields populated.
pub fn assert_video_valid(video: &Video) {
    assert!(!video.id.is_empty(), "Video id should not be empty");
    assert!(!video.title.is_empty(), "Video title should not be empty");
    assert!(!video.formats.is_empty(), "Video should have at least one format");
}

/// Assert an event was received on a broadcast channel within a timeout.
///
/// Returns the received event for further inspection.
pub async fn assert_event_received(
    rx: &mut tokio::sync::broadcast::Receiver<std::sync::Arc<yt_dlp::events::DownloadEvent>>,
) -> std::sync::Arc<yt_dlp::events::DownloadEvent> {
    tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("Timed out waiting for event")
        .expect("Broadcast channel closed or lagged")
}
