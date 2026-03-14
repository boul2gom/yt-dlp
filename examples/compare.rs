//! Raw yt-dlp subprocess vs library performance comparison.
//!
//! Iterates over every scenario from the README performance table, measures
//! raw yt-dlp **and** the three library speed profiles (Conservative, Balanced,
//! Aggressive), then prints the results as Markdown tables ready to paste into
//! the README.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example compare --features profiling -- <URL> [--runs <N>]
//! ```
//!
//! ## Options
//!
//! - `<URL>` — YouTube URL (required, positional)
//! - `--runs <N>` — repetition count per measurement (default: 3)
//! - `--cookies <file>` — pass a Netscape cookie file to yt-dlp
//! - `--cookies-from-browser <name>` — extract cookies from browser (chrome, firefox, …)

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use console::{Term, style};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tracing_subscriber::EnvFilter;
use yt_dlp::download::SpeedProfile;
use yt_dlp::executor::Executor;
use yt_dlp::model::Video;
use yt_dlp::model::format::FormatType;
use yt_dlp::model::selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};
use yt_dlp::{Downloader, VideoSelection};

// Default number of download repetitions per scenario.
const DEFAULT_RUNS: usize = 3;
// Spinner animation tick interval in milliseconds.
const SPINNER_TICK_MILLIS: u64 = 80;
// Timeout for each raw yt-dlp download run, in seconds.
const RAW_DOWNLOAD_TIMEOUT_SECS: u64 = 300;
// Timeout for the initial info-json dump, in seconds.
const INFO_JSON_TIMEOUT_SECS: u64 = 120;
// Number of measurements per scenario: raw + Conservative + Balanced + Aggressive.
const PROFILES_PER_SCENARIO: usize = 4;

struct Args {
    url: String,
    runs: usize,
    cookies: Option<String>,
    cookies_from_browser: Option<String>,
}

fn parse_args() -> Args {
    let mut args = std::env::args().skip(1);
    let mut url: Option<String> = None;
    let mut runs = DEFAULT_RUNS;
    let mut cookies: Option<String> = None;
    let mut cookies_from_browser: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runs" => {
                if let Some(n) = args.next() {
                    runs = n.parse().unwrap_or(3);
                }
            }
            "--cookies" => {
                if let Some(v) = args.next() {
                    cookies = Some(v);
                }
            }
            "--cookies-from-browser" => {
                if let Some(v) = args.next() {
                    cookies_from_browser = Some(v);
                }
            }
            s if s.starts_with("http") => url = Some(s.to_string()),
            _ => {}
        }
    }

    let url = url.unwrap_or_else(|| {
        eprintln!(
            "{} {} <URL> [--runs N] [--cookies <file>] [--cookies-from-browser <browser>]",
            style("Usage:").bold(),
            style("compare").cyan()
        );
        std::process::exit(1);
    });

    Args {
        url,
        runs,
        cookies,
        cookies_from_browser,
    }
}

fn avg_duration(samples: &[Duration]) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    samples.iter().sum::<Duration>() / samples.len() as u32
}

fn fmt_secs(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 0.001 {
        format!("{:.1}ms", s * 1000.0)
    } else if s < 1.0 {
        format!("{:.0}ms", s * 1000.0)
    } else if s < 10.0 {
        format!("{:.2}s", s)
    } else {
        format!("{:.1}s", s)
    }
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("  {spinner:.cyan} {msg}")
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"])
}

fn progress_style() -> ProgressStyle {
    ProgressStyle::with_template("  {spinner:.cyan} {msg} {bar:20.cyan/dim} {pos}/{len}")
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✓"])
        .progress_chars("━━╸")
}

#[derive(Debug, Clone)]
enum ScenarioKind {
    /// Audio-only stream.
    Audio(AudioQuality),
    /// Video-only stream (no audio).
    Video(VideoQuality),
    /// Native pre-muxed format (AudioVideo) with max height.
    NativeMuxed(u32),
    /// Separate video + audio, combined by ffmpeg.
    Muxed(VideoQuality),
}

#[derive(Debug, Clone)]
struct Scenario {
    label: &'static str,
    yt_dlp_format: &'static str,
    extra_args: Vec<&'static str>,
    kind: ScenarioKind,
    output_ext: &'static str,
}

#[derive(Debug)]
struct Section {
    title: &'static str,
    emoji: &'static str,
    scenarios: Vec<Scenario>,
}

fn build_sections() -> Vec<Section> {
    vec![
        Section {
            title: "Audio streams",
            emoji: "🎵",
            scenarios: vec![
                Scenario {
                    label: "Audio 96 kbps (Low)",
                    yt_dlp_format: "bestaudio[abr<=96]",
                    extra_args: vec![],
                    kind: ScenarioKind::Audio(AudioQuality::Low),
                    output_ext: "m4a",
                },
                Scenario {
                    label: "Audio 128 kbps (Medium)",
                    yt_dlp_format: "bestaudio[abr<=128]",
                    extra_args: vec![],
                    kind: ScenarioKind::Audio(AudioQuality::Medium),
                    output_ext: "m4a",
                },
                Scenario {
                    label: "Audio 192 kbps (High)",
                    yt_dlp_format: "bestaudio[abr<=192]",
                    extra_args: vec![],
                    kind: ScenarioKind::Audio(AudioQuality::High),
                    output_ext: "m4a",
                },
                Scenario {
                    label: "Audio best quality",
                    yt_dlp_format: "bestaudio",
                    extra_args: vec![],
                    kind: ScenarioKind::Audio(AudioQuality::Best),
                    output_ext: "m4a",
                },
            ],
        },
        Section {
            title: "Video streams (no audio)",
            emoji: "🎬",
            scenarios: vec![
                Scenario {
                    label: "Video 480p",
                    yt_dlp_format: "bestvideo[height<=480]",
                    extra_args: vec![],
                    kind: ScenarioKind::Video(VideoQuality::Low),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Video 720p",
                    yt_dlp_format: "bestvideo[height<=720]",
                    extra_args: vec![],
                    kind: ScenarioKind::Video(VideoQuality::Medium),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Video 1080p",
                    yt_dlp_format: "bestvideo[height<=1080]",
                    extra_args: vec![],
                    kind: ScenarioKind::Video(VideoQuality::High),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Video best quality",
                    yt_dlp_format: "bestvideo",
                    extra_args: vec![],
                    kind: ScenarioKind::Video(VideoQuality::Best),
                    output_ext: "mp4",
                },
            ],
        },
        Section {
            title: "Muxed streams — native (YouTube pre-muxed, no ffmpeg)",
            emoji: "📦",
            scenarios: vec![
                Scenario {
                    label: "Native 360p (mp4)",
                    yt_dlp_format: "best[height<=360][ext=mp4]",
                    extra_args: vec![],
                    kind: ScenarioKind::NativeMuxed(360),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Native 720p (mp4)",
                    yt_dlp_format: "best[height<=720][ext=mp4]",
                    extra_args: vec![],
                    kind: ScenarioKind::NativeMuxed(720),
                    output_ext: "mp4",
                },
            ],
        },
        Section {
            title: "Muxed streams — combined by ffmpeg",
            emoji: "📦",
            scenarios: vec![
                Scenario {
                    label: "Muxed 480p",
                    yt_dlp_format: "bestvideo[height<=480]+bestaudio",
                    extra_args: vec!["--merge-output-format", "mp4"],
                    kind: ScenarioKind::Muxed(VideoQuality::Low),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Muxed 720p",
                    yt_dlp_format: "bestvideo[height<=720]+bestaudio",
                    extra_args: vec!["--merge-output-format", "mp4"],
                    kind: ScenarioKind::Muxed(VideoQuality::Medium),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Muxed 1080p",
                    yt_dlp_format: "bestvideo[height<=1080]+bestaudio",
                    extra_args: vec!["--merge-output-format", "mp4"],
                    kind: ScenarioKind::Muxed(VideoQuality::High),
                    output_ext: "mp4",
                },
                Scenario {
                    label: "Muxed best quality",
                    yt_dlp_format: "bestvideo+bestaudio",
                    extra_args: vec!["--merge-output-format", "mp4"],
                    kind: ScenarioKind::Muxed(VideoQuality::Best),
                    output_ext: "mp4",
                },
            ],
        },
    ]
}

struct RowResult {
    label: String,
    raw_avg: Duration,
    conservative_avg: Duration,
    balanced_avg: Duration,
    aggressive_avg: Duration,
}

impl RowResult {
    fn best_library_avg(&self) -> Duration {
        self.conservative_avg.min(self.balanced_avg).min(self.aggressive_avg)
    }

    fn speedup_pct(&self) -> f64 {
        let raw = self.raw_avg.as_secs_f64();
        let best = self.best_library_avg().as_secs_f64();
        if raw <= 0.0 {
            return 0.0;
        }
        ((raw - best) / raw) * 100.0
    }
}

struct RawDownloadArgs<'a> {
    yt_dlp_bin: &'a Path,
    info_json_path: &'a Path,
    format_selector: &'a str,
    extra_args: &'a [&'a str],
    output_dir: &'a Path,
    file_prefix: &'a str,
    ext: &'a str,
    runs: usize,
    pb: &'a ProgressBar,
}

async fn raw_download(args: RawDownloadArgs<'_>) -> Vec<Duration> {
    let mut samples = Vec::with_capacity(args.runs);
    for i in 0..args.runs {
        args.pb.set_message(format!(
            "🔧 {} (run {}/{})",
            style("yt-dlp raw").dim(),
            i + 1,
            args.runs
        ));
        args.pb.set_position((i + 1) as u64);

        let out = args
            .output_dir
            .join(format!("{}-{}.{}", args.file_prefix, i, args.ext))
            .to_string_lossy()
            .into_owned();

        let mut cmd_args: Vec<String> = vec![
            "--load-info-json".to_string(),
            args.info_json_path.to_string_lossy().into_owned(),
            "-f".to_string(),
            args.format_selector.to_string(),
        ];
        cmd_args.extend(args.extra_args.iter().map(|s| s.to_string()));
        cmd_args.extend([
            "-o".to_string(),
            out.clone(),
            "--no-playlist".to_string(),
            "--quiet".to_string(),
        ]);

        let start = Instant::now();
        Executor::new(
            args.yt_dlp_bin,
            cmd_args,
            Duration::from_secs(RAW_DOWNLOAD_TIMEOUT_SECS),
        )
        .execute()
        .await
        .expect("raw yt-dlp download failed");
        let elapsed = start.elapsed();
        samples.push(elapsed);

        let _ = tokio::fs::remove_file(&out).await;
    }
    samples
}

struct LibDownloadArgs<'a> {
    downloader: &'a Downloader,
    scenario: &'a Scenario,
    video: &'a Video,
    output_dir: &'a Path,
    prefix: &'a str,
    runs: usize,
    profile_name: &'a str,
    pb: &'a ProgressBar,
}

async fn lib_download(args: LibDownloadArgs<'_>) -> Vec<Duration> {
    let mut samples = Vec::with_capacity(args.runs);

    for i in 0..args.runs {
        args.pb.set_message(format!(
            "📦 {} (run {}/{})",
            style(args.profile_name).dim(),
            i + 1,
            args.runs
        ));
        args.pb.inc(1);

        let out_name = format!("{}-{}.{}", args.prefix, i, args.scenario.output_ext);
        let out_path = args.output_dir.join(&out_name);

        let start = Instant::now();

        // Use pre-fetched video metadata + download_format to avoid internal re-fetch
        let result = match &args.scenario.kind {
            ScenarioKind::Audio(quality) => match args.video.select_audio_format(*quality, AudioCodecPreference::Any) {
                Some(f) => args.downloader.download_format(f, &out_name).await,
                None => {
                    args.pb.println(format!(
                        "  {} Skipped {} — no matching audio format",
                        style("⚠").yellow(),
                        args.scenario.label
                    ));
                    continue;
                }
            },
            ScenarioKind::Video(quality) => match args.video.select_video_format(*quality, VideoCodecPreference::Any) {
                Some(f) => args.downloader.download_format(f, &out_name).await,
                None => {
                    args.pb.println(format!(
                        "  {} Skipped {} — no matching video format",
                        style("⚠").yellow(),
                        args.scenario.label
                    ));
                    continue;
                }
            },
            ScenarioKind::NativeMuxed(max_height) => {
                // Find a pre-muxed AudioVideo format with height <= max_height
                let format = args
                    .video
                    .formats
                    .iter()
                    .filter(|f| f.format_type() == FormatType::AudioVideo)
                    .filter(|f| f.video_resolution.height.is_some_and(|h| h <= *max_height))
                    .max_by_key(|f| f.video_resolution.height.unwrap_or(0));

                match format {
                    Some(f) => args.downloader.download_format(f, &out_name).await,
                    None => {
                        args.pb.println(format!(
                            "  {} Skipped {} — no native muxed format ≤{}p",
                            style("⚠").yellow(),
                            args.scenario.label,
                            max_height
                        ));
                        continue;
                    }
                }
            }
            ScenarioKind::Muxed(quality) => {
                args.downloader
                    .download_video_with_quality(
                        args.video,
                        &out_name,
                        *quality,
                        VideoCodecPreference::Any,
                        AudioQuality::Best,
                        AudioCodecPreference::Any,
                    )
                    .await
            }
        };

        match result {
            Ok(_) => samples.push(start.elapsed()),
            Err(e) => {
                args.pb.println(format!(
                    "  {} {} failed on run {}: {}",
                    style("⚠").yellow(),
                    args.scenario.label,
                    i + 1,
                    e
                ));
            }
        }

        let _ = tokio::fs::remove_file(&out_path).await;
    }

    samples
}

async fn build_downloader_with_profile(
    libs_dir: &Path,
    output_dir: &Path,
    profile: SpeedProfile,
    cookies: Option<&str>,
    cookies_from_browser: Option<&str>,
) -> Downloader {
    let mut builder = Downloader::with_new_binaries(libs_dir, output_dir)
        .await
        .expect("downloader setup failed")
        .with_speed_profile(profile);
    if let Some(c) = cookies {
        builder = builder.with_cookies(c);
    }
    if let Some(b) = cookies_from_browser {
        builder = builder.with_cookies_from_browser(b);
    }
    builder.build().await.expect("downloader build failed")
}

fn print_header(url: &str, runs: usize, video_title: &str) {
    let term_width = Term::stdout().size().1 as usize;
    let width = term_width.min(70);
    let line = "─".repeat(width);

    println!();
    println!("  {}", style(&line).cyan());
    println!(
        "  {}  {}",
        style("⚡").bold(),
        style("Performance Comparison - yt-dlp vs library").bold().cyan()
    );
    println!("  {}", style(&line).cyan());
    println!();
    println!("  {}  {}", style("📺").bold(), style(video_title).white().bold());
    println!("  {}  {}", style("🔗").bold(), style(url).dim());
    println!(
        "  {}  {} runs per scenario",
        style("🔄").bold(),
        style(runs).yellow().bold()
    );
    println!();
}

fn print_section_header(emoji: &str, title: &str) {
    println!();
    println!("  {} {}", emoji, style(title).bold().underlined());
    println!();
}

fn print_scenario_result(label: &str, raw_avg: Duration, cons: Duration, bal: Duration, agg: Duration) {
    let raw_s = fmt_secs(raw_avg);
    let best = raw_avg.min(cons).min(bal).min(agg);
    let speedup = if raw_avg > Duration::ZERO {
        ((raw_avg.as_secs_f64() - best.as_secs_f64()) / raw_avg.as_secs_f64()) * 100.0
    } else {
        0.0
    };

    let speedup_str = if speedup > 0.0 {
        format!("{}% faster", style(format!("{:.0}", speedup)).green().bold())
    } else {
        style("baseline").dim().to_string()
    };

    println!(
        "  {} {:<30} raw: {:<8} cons: {:<8} bal: {:<8} agg: {:<8} {}",
        style("✅").green(),
        style(label).white(),
        style(&raw_s).yellow(),
        style(fmt_secs(cons)).dim(),
        style(fmt_secs(bal)).cyan(),
        style(fmt_secs(agg)).magenta(),
        speedup_str,
    );
}

fn print_styled_table(section: &Section, rows: &[RowResult]) {
    let term_width = Term::stdout().size().1 as usize;
    let width = term_width.min(90);
    let sep = "─".repeat(width);

    println!();
    println!("  {}", style(&sep).dim());
    println!("  {} {} — Results", section.emoji, style(section.title).bold());
    println!("  {}", style(&sep).dim());

    // Column headers
    println!(
        "  {:<32} {:>8} {:>14} {:>14} {:>14}",
        style("Scenario").underlined(),
        style("yt-dlp").underlined().yellow(),
        style("Conservative").underlined().dim(),
        style("Balanced").underlined().cyan(),
        style("Aggressive").underlined().magenta(),
    );

    for row in rows {
        let speedup = row.speedup_pct();
        let speedup_str = if speedup > 5.0 {
            style(format!(" ↑{:.0}%", speedup)).green().to_string()
        } else {
            String::new()
        };

        println!(
            "  {:<32} {:>8} {:>14} {:>14} {:>14}{}",
            row.label,
            style(fmt_secs(row.raw_avg)).yellow(),
            style(fmt_secs(row.conservative_avg)).dim(),
            style(fmt_secs(row.balanced_avg)).cyan().bold(),
            style(fmt_secs(row.aggressive_avg)).magenta(),
            speedup_str,
        );
    }
    println!("  {}", style(&sep).dim());
}

fn print_markdown_tables(sections: &[(&Section, Vec<RowResult>)]) {
    println!();
    println!(
        "  {}",
        style("📋 Markdown tables (copy-paste into README)").bold().green()
    );
    let term_width = Term::stdout().size().1 as usize;
    let sep = "─".repeat(term_width.min(70));
    println!("  {}", style(&sep).dim());
    println!();

    for (section, rows) in sections {
        println!("### {} {}", section.emoji, section.title);
        println!();
        println!("| Scenario | `yt-dlp` | Conservative | Balanced *(default)* | Aggressive |");
        println!("|---|---|---|---|---|");
        for row in rows {
            println!(
                "| {} | {} | {} | {} | {} |",
                row.label,
                fmt_secs(row.raw_avg),
                fmt_secs(row.conservative_avg),
                fmt_secs(row.balanced_avg),
                fmt_secs(row.aggressive_avg),
            );
        }
        println!();
    }
}

fn print_summary(all_rows: &[&RowResult], total_elapsed: Duration) {
    let term_width = Term::stdout().size().1 as usize;
    let sep = "─".repeat(term_width.min(70));

    println!();
    println!("  {}", style(&sep).cyan());
    println!(
        "  {}  {}",
        style("🏁").bold(),
        style("Benchmark Complete!").bold().cyan()
    );
    println!("  {}", style(&sep).cyan());
    println!();

    let total_scenarios = all_rows.len();
    let avg_speedup: f64 = all_rows.iter().map(|r| r.speedup_pct()).sum::<f64>() / total_scenarios as f64;
    let best_speedup = all_rows.iter().map(|r| r.speedup_pct()).fold(0.0_f64, f64::max);

    println!(
        "  {} {} scenarios benchmarked in {}",
        style("📊").bold(),
        style(total_scenarios).yellow().bold(),
        style(fmt_secs(total_elapsed)).cyan().bold()
    );
    println!(
        "  {} Average speedup: {}",
        style("⚡").bold(),
        style(format!("{:.0}%", avg_speedup)).green().bold()
    );
    println!(
        "  {} Best speedup: {}",
        style("🚀").bold(),
        style(format!("{:.0}%", best_speedup)).green().bold()
    );
    println!();
}

#[tokio::main]
async fn main() {
    let args = parse_args();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    let libs_dir = PathBuf::from("profiling-libs");
    let output_dir = PathBuf::from("profiling-output");

    tokio::fs::create_dir_all(&libs_dir)
        .await
        .expect("create libs dir failed");
    tokio::fs::create_dir_all(&output_dir)
        .await
        .expect("create output dir failed");

    let yt_dlp_bin = libs_dir.join(if cfg!(target_os = "windows") {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    });

    let mp = MultiProgress::new();

    let setup_spinner = mp.add(ProgressBar::new_spinner());
    setup_spinner.set_style(spinner_style());
    setup_spinner.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MILLIS));
    setup_spinner.set_message("🔧 Setting up downloaders...");

    let dl_conservative = build_downloader_with_profile(
        &libs_dir,
        &output_dir,
        SpeedProfile::Conservative,
        args.cookies.as_deref(),
        args.cookies_from_browser.as_deref(),
    )
    .await;
    let dl_balanced = build_downloader_with_profile(
        &libs_dir,
        &output_dir,
        SpeedProfile::Balanced,
        args.cookies.as_deref(),
        args.cookies_from_browser.as_deref(),
    )
    .await;
    let dl_aggressive = build_downloader_with_profile(
        &libs_dir,
        &output_dir,
        SpeedProfile::Aggressive,
        args.cookies.as_deref(),
        args.cookies_from_browser.as_deref(),
    )
    .await;

    setup_spinner.finish_and_clear();
    println!(
        "  {} Downloaders ready (Conservative, Balanced, Aggressive)",
        style("✅").green()
    );

    let meta_spinner = mp.add(ProgressBar::new_spinner());
    meta_spinner.set_style(spinner_style());
    meta_spinner.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MILLIS));
    meta_spinner.set_message("📡 Fetching video metadata...");

    let video = dl_balanced
        .fetch_video_infos(&args.url)
        .await
        .expect("failed to fetch video metadata");

    meta_spinner.set_message("📡 Fetching video metadata with yt-dlp...");

    let info_json_path = output_dir.join("info.json");
    let mut dump_args = vec![
        args.url.clone(),
        "--dump-single-json".to_string(),
        "--no-playlist".to_string(),
    ];
    if let Some(ref c) = args.cookies {
        dump_args.push(format!("--cookies={}", c));
    }
    if let Some(ref b) = args.cookies_from_browser {
        dump_args.push(format!("--cookies-from-browser={}", b));
    }
    let dump_output = Executor::new(&yt_dlp_bin, dump_args, Duration::from_secs(INFO_JSON_TIMEOUT_SECS))
        .execute()
        .await
        .expect("failed to dump json with yt-dlp");
    tokio::fs::write(&info_json_path, dump_output.stdout)
        .await
        .expect("failed to write info.json");

    meta_spinner.finish_and_clear();
    println!("  {} Metadata fetched: \"{}\"", style("✅").green(), &video.title);

    print_header(&args.url, args.runs, &video.title);

    let sections = build_sections();
    let total_scenarios: usize = sections.iter().map(|s| s.scenarios.len()).sum();
    let total_measurements = total_scenarios * PROFILES_PER_SCENARIO;
    let mut global_idx = 0usize;
    let mut all_section_results: Vec<(&Section, Vec<RowResult>)> = Vec::new();
    let global_start = Instant::now();

    let overall_pb = mp.add(ProgressBar::new(total_measurements as u64));
    overall_pb.set_style(
        ProgressStyle::with_template(
            "  {spinner:.cyan} Overall {bar:30.green/dim} {pos}/{len} measurements ({eta} remaining)",
        )
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "🏁"])
        .progress_chars("━━╸"),
    );
    overall_pb.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MILLIS));

    for section in &sections {
        print_section_header(section.emoji, section.title);

        let mut rows: Vec<RowResult> = Vec::new();

        for scenario in &section.scenarios {
            global_idx += 1;
            let slug = scenario.label.to_lowercase().replace(' ', "-").replace(['(', ')'], "");

            // Per-scenario progress bar (PROFILES_PER_SCENARIO measurements × runs)
            let total_runs = (args.runs * PROFILES_PER_SCENARIO) as u64;
            let scenario_pb = mp.add(ProgressBar::new(total_runs));
            scenario_pb.set_style(progress_style());
            scenario_pb.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MILLIS));
            scenario_pb.set_message(format!("⏳ [{}/{}] {}", global_idx, total_scenarios, scenario.label));

            // 1) Raw yt-dlp
            let raw_samples = raw_download(RawDownloadArgs {
                yt_dlp_bin: &yt_dlp_bin,
                info_json_path: &info_json_path,
                format_selector: scenario.yt_dlp_format,
                extra_args: &scenario.extra_args,
                output_dir: &output_dir,
                file_prefix: &format!("raw-{}", slug),
                ext: scenario.output_ext,
                runs: args.runs,
                pb: &scenario_pb,
            })
            .await;
            let raw_avg = avg_duration(&raw_samples);
            overall_pb.inc(1);

            // 2) Conservative
            let conservative_samples = lib_download(LibDownloadArgs {
                downloader: &dl_conservative,
                scenario,
                video: &video,
                output_dir: &output_dir,
                prefix: &format!("conservative-{}", slug),
                runs: args.runs,
                profile_name: "Conservative",
                pb: &scenario_pb,
            })
            .await;
            let conservative_avg = avg_duration(&conservative_samples);
            overall_pb.inc(1);

            // 3) Balanced
            let balanced_samples = lib_download(LibDownloadArgs {
                downloader: &dl_balanced,
                scenario,
                video: &video,
                output_dir: &output_dir,
                prefix: &format!("balanced-{}", slug),
                runs: args.runs,
                profile_name: "Balanced",
                pb: &scenario_pb,
            })
            .await;
            let balanced_avg = avg_duration(&balanced_samples);
            overall_pb.inc(1);

            // 4) Aggressive
            let aggressive_samples = lib_download(LibDownloadArgs {
                downloader: &dl_aggressive,
                scenario,
                video: &video,
                output_dir: &output_dir,
                prefix: &format!("aggressive-{}", slug),
                runs: args.runs,
                profile_name: "Aggressive",
                pb: &scenario_pb,
            })
            .await;
            let aggressive_avg = avg_duration(&aggressive_samples);
            overall_pb.inc(1);

            scenario_pb.finish_and_clear();

            // Print inline result
            print_scenario_result(scenario.label, raw_avg, conservative_avg, balanced_avg, aggressive_avg);

            rows.push(RowResult {
                label: scenario.label.to_string(),
                raw_avg,
                conservative_avg,
                balanced_avg,
                aggressive_avg,
            });
        }

        // Print styled table for this section
        print_styled_table(section, &rows);
        all_section_results.push((section, rows));
    }

    overall_pb.finish_and_clear();

    let all_rows: Vec<&RowResult> = all_section_results.iter().flat_map(|(_, rows)| rows.iter()).collect();

    print_summary(&all_rows, global_start.elapsed());

    print_markdown_tables(&all_section_results);

    let _ = tokio::fs::remove_file(&info_json_path).await;

    println!(
        "  {} Copy the tables above into the README's {} section.",
        style("💡").bold(),
        style("## 🏎️ Performances").cyan().bold()
    );
    println!();
}
