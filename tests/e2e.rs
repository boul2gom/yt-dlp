mod common {
    #[path = "../common/assertions.rs"]
    pub mod assertions;
    #[path = "../common/downloader.rs"]
    pub mod downloader;
    #[path = "../common/fixtures.rs"]
    pub mod fixtures;
    #[path = "../common/server.rs"]
    pub mod server;
}

#[path = "e2e/helpers.rs"]
mod helpers;

#[cfg(cache)]
#[path = "e2e/cache/pipeline.rs"]
mod cache_pipeline;
#[path = "e2e/events/cancellation.rs"]
mod cancellation;
#[path = "e2e/download/combine.rs"]
mod download_combine;
#[path = "e2e/download/manager.rs"]
mod download_manager_e2e;
#[path = "e2e/download/partial.rs"]
mod download_partial;
#[path = "e2e/download/video.rs"]
mod download_video;
#[path = "e2e/error_paths.rs"]
mod error_paths;
#[path = "e2e/events/pipeline.rs"]
mod events_pipeline;
#[cfg(feature = "live-recording")]
#[path = "e2e/live/recording.rs"]
mod live_e2e;
#[path = "e2e/pipeline/fluent.rs"]
mod pipeline_fluent;
#[path = "e2e/playlist.rs"]
mod playlist;
#[path = "e2e/subtitle.rs"]
mod subtitle;

#[path = "e2e/media-seek/mod.rs"]
mod media_seek;
