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

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ordered_float::OrderedFloat;
use std::path::PathBuf;
use yt_dlp::VideoSelection;
use yt_dlp::download::SpeedProfile;
use yt_dlp::download::manager::ManagerConfig;
use yt_dlp::download::postprocess::{AudioCodec, PostProcessConfig, VideoCodec};
use yt_dlp::events::{DownloadEvent, EventFilter};
use yt_dlp::model::chapter::Chapter;
use yt_dlp::model::format::{
    CodecInfo, DownloadInfo, Extension, FileInfo, Format, HttpHeaders, QualityInfo, RatesInfo,
    StoryboardInfo, VideoResolution,
};
use yt_dlp::model::heatmap::{Heatmap, HeatmapPoint};
use yt_dlp::model::playlist::{Playlist, PlaylistEntry};
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use yt_dlp::model::video::{ExtractorInfo, Version};
use yt_dlp::model::{ChapterList, Video};
use yt_dlp::utils::validation::{sanitize_filename, sanitize_path, validate_youtube_url};

fn make_format(
    id: &str,
    height: Option<u32>,
    vcodec: Option<&str>,
    acodec: Option<&str>,
    vbr: Option<f64>,
    abr: Option<f64>,
) -> Format {
    use yt_dlp::model::format::Protocol;

    Format {
        format: format!("{} - {}p", id, height.unwrap_or(0)),
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

    let audio_count = n_formats.saturating_sub(idx);
    for i in 0..audio_count {
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
        tags: vec!["zoo".to_string()],
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

    for &n_formats in &[10usize, 50, 100, 500] {
        let video = make_video(n_formats);

        group.bench_with_input(
            BenchmarkId::new("best_video_format", n_formats),
            &video,
            |b, v| b.iter(|| v.best_video_format()),
        );

        group.bench_with_input(
            BenchmarkId::new("best_audio_format", n_formats),
            &video,
            |b, v| b.iter(|| v.best_audio_format()),
        );

        group.bench_with_input(
            BenchmarkId::new("worst_video_format", n_formats),
            &video,
            |b, v| b.iter(|| v.worst_video_format()),
        );

        group.bench_with_input(
            BenchmarkId::new("worst_audio_format", n_formats),
            &video,
            |b, v| b.iter(|| v.worst_audio_format()),
        );

        group.bench_with_input(
            BenchmarkId::new("select_video_High_AVC1", n_formats),
            &video,
            |b, v| b.iter(|| v.select_video_format(VideoQuality::High, VideoCodecPreference::AVC1)),
        );

        group.bench_with_input(
            BenchmarkId::new("select_audio_Best_Opus", n_formats),
            &video,
            |b, v| b.iter(|| v.select_audio_format(AudioQuality::Best, AudioCodecPreference::Opus)),
        );
    }

    group.finish();
}

fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    let valid_yt = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
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
    let video = make_video(100);

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

        group.bench_with_input(
            BenchmarkId::new("find_by_timestamp", n),
            &chapters,
            |b, ch| {
                let list = ChapterList::new(ch);
                b.iter(|| list.find_by_timestamp(mid_time))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("search_by_title", n),
            &chapters,
            |b, ch| {
                let list = ChapterList::new(ch);
                b.iter(|| list.search_by_title("Chapter"))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("contains_timestamp", n),
            single,
            |b, ch| b.iter(|| ch.contains_timestamp(30.0)),
        );

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
            |b, h| b.iter(|| h.most_engaged_segment()),
        );

        group.bench_with_input(
            BenchmarkId::new("get_highly_engaged_segments_0_7", n),
            &heatmap,
            |b, h| b.iter(|| h.get_highly_engaged_segments(0.7)),
        );

        group.bench_with_input(
            BenchmarkId::new("get_point_at_time_42", n),
            &heatmap,
            |b, h| b.iter(|| h.get_point_at_time(42.0)),
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
            |b, p| b.iter(|| p.available_entries()),
        );

        group.bench_with_input(
            BenchmarkId::new("search_entries_by_title", n),
            &playlist,
            |b, p| b.iter(|| p.search_entries_by_title("Video")),
        );

        group.bench_with_input(
            BenchmarkId::new("filter_by_uploader", n),
            &playlist,
            |b, p| b.iter(|| p.filter_by_uploader("uploader_a")),
        );
    }

    group.finish();
}

fn bench_format_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_type");

    // video-only: video_codec set, no audio_codec, no manifest_url
    let video_only = make_format("v-only", Some(1080), Some("vp9"), None, Some(2000.0), None);
    // audio-only: audio_codec set, no video_codec
    let audio_only = make_format("a-only", None, None, Some("opus"), None, Some(128.0));
    // muxed: both codecs set
    let muxed = make_format(
        "muxed",
        Some(720),
        Some("avc1"),
        Some("mp4a.40.2"),
        Some(1500.0),
        Some(128.0),
    );
    // manifest: manifest_url set
    let mut manifest = make_format("manifest", None, None, None, None, None);
    manifest.download_info.manifest_url = Some("https://example.com/manifest.m3u8".to_string());

    group.bench_function("format_type_video", |b| b.iter(|| video_only.format_type()));
    group.bench_function("format_type_audio", |b| b.iter(|| audio_only.format_type()));
    group.bench_function("format_type_muxed", |b| b.iter(|| muxed.format_type()));
    group.bench_function("format_type_manifest", |b| {
        b.iter(|| manifest.format_type())
    });
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
        group.bench_with_input(
            BenchmarkId::new("delay_for_attempt", attempt),
            &attempt,
            |b, &a| b.iter(|| strategy.delay_for_attempt(a)),
        );
    }

    group.bench_function("should_retry_true", |b| b.iter(|| strategy.should_retry(0)));
    group.bench_function("should_retry_false", |b| {
        b.iter(|| strategy.should_retry(10))
    });

    group.finish();
}

#[cfg(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))]
fn bench_cache_ops(c: &mut Criterion) {
    use yt_dlp::cache::VideoCache;

    let mut group = c.benchmark_group("cache_ops");

    let video = make_video(10);

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime failed");

    group.bench_function("cache_put_single", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                let path = dir.path().to_path_buf();
                (dir, path, video.clone())
            },
            |(dir, path, v)| async move {
                let cache = VideoCache::new(path, None)
                    .await
                    .expect("cache init failed");
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
                let cache = VideoCache::new(path, None)
                    .await
                    .expect("cache init failed");
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
                let cache = VideoCache::new(path, None)
                    .await
                    .expect("cache init failed");
                let _ = cache
                    .get("https://example.com/missing")
                    .await
                    .expect("get failed");
            },
        );
    });

    group.bench_function("cache_put_500", |b| {
        b.to_async(&rt).iter_with_setup(
            || {
                let dir = tempfile::TempDir::new().expect("tempdir failed");
                dir.path().to_path_buf()
            },
            |path| async move {
                let cache = VideoCache::new(path, None)
                    .await
                    .expect("cache init failed");
                for i in 0..500usize {
                    let mut v = make_video(5);
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

#[cfg(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))]
criterion_group!(cache_benches, bench_cache_ops);

#[cfg(all(
    feature = "webhooks",
    any(feature = "cache", feature = "cache-json", feature = "cache-sqlite")
))]
criterion_main!(benches, webhooks_benches, cache_benches);

#[cfg(all(
    feature = "webhooks",
    not(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))
))]
criterion_main!(benches, webhooks_benches);

#[cfg(all(
    not(feature = "webhooks"),
    any(feature = "cache", feature = "cache-json", feature = "cache-sqlite")
))]
criterion_main!(benches, cache_benches);

#[cfg(all(
    not(feature = "webhooks"),
    not(any(feature = "cache", feature = "cache-json", feature = "cache-sqlite"))
))]
criterion_main!(benches);
