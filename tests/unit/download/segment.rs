use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tempfile::TempDir;
use yt_dlp::download::engine::segment::SegmentContext;

fn make_temp_file() -> (Arc<std::fs::File>, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("segment_test.bin");
    let file = Arc::new(std::fs::File::create(path).unwrap());
    (file, dir)
}

// ============================== SegmentContext::new ==============================

#[test]
fn create_segment_context_initializes_all_fields() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext::new(file, 1024, None);
    assert_eq!(ctx.total_bytes, 1024);
    assert_eq!(ctx.downloaded_bytes.load(Ordering::Relaxed), 0);
    assert!(ctx.progress_callback.is_none());
    assert_eq!(ctx.file_offset_base, 0);
    assert!(!ctx.is_resuming);
}

#[test]
fn create_segment_context_with_callback_stores_it() {
    let (file, _dir) = make_temp_file();
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    // ProgressCallback is pub(crate) — construct the underlying type directly
    let callback: Arc<dyn Fn(u64, u64) + Send + Sync> = Arc::new(move |_downloaded, _total| {
        called_clone.store(true, Ordering::Relaxed);
    });

    let ctx = SegmentContext::new(file, 2048, Some(callback));
    assert!(ctx.progress_callback.is_some());
    assert_eq!(ctx.total_bytes, 2048);
}

// ============================== SegmentContext::update_progress ==============================

#[test]
fn update_segment_context_progress_increments_bytes() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext::new(file, 1000, None);

    ctx.update_progress(100);
    assert_eq!(ctx.downloaded_bytes.load(Ordering::Relaxed), 100);

    ctx.update_progress(200);
    assert_eq!(ctx.downloaded_bytes.load(Ordering::Relaxed), 300);
}

#[test]
fn update_segment_context_progress_calls_callback() {
    let (file, _dir) = make_temp_file();
    let received_downloaded = Arc::new(AtomicU64::new(0));
    let received_total = Arc::new(AtomicU64::new(0));

    let dl_clone = received_downloaded.clone();
    let tot_clone = received_total.clone();
    let callback: Arc<dyn Fn(u64, u64) + Send + Sync> = Arc::new(move |downloaded, total| {
        dl_clone.store(downloaded, Ordering::Relaxed);
        tot_clone.store(total, Ordering::Relaxed);
    });

    let ctx = SegmentContext::new(file, 500, Some(callback));
    ctx.update_progress(150);

    assert_eq!(received_downloaded.load(Ordering::Relaxed), 150);
    assert_eq!(received_total.load(Ordering::Relaxed), 500);
}

#[test]
fn update_segment_context_progress_without_callback_does_not_panic() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext::new(file, 100, None);
    // Should not panic with no callback
    ctx.update_progress(50);
    assert_eq!(ctx.downloaded_bytes.load(Ordering::Relaxed), 50);
}

// ============================== SegmentContext Debug ==============================

#[test]
fn debug_segment_context_shows_key_fields() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext::new(file, 1024, None);
    let debug = format!("{:?}", ctx);
    assert!(debug.contains("SegmentContext"));
    assert!(debug.contains("downloaded_bytes"));
    assert!(debug.contains("total_bytes"));
    assert!(debug.contains("has_callback"));
}

// ============================== SegmentContext Display ==============================

#[test]
fn display_segment_context_shows_bytes() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext::new(file, 2048, None);
    ctx.update_progress(512);

    let display = format!("{}", ctx);
    assert!(display.contains("SegmentContext"));
    assert!(display.contains("512"));
    assert!(display.contains("2048"));
}

// ============================== Struct literal / field access ==============================

#[test]
fn create_segment_context_literal_with_resuming_flag_true() {
    let (file, _dir) = make_temp_file();
    let ctx = SegmentContext {
        file,
        downloaded_bytes: Arc::new(AtomicU64::new(0)),
        progress_callback: None,
        total_bytes: 4096,
        file_offset_base: 1024,
        is_resuming: true,
    };
    assert!(ctx.is_resuming);
    assert_eq!(ctx.file_offset_base, 1024);
    assert_eq!(ctx.total_bytes, 4096);
}
