//! Criterion micro-benchmarks for pure-Rust, non-network code paths.
//!
//! Run all benchmarks:
//! ```bash
//! cargo bench
//! # HTML reports are written to target/criterion/
//! ```
//!
//! Run a specific group:
//! ```bash
//! cargo bench -- format_selection
//! ```

use std::path::PathBuf;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use yt_dlp::VideoSelection;
use yt_dlp::download::manager::ManagerConfig;
use yt_dlp::download::{AudioCodec, PostProcessConfig, SpeedProfile, VideoCodec};
use yt_dlp::events::{DownloadEvent, EventFilter};
use yt_dlp::model::chapter::Chapter;
use yt_dlp::model::heatmap::{Heatmap, HeatmapPoint};
use yt_dlp::model::playlist::{Playlist, PlaylistEntry};
use yt_dlp::model::selector::{AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality};
use yt_dlp::model::{ChapterList, Video};
use yt_dlp::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

fn make_chapters(n: usize) -> Vec<Chapter> {
    (0..n)
        .map(|i| Chapter {
            start_time: (i as f64) * 60.0,
            end_time: (i as f64) * 60.0 + 60.0,
            title: Some(format!("Chapter {}", i + 1)),
        })
        .collect()
}

fn make_heatmap(n: usize) -> Heatmap {
    let total_duration = 600.0_f64;
    let step = total_duration / n as f64;
    let points = (0..n)
        .map(|i| {
            let start = i as f64 * step;
            HeatmapPoint {
                start_time: start,
                end_time: start + step,
                value: (i as f64 / n as f64).min(1.0),
            }
        })
        .collect();
    Heatmap::new(points)
}

fn make_playlist(n: usize) -> Playlist {
    let entries = (0..n)
        .map(|i| PlaylistEntry {
            id: format!("vid{}", i),
            title: format!("Video {}", i),
            url: format!("https://www.youtube.com/watch?v=vid{}", i),
            index: Some(i),
            duration: Some(300.0),
            thumbnail: None,
            uploader: Some("uploader_a".to_string()),
            channel_id: Some("channel_a".to_string()),
            availability: Some("public".to_string()),
        })
        .collect();

    Playlist {
        id: "PL_test".to_string(),
        title: "Test Playlist".to_string(),
        description: None,
        uploader: Some("uploader_a".to_string()),
        uploader_id: None,
        uploader_url: None,
        entries,
        video_count: Some(n),
        url: None,
    }
}

fn bench_format_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_selection");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let video = rt.block_on(async {
        let libraries = yt_dlp::client::deps::Libraries::new("libs/yt-dlp".into(), "libs/ffmpeg".into());
        let downloader = yt_dlp::Downloader::builder(libraries, "output")
            .build()
            .await
            .expect("failed to build downloader");
        downloader
            .fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo")
            .await
            .expect("failed to fetch video")
    });

    // Test on the real video instead of synthetic loop
    let n_formats = video.formats.len();

    group.bench_with_input(
        BenchmarkId::new("best_video_format", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.best_video_format()),
    );

    group.bench_with_input(
        BenchmarkId::new("best_audio_format", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.best_audio_format()),
    );

    group.bench_with_input(
        BenchmarkId::new("worst_video_format", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.worst_video_format()),
    );

    group.bench_with_input(
        BenchmarkId::new("worst_audio_format", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.worst_audio_format()),
    );

    group.bench_with_input(
        BenchmarkId::new("select_video_High_AVC1", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.select_video_format(VideoQuality::High, VideoCodecPreference::AVC1)),
    );

    group.bench_with_input(
        BenchmarkId::new("select_audio_Best_Opus", n_formats),
        &video,
        |b, v: &Video| b.iter(|| v.select_audio_format(AudioQuality::Best, AudioCodecPreference::Opus)),
    );

    group.finish();
}

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    let valid_yt = "https://www.youtube.com/watch?v=gXtp6C-3JKo";
    let valid_short = "https://youtu.be/jNQXAC9IVRw";
    let non_yt = "https://vimeo.com/12345";
    let invalid = "not-a-url";

    group.bench_function("validate_youtube_url/valid_youtube", |b| {
        b.iter(|| validate_youtube_url(valid_yt))
    });
    group.bench_function("validate_youtube_url/valid_youtu_be", |b| {
        b.iter(|| validate_youtube_url(valid_short))
    });
    group.bench_function("validate_youtube_url/non_youtube", |b| {
        b.iter(|| validate_youtube_url(non_yt))
    });
    group.bench_function("validate_youtube_url/invalid", |b| {
        b.iter(|| validate_youtube_url(invalid))
    });

    group.bench_function("sanitize_filename/normal", |b| {
        b.iter(|| sanitize_filename("my-video.mp4"))
    });
    group.bench_function("sanitize_filename/special_chars", |b| {
        b.iter(|| sanitize_filename("My Video: Great/Stuff\\file.mp4"))
    });
    group.bench_function("sanitize_filename/unicode", |b| {
        b.iter(|| sanitize_filename("Vidéo été été 🎬.mp4"))
    });

    group.bench_function("sanitize_path/simple", |b| {
        b.iter(|| sanitize_path("downloads/my-video.mp4"))
    });
    group.bench_function("sanitize_path/traversal", |b| {
        b.iter(|| sanitize_path("../../../etc/passwd"))
    });

    group.finish();
}

fn bench_model_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_operations");

    // Fetch real video metadata for benchmarking
    let rt = tokio::runtime::Runtime::new().unwrap();
    let video = rt.block_on(async {
        let libraries = yt_dlp::client::deps::Libraries::new("libs/yt-dlp".into(), "libs/ffmpeg".into());
        let downloader = yt_dlp::Downloader::builder(libraries, "output")
            .build()
            .await
            .expect("failed to build downloader");
        downloader
            .fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo")
            .await
            .expect("failed to fetch video")
    });

    group.bench_function("has_chapters", |b| b.iter(|| video.has_chapters()));
    group.bench_function("get_chapters", |b| b.iter(|| video.get_chapters()));
    group.bench_function("has_heatmap", |b| b.iter(|| video.has_heatmap()));

    group.bench_function("has_subtitle_language_en", |b| {
        b.iter(|| video.subtitles.contains_key("en"))
    });

    // JSON serialization / deserialization
    let json = serde_json::to_string(&video).expect("serialize failed");
    group.bench_function("serialize_to_json", |b| {
        b.iter(|| serde_json::to_string(&video).unwrap())
    });
    group.bench_function("deserialize_from_json", |b| {
        b.iter(|| serde_json::from_str::<Video>(&json).unwrap())
    });

    group.finish();
}

fn bench_config_builders(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_builders");

    group.bench_function("ManagerConfig_default", |b| b.iter(ManagerConfig::default));

    group.bench_function("ManagerConfig_builder_max5", |b| {
        b.iter(|| ManagerConfig::builder().max_concurrent_downloads(5).build())
    });

    group.bench_function("PostProcessConfig_H264_AAC", |b| {
        b.iter(|| {
            PostProcessConfig::new()
                .with_video_codec(VideoCodec::H264)
                .with_audio_codec(AudioCodec::AAC)
        })
    });

    group.finish();
}

fn bench_chapter_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("chapter_ops");

    for &n in &[5usize, 20, 50, 100] {
        let chapters = make_chapters(n);
        let mid_time = (n as f64 / 2.0) * 60.0 + 30.0;
        let single = &chapters[0];

        group.bench_with_input(BenchmarkId::new("find_by_timestamp", n), &chapters, |b, ch| {
            let list = ChapterList::new(ch);
            b.iter(|| list.find_by_timestamp(mid_time))
        });

        group.bench_with_input(BenchmarkId::new("search_by_title", n), &chapters, |b, ch| {
            let list = ChapterList::new(ch);
            b.iter(|| list.search_by_title("Chapter"))
        });

        group.bench_with_input(BenchmarkId::new("contains_timestamp", n), single, |b, ch| {
            b.iter(|| ch.contains_timestamp(30.0))
        });

        group.bench_with_input(BenchmarkId::new("validate", n), &chapters, |b, ch| {
            let list = ChapterList::new(ch);
            b.iter(|| list.validate())
        });
    }

    group.finish();
}

fn bench_heatmap_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("heatmap_ops");

    for &n in &[10usize, 100, 1_000] {
        let heatmap = make_heatmap(n);

        group.bench_with_input(
            BenchmarkId::new("most_engaged_segment", n),
            &heatmap,
            |b, h: &Heatmap| b.iter(|| h.most_engaged_segment()),
        );

        group.bench_with_input(
            BenchmarkId::new("get_highly_engaged_segments_0_7", n),
            &heatmap,
            |b, h: &Heatmap| b.iter(|| h.get_highly_engaged_segments(0.7)),
        );

        group.bench_with_input(
            BenchmarkId::new("get_point_at_time_42", n),
            &heatmap,
            |b, h: &Heatmap| b.iter(|| h.get_point_at_time(42.0)),
        );
    }

    group.finish();
}

fn bench_playlist_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("playlist_ops");

    for &n in &[10usize, 50, 200] {
        let playlist = make_playlist(n);

        group.bench_with_input(
            BenchmarkId::new("available_entries", n),
            &playlist,
            |b, p: &Playlist| b.iter(|| p.available_entries()),
        );

        group.bench_with_input(
            BenchmarkId::new("search_entries_by_title", n),
            &playlist,
            |b, p: &Playlist| b.iter(|| p.search_entries_by_title("Video")),
        );

        group.bench_with_input(
            BenchmarkId::new("filter_by_uploader", n),
            &playlist,
            |b, p: &Playlist| b.iter(|| p.filter_by_uploader("uploader_a")),
        );
    }

    group.finish();
}

fn bench_format_type(c: &mut Criterion) {
    use yt_dlp::model::format::{
        CodecInfo, DownloadInfo, Extension, FileInfo, Format, HttpHeaders, QualityInfo, RatesInfo, StoryboardInfo,
        VideoResolution,
    };

    let mut group = c.benchmark_group("format_type");

    let base_format = || Format {
        format: "test".to_string(),
        format_id: "test".to_string(),
        format_note: None,
        protocol: Default::default(),
        language: None,
        has_drm: None,
        container: None,
        available_at: None,
        language_preference: None,
        source_preference: None,
        codec_info: CodecInfo {
            audio_codec: None,
            video_codec: None,
            audio_ext: Extension::None,
            video_ext: Extension::None,
            audio_channels: None,
            asr: None,
        },
        video_resolution: VideoResolution {
            width: None,
            height: None,
            resolution: None,
            fps: None,
            aspect_ratio: None,
        },
        download_info: DownloadInfo {
            url: None,
            ext: Extension::None,
            http_headers: HttpHeaders {
                user_agent: String::new(),
                accept: String::new(),
                accept_language: String::new(),
                sec_fetch_mode: String::new(),
            },
            manifest_url: None,
            downloader_options: None,
        },
        quality_info: QualityInfo {
            quality: None,
            dynamic_range: None,
        },
        file_info: FileInfo {
            filesize_approx: None,
            filesize: None,
        },
        storyboard_info: StoryboardInfo {
            rows: None,
            columns: None,
            fragments: None,
        },
        rates_info: RatesInfo {
            video_rate: None,
            audio_rate: None,
            total_rate: None,
        },
        video_id: None,
    };

    // video-only
    let mut video_only = base_format();
    video_only.codec_info.video_codec = Some("vp9".to_string());

    // audio-only
    let mut audio_only = base_format();
    audio_only.codec_info.audio_codec = Some("opus".to_string());

    // muxed
    let mut muxed = base_format();
    muxed.codec_info.video_codec = Some("avc1".to_string());
    muxed.codec_info.audio_codec = Some("mp4a.40.2".to_string());

    // manifest
    let mut manifest = base_format();
    manifest.download_info.manifest_url = Some("https://example.com/manifest.m3u8".to_string());

    group.bench_function("format_type_video", |b| b.iter(|| video_only.format_type()));
    group.bench_function("format_type_audio", |b| b.iter(|| audio_only.format_type()));
    group.bench_function("format_type_muxed", |b| b.iter(|| muxed.format_type()));
    group.bench_function("format_type_manifest", |b| b.iter(|| manifest.format_type()));
    group.bench_function("is_video", |b| b.iter(|| video_only.is_video()));
    group.bench_function("is_audio", |b| b.iter(|| audio_only.is_audio()));

    group.finish();
}

fn bench_event_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_filter");

    let terminal_event = DownloadEvent::DownloadCompleted {
        download_id: 1,
        url: "https://www.youtube.com/watch?v=test".to_string(),
        output_path: PathBuf::from("output.mp4"),
        duration: std::time::Duration::from_secs(5),
        total_bytes: 1024 * 1024,
    };
    let progress_event = DownloadEvent::DownloadProgress {
        download_id: 1,
        downloaded_bytes: 512 * 1024,
        total_bytes: 1024 * 1024,
        speed_bytes_per_sec: 1_000_000.0,
        eta_seconds: Some(1),
    };

    group.bench_function("event_filter_all_matches", |b| {
        let filter = EventFilter::all();
        b.iter(|| filter.matches(&terminal_event))
    });

    group.bench_function("event_filter_only_terminal_match", |b| {
        let filter = EventFilter::only_terminal();
        b.iter(|| filter.matches(&terminal_event))
    });

    group.bench_function("event_filter_only_terminal_no_match", |b| {
        let filter = EventFilter::only_terminal();
        b.iter(|| filter.matches(&progress_event))
    });

    group.finish();
}

fn bench_speed_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("speed_profile");

    let profiles = [
        ("Conservative", SpeedProfile::Conservative),
        ("Balanced", SpeedProfile::Balanced),
        ("Aggressive", SpeedProfile::Aggressive),
    ];
    let file_sizes: &[u64] = &[1_000_000, 50_000_000, 500_000_000, 2_000_000_000];

    for (profile_name, profile) in &profiles {
        for &size in file_sizes {
            let segment_size = profile.segment_size() as u64;
            group.bench_with_input(
                BenchmarkId::new(format!("{profile_name}/calculate_optimal_segments"), size),
                &size,
                |b, &sz| b.iter(|| profile.calculate_optimal_segments(sz, segment_size)),
            );
        }
    }

    group.finish();
}

#[cfg(feature = "webhooks")]
fn bench_retry_strategy(c: &mut Criterion) {
    use yt_dlp::events::RetryStrategy;

    let mut group = c.benchmark_group("retry_strategy");
    let strategy = RetryStrategy::default();

    for attempt in 0usize..=5 {
        group.bench_with_input(BenchmarkId::new("delay_for_attempt", attempt), &attempt, |b, &a| {
            b.iter(|| strategy.delay_for_attempt(a))
        });
    }

    group.bench_function("should_retry_true", |b| b.iter(|| strategy.should_retry(0)));
    group.bench_function("should_retry_false", |b| b.iter(|| strategy.should_retry(10)));

    group.finish();
}

#[cfg(cache)]
fn bench_cache_ops(c: &mut Criterion) {
    use yt_dlp::cache::VideoCache;

    let mut group = c.benchmark_group("cache_ops");

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime failed");
    let video = rt.block_on(async {
        let libraries = yt_dlp::client::deps::Libraries::new("libs/yt-dlp".into(), "libs/ffmpeg".into());
        let downloader = yt_dlp::Downloader::builder(libraries, "output")
            .build()
            .await
            .expect("failed to build downloader");
        downloader
            .fetch_video_infos("https://www.youtube.com/watch?v=gXtp6C-3JKo")
            .await
            .expect("failed to fetch video")
    });

    group.bench_function("cache_put_single", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                let path = dir.path().to_path_buf();
                (dir, path, video.clone())
            },
            |(dir, path, v)| async move {
                let cache = VideoCache::new(path, None).await.expect("cache init failed");
                cache
                    .put(format!("https://example.com/{}", v.id), v)
                    .await
                    .expect("put failed");
                drop(dir);
            },
        );
    });

    group.bench_function("cache_get_hit", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                let path = dir.path().to_path_buf();
                (dir, path, video.clone())
            },
            |(dir, path, v)| async move {
                let cache = VideoCache::new(path, None).await.expect("cache init failed");
                let url = format!("https://example.com/{}", v.id);
                cache.put(url.clone(), v).await.expect("put failed");
                let _ = cache.get(&url).await.expect("get failed");
                drop(dir);
            },
        );
    });

    group.bench_function("cache_get_miss", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                dir.path().to_path_buf()
            },
            |path| async move {
                let cache = VideoCache::new(path, None).await.expect("cache init failed");
                let _ = cache.get("https://example.com/missing").await.expect("get failed");
            },
        );
    });

    group.bench_function("cache_put_500", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                (dir.path().to_path_buf(), video.clone())
            },
            |(path, base_video)| async move {
                let cache = VideoCache::new(path, None).await.expect("cache init failed");
                for i in 0..500usize {
                    let mut v = base_video.clone();
                    v.id = format!("bench-{}", i);
                    cache
                        .put(format!("https://example.com/bench-{}", i), v)
                        .await
                        .expect("put failed");
                }
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_format_selection,
    bench_validation,
    bench_model_ops,
    bench_config_builders,
    bench_chapter_ops,
    bench_heatmap_ops,
    bench_playlist_ops,
    bench_format_type,
    bench_event_filter,
    bench_speed_profile,
);

#[cfg(feature = "webhooks")]
criterion_group!(webhooks_benches, bench_retry_strategy);

#[cfg(cache)]
criterion_group!(cache_benches, bench_cache_ops);

#[cfg(all(feature = "webhooks", cache))]
criterion_main!(benches, webhooks_benches, cache_benches);

#[cfg(all(feature = "webhooks", not(cache)))]
criterion_main!(benches, webhooks_benches);

#[cfg(all(not(feature = "webhooks"), cache))]
criterion_main!(benches, cache_benches);

#[cfg(all(not(feature = "webhooks"), not(cache)))]
criterion_main!(benches);
