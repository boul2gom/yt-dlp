use std::time::Duration;

use yt_dlp::download::DownloadPriority;
use yt_dlp::stats::{
    ActiveDownloadSnapshot, DownloadOutcomeSnapshot, DownloadSnapshot, DownloadStats, FetchStats, GlobalSnapshot,
    PlaylistStats, PostProcessStats,
};

// ============================== DownloadStats Display ==============================

#[test]
fn display_download_stats_shows_key_counts() {
    let stats = DownloadStats {
        attempted: 10,
        completed: 7,
        failed: 2,
        canceled: 1,
        queued: 0,
        total_bytes: 1024 * 1024,
        total_retries: 3,
        total_duration: Duration::from_secs(60),
        avg_duration: Some(Duration::from_secs(8)),
        avg_speed_bytes_per_sec: Some(100_000.0),
        peak_speed_bytes_per_sec: 200_000.0,
        success_rate: Some(0.7),
    };
    let display = format!("{}", stats);
    assert!(display.contains("10"));
    assert!(display.contains("7"));
    assert!(display.contains("2"));
    assert!(display.contains("DownloadStats"));
}

#[test]
fn create_download_stats_zero_activity_has_none_fields() {
    let stats = DownloadStats {
        attempted: 0,
        completed: 0,
        failed: 0,
        canceled: 0,
        queued: 0,
        total_bytes: 0,
        total_retries: 0,
        total_duration: Duration::ZERO,
        avg_duration: None,
        avg_speed_bytes_per_sec: None,
        peak_speed_bytes_per_sec: 0.0,
        success_rate: None,
    };
    assert!(stats.avg_duration.is_none());
    assert!(stats.avg_speed_bytes_per_sec.is_none());
    assert!(stats.success_rate.is_none());
}

// ============================== FetchStats Display ==============================

#[test]
fn display_fetch_stats_shows_key_counts() {
    let stats = FetchStats {
        attempted: 5,
        succeeded: 4,
        failed: 1,
        avg_duration: Some(Duration::from_millis(200)),
        success_rate: Some(0.8),
    };
    let display = format!("{}", stats);
    assert!(display.contains("5"));
    assert!(display.contains("4"));
    assert!(display.contains("1"));
    assert!(display.contains("FetchStats"));
}

#[test]
fn create_fetch_stats_zero_activity_has_none_fields() {
    let stats = FetchStats {
        attempted: 0,
        succeeded: 0,
        failed: 0,
        avg_duration: None,
        success_rate: None,
    };
    assert!(stats.success_rate.is_none());
    assert!(stats.avg_duration.is_none());
}

// ============================== PostProcessStats Display ==============================

#[test]
fn display_post_process_stats_shows_key_counts() {
    let stats = PostProcessStats {
        attempted: 3,
        succeeded: 3,
        failed: 0,
        avg_duration: Some(Duration::from_secs(5)),
        success_rate: Some(1.0),
    };
    let display = format!("{}", stats);
    assert!(display.contains("3"));
    assert!(display.contains("PostProcessStats"));
}

// ============================== PlaylistStats Display ==============================

#[test]
fn display_playlist_stats_shows_key_counts() {
    let stats = PlaylistStats {
        playlists_fetched: 2,
        playlists_fetch_failed: 0,
        items_successful: 10,
        items_failed: 1,
        item_success_rate: Some(0.9),
    };
    let display = format!("{}", stats);
    assert!(display.contains("2"));
    assert!(display.contains("10"));
    assert!(display.contains("1"));
    assert!(display.contains("PlaylistStats"));
}

#[test]
fn create_playlist_stats_zero_items_has_none_rate() {
    let stats = PlaylistStats {
        playlists_fetched: 0,
        playlists_fetch_failed: 0,
        items_successful: 0,
        items_failed: 0,
        item_success_rate: None,
    };
    assert!(stats.item_success_rate.is_none());
}

// ============================== ActiveDownloadSnapshot Display ==============================

#[test]
fn display_active_download_snapshot_shows_key_fields() {
    let snap = ActiveDownloadSnapshot {
        download_id: 42,
        url: "https://example.com".to_string(),
        priority: DownloadPriority::Normal,
        downloaded_bytes: 512,
        total_bytes: 1024,
        progress: Some(0.5),
        peak_speed_bytes_per_sec: 50_000.0,
        elapsed: Some(Duration::from_secs(2)),
        time_since_queued: Duration::from_secs(3),
    };
    let display = format!("{}", snap);
    assert!(display.contains("42"));
    assert!(display.contains("512"));
    assert!(display.contains("1024"));
    assert!(display.contains("ActiveDownloadSnapshot"));
}

#[test]
fn compute_active_snapshot_progress_some_when_total_nonzero() {
    let snap = ActiveDownloadSnapshot {
        download_id: 1,
        url: "https://example.com".to_string(),
        priority: DownloadPriority::Low,
        downloaded_bytes: 250,
        total_bytes: 1000,
        progress: Some(0.25),
        peak_speed_bytes_per_sec: 0.0,
        elapsed: None,
        time_since_queued: Duration::ZERO,
    };
    assert_eq!(snap.progress, Some(0.25));
}

#[test]
fn compute_active_snapshot_progress_none_when_total_zero() {
    let snap = ActiveDownloadSnapshot {
        download_id: 1,
        url: "https://example.com".to_string(),
        priority: DownloadPriority::Normal,
        downloaded_bytes: 0,
        total_bytes: 0,
        progress: None,
        peak_speed_bytes_per_sec: 0.0,
        elapsed: None,
        time_since_queued: Duration::ZERO,
    };
    assert!(snap.progress.is_none());
}

// ============================== DownloadOutcomeSnapshot Display ==============================

#[test]
fn display_outcome_completed_shows_completed_string() {
    assert_eq!(format!("{}", DownloadOutcomeSnapshot::Completed), "Completed");
}

#[test]
fn display_outcome_failed_shows_failed_string() {
    assert_eq!(format!("{}", DownloadOutcomeSnapshot::Failed), "Failed");
}

#[test]
fn display_outcome_canceled_shows_canceled_string() {
    assert_eq!(format!("{}", DownloadOutcomeSnapshot::Canceled), "Canceled");
}

// ============================== DownloadSnapshot Display ==============================

#[test]
fn display_download_snapshot_shows_key_fields() {
    let snap = DownloadSnapshot {
        download_id: 7,
        url: "https://example.com/video".to_string(),
        priority: DownloadPriority::High,
        outcome: DownloadOutcomeSnapshot::Completed,
        bytes: 2048,
        duration: Some(Duration::from_secs(10)),
        queue_wait: Some(Duration::from_millis(100)),
        peak_speed_bytes_per_sec: 1_000_000.0,
        retry_count: 0,
    };
    let display = format!("{}", snap);
    assert!(display.contains("7"));
    assert!(display.contains("Completed"));
    assert!(display.contains("2048"));
    assert!(display.contains("DownloadSnapshot"));
}

// ============================== GlobalSnapshot Display ==============================

#[test]
fn display_global_snapshot_shows_active_count() {
    let snap = GlobalSnapshot {
        downloads: DownloadStats {
            attempted: 5,
            completed: 4,
            failed: 1,
            canceled: 0,
            queued: 0,
            total_bytes: 100_000,
            total_retries: 0,
            total_duration: Duration::from_secs(20),
            avg_duration: Some(Duration::from_secs(5)),
            avg_speed_bytes_per_sec: Some(5_000.0),
            peak_speed_bytes_per_sec: 10_000.0,
            success_rate: Some(0.8),
        },
        fetches: FetchStats {
            attempted: 5,
            succeeded: 5,
            failed: 0,
            avg_duration: None,
            success_rate: Some(1.0),
        },
        post_processing: PostProcessStats {
            attempted: 4,
            succeeded: 4,
            failed: 0,
            avg_duration: None,
            success_rate: Some(1.0),
        },
        playlists: PlaylistStats {
            playlists_fetched: 1,
            playlists_fetch_failed: 0,
            items_successful: 5,
            items_failed: 0,
            item_success_rate: Some(1.0),
        },
        active_count: 0,
        active_downloads: vec![],
        recent_downloads: vec![],
    };
    let display = format!("{}", snap);
    assert!(display.contains("GlobalSnapshot"));
    assert!(display.contains("active=0"));
}
