use std::time::Duration;

use tokio_stream::StreamExt;
use yt_dlp::download::config::progress::{ProgressInfo, ProgressTracker};

// ============================== ProgressTracker ==============================

#[tokio::test]
async fn progress_tracker_single_update() {
    let tracker = ProgressTracker::new();
    let mut stream = tracker.stream();

    tracker.update(500, 1000);
    drop(tracker);

    let info = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .expect("timeout waiting for progress")
        .expect("stream ended early")
        .expect("broadcast recv error");

    assert_eq!(info.downloaded, 500);
    assert_eq!(info.total, 1000);
    assert!((info.percentage() - 0.5).abs() < f64::EPSILON);
}

#[tokio::test]
async fn progress_tracker_multiple_updates() {
    let tracker = ProgressTracker::new();
    let mut stream = tracker.stream();

    tracker.update(100, 1000);
    tracker.update(500, 1000);
    tracker.update(1000, 1000);
    drop(tracker);

    let mut received = Vec::new();
    while let Ok(Some(Ok(info))) = tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
        received.push(info);
    }

    assert_eq!(received.len(), 3);
    assert_eq!(received[0].downloaded, 100);
    assert_eq!(received[1].downloaded, 500);
    assert_eq!(received[2].downloaded, 1000);
    assert!((received[2].percentage() - 1.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn progress_tracker_multiple_subscribers() {
    let tracker = ProgressTracker::new();
    let mut stream1 = tracker.stream();
    let mut stream2 = tracker.stream();

    tracker.update(250, 500);
    drop(tracker);

    let p1 = tokio::time::timeout(Duration::from_millis(100), stream1.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let p2 = tokio::time::timeout(Duration::from_millis(100), stream2.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(p1, p2);
    assert_eq!(p1.downloaded, 250);
    assert_eq!(p1.total, 500);
}

#[tokio::test]
async fn progress_tracker_callback() {
    let tracker = ProgressTracker::new();
    let mut stream = tracker.stream();

    let cb = tracker.callback();
    cb(300, 600);
    drop(tracker);
    drop(cb);

    let info = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(info.downloaded, 300);
    assert_eq!(info.total, 600);
}

#[tokio::test]
async fn progress_tracker_callback_thread_safe() {
    let tracker = ProgressTracker::new();
    let mut stream = tracker.stream();

    let cb = tracker.callback();
    let handle = tokio::task::spawn_blocking(move || {
        cb(1000, 1000);
    });
    handle.await.unwrap();
    drop(tracker);

    let info = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(info.downloaded, 1000);
    assert_eq!(info.total, 1000);
}

#[tokio::test]
async fn progress_tracker_default() {
    let tracker = ProgressTracker::default();
    let mut stream = tracker.stream();

    tracker.update(1, 10);
    drop(tracker);

    let info = tokio::time::timeout(Duration::from_millis(100), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(info.downloaded, 1);
    assert_eq!(info.total, 10);
}

// ============================== ProgressInfo ==============================

#[test]
fn progress_info_equality() {
    let a = ProgressInfo::new(100, 200);
    let b = ProgressInfo::new(100, 200);
    assert_eq!(a, b);
}

#[test]
fn progress_info_copy() {
    let a = ProgressInfo::new(10, 20);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn progress_info_percentage_complete() {
    let info = ProgressInfo::new(1000, 1000);
    assert!((info.percentage() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn progress_info_percentage_zero_downloaded() {
    let info = ProgressInfo::new(0, 1000);
    assert!((info.percentage()).abs() < f64::EPSILON);
}

#[test]
fn progress_info_debug() {
    let info = ProgressInfo::new(42, 100);
    let debug = format!("{:?}", info);
    assert!(debug.contains("ProgressInfo"));
    assert!(debug.contains("42"));
}
