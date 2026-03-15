mod common {
    #[path = "../common/fixtures.rs"]
    pub mod fixtures;
    #[path = "../common/media_seek.rs"]
    pub mod media_seek;
    #[path = "../common/test_extractor.rs"]
    pub mod test_extractor;
}

#[cfg(cache)]
#[path = "unit/cache/config.rs"]
mod cache_config;
#[path = "unit/download/config.rs"]
mod config;
#[path = "unit/error.rs"]
mod error;
#[path = "unit/events.rs"]
mod events;
#[path = "unit/extractor/extractor.rs"]
mod extractor;
#[path = "unit/executor/ffmpeg_args.rs"]
mod ffmpeg_args;
#[path = "unit/utils/fs_utils.rs"]
mod fs_utils;
#[cfg(any(feature = "live-recording", feature = "live-streaming"))]
#[path = "unit/live/hls_parsing.rs"]
mod hls_parsing;
#[path = "unit/media-seek/mod.rs"]
mod media_seek;
#[path = "unit/metadata.rs"]
mod metadata;
#[path = "unit/model/mod.rs"]
mod model;
#[path = "unit/utils/network_retry.rs"]
mod network_retry;
#[path = "unit/download/partial.rs"]
mod partial_range;
#[path = "unit/utils/platform.rs"]
mod platform;
#[path = "unit/executor/process.rs"]
mod process;
#[path = "unit/utils/proxy.rs"]
mod proxy;
#[path = "unit/download/segment.rs"]
mod segment;
#[path = "unit/selection.rs"]
mod selection;
#[cfg(feature = "statistics")]
#[path = "unit/statistics.rs"]
mod stats;
#[path = "unit/utils/subtitle.rs"]
mod subtitle;
#[path = "unit/extractor/test_extractor.rs"]
mod test_extractor;
#[path = "unit/utils/validation.rs"]
mod validation;
