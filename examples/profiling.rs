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
//! cargo run --example profiling --features profiling --release -- <URL>
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
//! ## Dry-run (no network required)
//! ```bash
//! cargo run --example profiling --features profiling --release -- --dry-run
//! ```

#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use yt_dlp::Downloader;
use yt_dlp::VideoSelection;
use yt_dlp::download::postprocess::{AudioCodec, PostProcessConfig, VideoCodec};
use yt_dlp::events::{DownloadEvent, EventBus};
use yt_dlp::model::Video;
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use yt_dlp::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

// Default short public YouTube video used when no URL is provided
const DEFAULT_VIDEO_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

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
    dry_run: bool,
    scenario: Option<String>,
    cookies: Option<String>,
    cookies_from_browser: Option<String>,
    extra_args: Vec<String>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut url = DEFAULT_VIDEO_URL.to_string();
    let mut verbose = false;
    let mut dry_run = false;
    let mut scenario: Option<String> = None;
    let mut cookies = None;
    let mut cookies_from_browser = None;
    let mut extra_args = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--dry-run" => dry_run = true,
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
        dry_run,
        scenario,
        cookies,
        cookies_from_browser,
        extra_args,
    }
}

async fn setup_downloader(
    libs: &Path,
    output: &Path,
    args: &Args,
) -> yt_dlp::error::Result<Downloader> {
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

fn make_format(
    id: &str,
    height: Option<u32>,
    vcodec: Option<&str>,
    acodec: Option<&str>,
    vbr: Option<f64>,
    abr: Option<f64>,
) -> yt_dlp::model::format::Format {
    use ordered_float::OrderedFloat;
    use yt_dlp::model::format::*;

    Format {
        format: format!("{} - {}x{}", id, height.unwrap_or(0), height.unwrap_or(0)),
        format_id: id.to_string(),
        format_note: None,
        protocol: Protocol::Https,
        language: None,
        has_drm: None,
        container: None,
        codec_info: CodecInfo {
            audio_codec: acodec.map(str::to_string),
            video_codec: vcodec.map(str::to_string),
            audio_ext: Extension::Unknown,
            video_ext: Extension::Unknown,
            audio_channels: acodec.map(|_| 2),
            asr: acodec.map(|_| 48000),
        },
        video_resolution: VideoResolution {
            width: height.map(|h| h * 16 / 9),
            height,
            resolution: height.map(|h| format!("{}x{}", h * 16 / 9, h)),
            fps: height.map(|_| OrderedFloat(30.0)),
            aspect_ratio: Some(OrderedFloat(16.0 / 9.0)),
        },
        download_info: DownloadInfo {
            url: Some(format!("https://example.com/stream/{}", id)),
            ext: Extension::Mp4,
            http_headers: HttpHeaders {
                user_agent: "Mozilla/5.0".to_string(),
                accept: "*/*".to_string(),
                accept_language: "en-US,en;q=0.9".to_string(),
                sec_fetch_mode: "navigate".to_string(),
            },
            manifest_url: None,
            downloader_options: None,
        },
        quality_info: QualityInfo {
            quality: height.map(|h| OrderedFloat(h as f64)),
            dynamic_range: None,
        },
        file_info: FileInfo {
            filesize_approx: height.map(|h| (h as i64) * 100_000),
            filesize: None,
        },
        storyboard_info: StoryboardInfo {
            rows: None,
            columns: None,
            fragments: None,
        },
        rates_info: RatesInfo {
            video_rate: vbr.map(OrderedFloat),
            audio_rate: abr.map(OrderedFloat),
            total_rate: Some(OrderedFloat(vbr.unwrap_or(0.0) + abr.unwrap_or(0.0))),
        },
        video_id: None,
    }
}

fn make_video(n_formats: usize) -> Video {
    use yt_dlp::model::video::{ExtractorInfo, Version};

    let heights = [2160u32, 1440, 1080, 720, 480, 360, 240, 144];
    let video_codecs = ["vp9", "avc1.42E01E", "av01.0.05M.08"];
    let audio_codecs = ["opus", "mp4a.40.2"];

    let mut formats = Vec::with_capacity(n_formats);
    let mut idx = 0usize;

    'outer: for &height in &heights {
        for &vcodec in &video_codecs {
            let vbr = height as f64 * 0.5;
            formats.push(make_format(
                &format!("{}-v{}", height, idx),
                Some(height),
                Some(vcodec),
                None,
                Some(vbr),
                None,
            ));
            idx += 1;
            if idx >= n_formats {
                break 'outer;
            }
        }
    }

    // Fill remaining slots with audio-only formats
    let audio_idx = idx;
    for i in 0..(n_formats.saturating_sub(audio_idx)) {
        let acodec = audio_codecs[i % audio_codecs.len()];
        let abr = 128.0 + (i as f64) * 32.0;
        formats.push(make_format(
            &format!("a{}", i),
            None,
            None,
            Some(acodec),
            None,
            Some(abr),
        ));
    }

    Video {
        id: "jNQXAC9IVRw".to_string(),
        title: "Me at the zoo".to_string(),
        thumbnail: None,
        description: Some("The first YouTube video.".to_string()),
        availability: Some("public".to_string()),
        upload_date: Some(1113005569),
        view_count: Some(300_000_000),
        like_count: Some(10_000_000),
        comment_count: Some(5_000_000),
        channel: Some("jawed".to_string()),
        channel_id: Some("UC4QobU6STFB0P71PMvkgx5g".to_string()),
        channel_url: Some("https://www.youtube.com/channel/UC4QobU6STFB0P71PMvkgx5g".to_string()),
        channel_follower_count: Some(1_000_000),
        uploader: Some("jawed".to_string()),
        uploader_id: Some("jawed".to_string()),
        formats,
        thumbnails: Vec::new(),
        automatic_captions: std::collections::HashMap::new(),
        subtitles: std::collections::HashMap::new(),
        chapters: Vec::new(),
        heatmap: None,
        tags: vec!["zoo".to_string(), "animals".to_string()],
        categories: vec!["Pets & Animals".to_string()],
        age_limit: 0,
        has_drm: None,
        live_status: "not_live".to_string(),
        playable_in_embed: true,
        extractor_info: ExtractorInfo {
            extractor: "youtube".to_string(),
            extractor_key: "Youtube".to_string(),
        },
        version: Version {
            version: "2024.10.22".to_string(),
            current_git_head: None,
            release_git_head: None,
            repository: "yt-dlp/yt-dlp".to_string(),
        },
    }
}

async fn run_setup(libs: &Path, output: &Path, args: &Args) -> ScenarioResult {
    let start = Instant::now();
    let _downloader = setup_downloader(libs, output, args)
        .await
        .expect("setup failed");
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
    const N: usize = 10_000;
    let start = Instant::now();
    for _ in 0..N {
        let _ = video.best_video_format();
        let _ = video.best_audio_format();
        let _ = video.worst_video_format();
        let _ = video.worst_audio_format();
        let _ = video.select_video_format(VideoQuality::High, VideoCodecPreference::AVC1);
        let _ = video.select_audio_format(AudioQuality::Best, AudioCodecPreference::Opus);
    }
    ScenarioResult {
        name: "format_selection".to_string(),
        iterations: N,
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
    use yt_dlp::model::selector::AudioCodecPreference;
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
    use yt_dlp::download::DownloadPriority;

    const N: usize = 10_000;
    let bus = EventBus::with_default_capacity();
    let mut rx = bus.subscribe();

    let start = Instant::now();
    for i in 0..N {
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
    ScenarioResult {
        name: "event_bus".to_string(),
        iterations: N,
        total: start.elapsed(),
    }
}

#[cfg(feature = "statistics")]
async fn run_statistics(downloader: &Downloader) -> ScenarioResult {
    const N: usize = 1_000;
    let start = Instant::now();
    for _ in 0..N {
        let _snapshot = downloader.statistics().snapshot().await;
    }
    ScenarioResult {
        name: "statistics".to_string(),
        iterations: N,
        total: start.elapsed(),
    }
}

#[cfg(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))]
async fn run_cache_ops() -> ScenarioResult {
    use yt_dlp::cache::VideoCache;

    const N: usize = 500;
    let dir = tempfile::TempDir::new().expect("tempdir failed");
    let cache = VideoCache::new(dir.path(), None)
        .await
        .expect("cache init failed");

    let start = Instant::now();
    for i in 0..N {
        let mut video = make_video(5);
        video.id = format!("bench-{}", i);
        let url = format!("https://example.com/bench-{}", i);
        cache.put(url.clone(), video).await.expect("put failed");
        let _ = cache.get(&url).await.expect("get failed");
    }
    ScenarioResult {
        name: "cache_ops".to_string(),
        iterations: N,
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
    println!(
        "{:<36} {:>6}  {:>10}  {:>10}",
        "Scenario", "Iters", "Total", "Avg/iter"
    );
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

#[tokio::main]
async fn main() {
    #[cfg(feature = "profiling")]
    let _profiler = dhat::Profiler::new_heap();

    let args = parse_args();

    let level = if args.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
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

    let synthetic_video = make_video(50);

    if run_scenario("format_selection") {
        println!("[format_selection] Running 10,000 iterations on synthetic video...");
        let r = run_format_selection(&synthetic_video);
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

    // Validation benchmarks (no network, illustrate overhead)
    if run_scenario("validation") {
        let inputs = [
            "https://www.youtube.com/watch?v=jNQXAC9IVRw",
            "https://youtu.be/jNQXAC9IVRw",
            "https://vimeo.com/12345",
            "not-a-url",
        ];
        let start = Instant::now();
        const N: usize = 10_000;
        for _ in 0..N {
            for url in &inputs {
                let _ = validate_youtube_url(url);
            }
            let _ = sanitize_filename("My video: great/stuff\\file.mp4");
            let _ = sanitize_path("downloads/my video.mp4");
        }
        results.push(ScenarioResult {
            name: "validation".to_string(),
            iterations: N,
            total: start.elapsed(),
        });
    }

    if !args.dry_run {
        let downloader = setup_downloader(&libs, &output, &args)
            .await
            .expect("failed to build downloader");

        let url = args.url.as_str();

        if run_scenario("metadata_cold") {
            println!("[metadata_cold] Fetching fresh metadata 3 times...");
            let r = run_metadata_cold(&downloader, url, 3).await;
            println!(
                "  done in {} total, {} avg",
                fmt_duration(r.total),
                fmt_duration(r.avg())
            );
            results.push(r);
        }

        if run_scenario("metadata_warm") {
            println!("[metadata_warm] Fetching cached metadata 10 times...");
            let r = run_metadata_warm(&downloader, url, 10).await;
            println!(
                "  done in {} total, {} avg",
                fmt_duration(r.total),
                fmt_duration(r.avg())
            );
            results.push(r);
        }

        if run_scenario("download_video") {
            println!("[download_video] Downloading lowest quality video...");
            let r = run_download_video(&downloader, url).await;
            println!("  done in {}", fmt_duration(r.total));
            results.push(r);
        }

        if run_scenario("download_audio") {
            println!("[download_audio] Downloading lowest quality audio...");
            let r = run_download_audio(&downloader, url).await;
            println!("  done in {}", fmt_duration(r.total));
            results.push(r);
        }

        if run_scenario("download_concurrent") {
            println!("[download_concurrent] Downloading 3 concurrent streams...");
            let r = run_download_concurrent(&downloader, url).await;
            println!("  done in {} total", fmt_duration(r.total));
            results.push(r);
        }

        if run_scenario("postprocess") {
            println!("[postprocess] Post-processing with H264/AAC...");
            let r = run_postprocess(&downloader, url).await;
            println!("  done in {}", fmt_duration(r.total));
            results.push(r);
        }

        #[cfg(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))]
        if run_scenario("cache_ops") {
            println!("[cache_ops] Running 500 put+get cycles on in-memory cache...");
            let r = run_cache_ops().await;
            println!(
                "  done in {} total, {} avg",
                fmt_duration(r.total),
                fmt_duration(r.avg())
            );
            results.push(r);
        }

        #[cfg(feature = "statistics")]
        if run_scenario("statistics") {
            println!("[statistics] Snapshotting statistics 1,000 times...");
            let r = run_statistics(&downloader).await;
            println!(
                "  done in {} total, {} avg",
                fmt_duration(r.total),
                fmt_duration(r.avg())
            );
            results.push(r);
        }
    } else {
        println!("[dry-run] Skipping network scenarios.");
    }

    print_results(&results);

    #[cfg(feature = "statistics")]
    if !args.dry_run
        && let Ok(downloader) = setup_downloader(&libs, &output, &args).await
    {
        let snap = downloader.statistics().snapshot().await;
        println!("=== Statistics Snapshot ===");
        println!("Downloads completed: {}", snap.downloads.completed);
        println!("Downloads failed:    {}", snap.downloads.failed);
        println!("Total bytes:         {}", snap.downloads.total_bytes);
    }
}
