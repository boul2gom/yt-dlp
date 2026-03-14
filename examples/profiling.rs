//! Profiling harness for the yt-dlp library.
//!
//! This example exercises all major code paths and can be used with several
//! profiling tools to understand where time and memory are spent.
//!
//! # Usage
//!
//! ## CPU profiling with flamegraph
//! ```bash
//! cargo install flamegraph
//! cargo flamegraph --example profiling --features profiling --release -- <URL>
//! # Opens target/flamegraph.svg
//! ```
//!
//! ## CPU profiling with samply (macOS/Linux)
//! ```bash
//! cargo install samply
//! cargo build --example profiling --features profiling --release
//! samply record ./target/release/examples/profiling <URL>
//! # Opens Firefox profiler automatically
//! ```
//!
//! ## Heap profiling with dhat-rs
//! ```bash
//! cargo run --example profiling --features profiling -- <URL>
//! # Writes dhat-heap.json in the current directory
//! # Open at: https://nnethercote.github.io/dh_view/dh_view.html
//! ```
//!
//! ## Heap profiling with heaptrack (Linux only)
//! ```bash
//! cargo build --example profiling --release
//! heaptrack ./target/release/examples/profiling <URL>
//! heaptrack --analyze heaptrack.profiling.*
//! ```
//!
//! ## To run all scenarios
//! ```bash
//! cargo run --example profiling --features profiling -- <URL>
//! ```

#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tracing_subscriber::EnvFilter;
use yt_dlp::download::{AudioCodec, PostProcessConfig, VideoCodec};
use yt_dlp::events::{DownloadEvent, EventBus};
use yt_dlp::model::Video;
use yt_dlp::model::selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};
use yt_dlp::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};
use yt_dlp::{DownloadPriority, Downloader, VideoSelection};

// Default short public YouTube video used when no URL is provided.
const DEFAULT_VIDEO_URL: &str = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
// Number of format-selection iterations for the CPU-bound scenario.
const FORMAT_SELECTION_ITERS: usize = 10_000;
// Number of event-bus iterations (two loops of this count each).
const EVENT_BUS_ITERS: usize = 10_000;
// Number of URL-validation iterations for the validation scenario.
const VALIDATION_ITERS: usize = 10_000;
// Number of statistics snapshot iterations.
#[cfg(feature = "statistics")]
const STATISTICS_ITERS: usize = 1_000;
// Number of cache put+get cycle iterations.
const CACHE_OPS_ITERS: usize = 500;

struct ScenarioResult {
    name: String,
    iterations: usize,
    total: Duration,
}

impl ScenarioResult {
    fn avg(&self) -> Duration {
        if self.iterations == 0 {
            return Duration::ZERO;
        }
        self.total / self.iterations as u32
    }
}

struct Args {
    url: String,
    verbose: bool,
    scenario: Option<String>,
    cookies: Option<String>,
    cookies_from_browser: Option<String>,
    extra_args: Vec<String>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut url = DEFAULT_VIDEO_URL.to_string();
    let mut verbose = false;
    let mut scenario: Option<String> = None;
    let mut cookies = None;
    let mut cookies_from_browser = None;
    let mut extra_args = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--scenario" => {
                scenario = args.next();
            }
            "--cookies" => {
                cookies = args.next();
            }
            "--cookies-from-browser" => {
                cookies_from_browser = args.next();
            }
            s if s.starts_with("http") => url = s.to_string(),
            _ => extra_args.push(arg),
        }
    }

    Args {
        url,
        verbose,
        scenario,
        cookies,
        cookies_from_browser,
        extra_args,
    }
}

async fn setup_downloader(libs: &Path, output: &Path, args: &Args) -> yt_dlp::error::Result<Downloader> {
    tokio::fs::create_dir_all(libs).await?;
    tokio::fs::create_dir_all(output).await?;

    let mut builder = Downloader::with_new_binaries(libs, output).await?;

    if let Some(cookies) = &args.cookies {
        builder = builder.with_cookies(cookies);
    }
    if let Some(cookies_from_browser) = &args.cookies_from_browser {
        builder = builder.with_cookies_from_browser(cookies_from_browser);
    }

    let downloader = builder.with_args(args.extra_args.clone()).build().await?;

    Ok(downloader)
}

async fn run_setup(libs: &Path, output: &Path, args: &Args) -> ScenarioResult {
    let start = Instant::now();
    let _downloader = setup_downloader(libs, output, args).await.expect("setup failed");
    ScenarioResult {
        name: "setup".to_string(),
        iterations: 1,
        total: start.elapsed(),
    }
}

async fn run_metadata_cold(downloader: &Downloader, url: &str, n: usize) -> ScenarioResult {
    let start = Instant::now();
    for _ in 0..n {
        downloader
            .fetch_video_infos_fresh(url)
            .await
            .expect("metadata_cold fetch failed");
    }
    ScenarioResult {
        name: "metadata_cold".to_string(),
        iterations: n,
        total: start.elapsed(),
    }
}

async fn run_metadata_warm(downloader: &Downloader, url: &str, n: usize) -> ScenarioResult {
    // Warm up the cache with a single fetch
    let _ = downloader.fetch_video_infos(url).await;

    let start = Instant::now();
    for _ in 0..n {
        downloader
            .fetch_video_infos(url)
            .await
            .expect("metadata_warm fetch failed");
    }
    ScenarioResult {
        name: "metadata_warm".to_string(),
        iterations: n,
        total: start.elapsed(),
    }
}

fn run_format_selection(video: &Video) -> ScenarioResult {
    let start = Instant::now();
    for _ in 0..FORMAT_SELECTION_ITERS {
        let _ = video.best_video_format();
        let _ = video.best_audio_format();
        let _ = video.worst_video_format();
        let _ = video.worst_audio_format();
        let _ = video.select_video_format(VideoQuality::High, VideoCodecPreference::AVC1);
        let _ = video.select_audio_format(AudioQuality::Best, AudioCodecPreference::Opus);
    }
    ScenarioResult {
        name: "format_selection".to_string(),
        iterations: FORMAT_SELECTION_ITERS,
        total: start.elapsed(),
    }
}

async fn run_download_video(downloader: &Downloader, url: &str) -> ScenarioResult {
    let start = Instant::now();
    let video = downloader.fetch_video_infos(url).await.unwrap();
    downloader
        .download(&video, "profiling-video.mp4")
        .video_quality(VideoQuality::Worst)
        .audio_quality(AudioQuality::Worst)
        .execute()
        .await
        .expect("download_video failed");
    ScenarioResult {
        name: "download_video".to_string(),
        iterations: 1,
        total: start.elapsed(),
    }
}

async fn run_download_audio(downloader: &Downloader, url: &str) -> ScenarioResult {
    let start = Instant::now();
    let video = downloader.fetch_video_infos(url).await.unwrap();
    downloader
        .download_audio_stream_with_quality(
            &video,
            "profiling-audio.m4a",
            AudioQuality::Worst,
            AudioCodecPreference::Any,
        )
        .await
        .expect("download_audio failed");
    ScenarioResult {
        name: "download_audio".to_string(),
        iterations: 1,
        total: start.elapsed(),
    }
}

async fn run_download_concurrent(downloader: &Downloader, url: &str) -> ScenarioResult {
    use yt_dlp::DownloadPriority;

    let start = Instant::now();
    let video = downloader
        .fetch_video_infos(url)
        .await
        .expect("concurrent fetch failed");

    let id1 = downloader
        .download_video_with_priority(&video, "concurrent-1.mp4", Some(DownloadPriority::Normal))
        .await
        .expect("concurrent enqueue 1 failed");
    let id2 = downloader
        .download_video_with_priority(&video, "concurrent-2.mp4", Some(DownloadPriority::High))
        .await
        .expect("concurrent enqueue 2 failed");
    let id3 = downloader
        .download_video_with_priority(&video, "concurrent-3.mp4", Some(DownloadPriority::Low))
        .await
        .expect("concurrent enqueue 3 failed");

    downloader.wait_for_download(id1).await;
    downloader.wait_for_download(id2).await;
    downloader.wait_for_download(id3).await;

    ScenarioResult {
        name: "download_concurrent".to_string(),
        iterations: 3,
        total: start.elapsed(),
    }
}

async fn run_postprocess(downloader: &Downloader, url: &str) -> ScenarioResult {
    let video = downloader.fetch_video_infos(url).await.unwrap();

    // Download something small first so we have a file to process
    downloader
        .download(&video, "profiling-pp-input.mp4")
        .video_quality(VideoQuality::Worst)
        .audio_quality(AudioQuality::Worst)
        .execute()
        .await
        .expect("postprocess download failed");

    let config = PostProcessConfig::new()
        .with_video_codec(VideoCodec::H264)
        .with_audio_codec(AudioCodec::AAC);

    let start = Instant::now();
    downloader
        .postprocess_video("profiling-pp-input.mp4", "profiling-pp-output.mp4", config)
        .await
        .expect("postprocess failed");
    ScenarioResult {
        name: "postprocess".to_string(),
        iterations: 1,
        total: start.elapsed(),
    }
}

fn run_event_bus() -> ScenarioResult {
    let bus = EventBus::with_default_capacity();
    let mut rx = bus.subscribe();

    let start = Instant::now();

    // Loop 1: DownloadQueued events — realistic allocation cost (String + PathBuf per event)
    for i in 0..EVENT_BUS_ITERS {
        let event = DownloadEvent::DownloadQueued {
            download_id: i as u64,
            url: "https://example.com".to_string(),
            priority: DownloadPriority::Normal,
            output_path: PathBuf::from(format!("output-{}.mp4", i)),
        };
        bus.emit(event);
        // Drain the receiver to prevent lagging
        while rx.try_recv().is_ok() {}
    }

    // Loop 2: DownloadProgress events — all-numeric fields, zero extra allocations;
    // isolates pure channel throughput from event construction cost
    for i in 0..EVENT_BUS_ITERS {
        let event = DownloadEvent::DownloadProgress {
            download_id: i as u64,
            downloaded_bytes: (i as u64) * 1024,
            total_bytes: EVENT_BUS_ITERS as u64 * 1024,
            speed_bytes_per_sec: 1_000_000.0,
            eta_seconds: Some((EVENT_BUS_ITERS - i) as u64),
        };
        bus.emit(event);
        while rx.try_recv().is_ok() {}
    }

    ScenarioResult {
        name: "event_bus".to_string(),
        iterations: EVENT_BUS_ITERS * 2,
        total: start.elapsed(),
    }
}

#[cfg(feature = "statistics")]
async fn run_statistics(downloader: &Downloader) -> ScenarioResult {
    let start = Instant::now();
    for _ in 0..STATISTICS_ITERS {
        let _snapshot = downloader.statistics().snapshot().await;
    }
    ScenarioResult {
        name: "statistics".to_string(),
        iterations: STATISTICS_ITERS,
        total: start.elapsed(),
    }
}

#[cfg(cache)]
async fn run_cache_ops(real_video: &Video) -> ScenarioResult {
    use yt_dlp::cache::{CacheConfig, VideoCache};

    let dir = tempfile::TempDir::new().expect("tempdir failed");
    let config = CacheConfig::builder().cache_dir(dir.path().to_path_buf()).build();
    let cache = VideoCache::new(&config, None).await.expect("cache init failed");

    let start = Instant::now();
    for i in 0..CACHE_OPS_ITERS {
        let mut video = real_video.clone();
        video.id = format!("bench-{}", i);
        let url = format!("https://example.com/bench-{}", i);
        cache.put(url.clone(), video).await.expect("put failed");
        let _ = cache.get(&url).await.expect("get failed");
    }
    ScenarioResult {
        name: "cache_ops".to_string(),
        iterations: CACHE_OPS_ITERS,
        total: start.elapsed(),
    }
}

fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= 1.0 {
        format!("{:.3}s", secs)
    } else if secs >= 0.001 {
        format!("{:.1}ms", secs * 1000.0)
    } else {
        format!("{:.1}µs", secs * 1_000_000.0)
    }
}

fn print_results(results: &[ScenarioResult]) {
    println!("\n=== yt-dlp Profiling Results ===");
    println!("{:<36} {:>6}  {:>10}  {:>10}", "Scenario", "Iters", "Total", "Avg/iter");
    println!("{}", "─".repeat(68));
    for r in results {
        println!(
            "{:<36} {:>6}  {:>10}  {:>10}",
            r.name,
            r.iterations,
            fmt_duration(r.total),
            fmt_duration(r.avg()),
        );
    }
    println!();
}

async fn run_cpu_scenarios(run_scenario: impl Fn(&str) -> bool, real_video: &Video, results: &mut Vec<ScenarioResult>) {
    if run_scenario("format_selection") {
        println!("[format_selection] Running 10,000 iterations on real video...");
        let r = run_format_selection(real_video);
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }

    if run_scenario("event_bus") {
        println!("[event_bus] Emitting 10,000 events...");
        let r = run_event_bus();
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }

    if run_scenario("validation") {
        let inputs = [
            "https://www.youtube.com/watch?v=gXtp6C-3JKo",
            "https://youtu.be/jNQXAC9IVRw",
            "https://vimeo.com/12345",
            "not-a-url",
        ];
        let start = Instant::now();
        for _ in 0..VALIDATION_ITERS {
            for url in &inputs {
                let _ = validate_youtube_url(url);
            }
            let _ = sanitize_filename("My video: great/stuff\\file.mp4");
            let _ = sanitize_path("downloads/my video.mp4");
        }
        results.push(ScenarioResult {
            name: "validation".to_string(),
            iterations: VALIDATION_ITERS,
            total: start.elapsed(),
        });
    }
}

async fn run_network_scenarios(
    run_scenario: impl Fn(&str) -> bool,
    downloader: &Downloader,
    url: &str,
    results: &mut Vec<ScenarioResult>,
) {
    if run_scenario("metadata_cold") {
        println!("[metadata_cold] Fetching fresh metadata 3 times...");
        let r = run_metadata_cold(downloader, url, 3).await;
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }

    if run_scenario("metadata_warm") {
        println!("[metadata_warm] Fetching cached metadata 10 times...");
        let r = run_metadata_warm(downloader, url, 10).await;
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }

    if run_scenario("download_video") {
        println!("[download_video] Downloading lowest quality video...");
        let r = run_download_video(downloader, url).await;
        println!("  done in {}", fmt_duration(r.total));
        results.push(r);
    }

    if run_scenario("download_audio") {
        println!("[download_audio] Downloading lowest quality audio...");
        let r = run_download_audio(downloader, url).await;
        println!("  done in {}", fmt_duration(r.total));
        results.push(r);
    }

    if run_scenario("download_concurrent") {
        println!("[download_concurrent] Downloading 3 concurrent streams...");
        let r = run_download_concurrent(downloader, url).await;
        println!("  done in {} total", fmt_duration(r.total));
        results.push(r);
    }

    if run_scenario("postprocess") {
        println!("[postprocess] Post-processing with H264/AAC...");
        let r = run_postprocess(downloader, url).await;
        println!("  done in {}", fmt_duration(r.total));
        results.push(r);
    }
}

#[cfg(cache)]
async fn run_cache_scenarios(
    run_scenario: impl Fn(&str) -> bool,
    real_video: &Video,
    results: &mut Vec<ScenarioResult>,
) {
    if run_scenario("cache_ops") {
        println!("[cache_ops] Running 500 put+get cycles on in-memory cache...");
        let r = run_cache_ops(real_video).await;
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }
}

#[cfg(feature = "statistics")]
async fn run_statistics_scenarios(
    run_scenario: impl Fn(&str) -> bool,
    downloader: &Downloader,
    results: &mut Vec<ScenarioResult>,
) {
    if run_scenario("statistics") {
        println!("[statistics] Snapshotting statistics 1,000 times...");
        let r = run_statistics(downloader).await;
        println!(
            "  done in {} total, {} avg",
            fmt_duration(r.total),
            fmt_duration(r.avg())
        );
        results.push(r);
    }
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "profiling")]
    let _profiler = dhat::Profiler::new_heap();

    let args = parse_args();

    let level = if args.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level)))
        .init();

    let libs = PathBuf::from("profiling-libs");
    let output = PathBuf::from("profiling-output");

    let mut results: Vec<ScenarioResult> = Vec::new();

    let run_scenario = |name: &str| -> bool { args.scenario.as_deref().is_none_or(|s| s == name) };

    if run_scenario("setup") {
        println!("[setup] Building downloader and checking binaries...");
        let r = run_setup(&libs, &output, &args).await;
        println!("  done in {}", fmt_duration(r.total));
        results.push(r);
    }

    let downloader = setup_downloader(&libs, &output, &args)
        .await
        .expect("failed to build downloader");

    let url = args.url.as_str();
    let real_video: Video = downloader
        .fetch_video_infos(url)
        .await
        .expect("failed to fetch real metadata");

    run_cpu_scenarios(&run_scenario, &real_video, &mut results).await;
    run_network_scenarios(&run_scenario, &downloader, url, &mut results).await;

    #[cfg(cache)]
    run_cache_scenarios(&run_scenario, &real_video, &mut results).await;

    #[cfg(feature = "statistics")]
    run_statistics_scenarios(&run_scenario, &downloader, &mut results).await;

    print_results(&results);

    #[cfg(feature = "statistics")]
    if let Ok(downloader) = setup_downloader(&libs, &output, &args).await {
        let snap = downloader.statistics().snapshot().await;
        println!("=== Statistics Snapshot ===");
        println!("Downloads completed: {}", snap.downloads.completed);
        println!("Downloads failed:    {}", snap.downloads.failed);
        println!("Total bytes:         {}", snap.downloads.total_bytes);
    }
}
