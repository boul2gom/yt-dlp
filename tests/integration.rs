mod common {
    #[path = "../common/cache.rs"]
    pub mod cache;
    #[path = "../common/fixtures.rs"]
    pub mod fixtures;
    #[path = "../common/media_seek.rs"]
    pub mod media_seek;
    #[path = "../common/server.rs"]
    pub mod server;
}

#[path = "integration/events/bus.rs"]
mod event_bus;

#[cfg(feature = "hooks")]
#[path = "integration/events/hooks.rs"]
mod hooks;

#[cfg(feature = "webhooks")]
#[path = "integration/events/webhooks.rs"]
mod webhooks;

#[cfg(feature = "statistics")]
#[path = "integration/statistics.rs"]
mod statistics;

#[cfg(feature = "cache-memory")]
#[path = "integration/cache/memory.rs"]
mod cache_memory;

#[cfg(feature = "cache-json")]
#[path = "integration/cache/json.rs"]
mod cache_json;

#[cfg(feature = "cache-json")]
#[path = "integration/cache/files.rs"]
mod cache_files;

#[cfg(feature = "cache-redb")]
#[path = "integration/cache/redb.rs"]
mod cache_redb;

#[cfg(feature = "cache-redis")]
#[path = "integration/cache/redis.rs"]
mod cache_redis;

#[cfg(persistent_cache)]
#[path = "integration/cache/layer.rs"]
mod cache_layer;

#[path = "integration/download/manager.rs"]
mod download_manager;

#[path = "integration/download/fetcher.rs"]
mod fetcher;

#[path = "integration/download/fetcher_range.rs"]
mod fetcher_range;

#[path = "integration/fs_io.rs"]
mod fs_io;

#[path = "integration/builder.rs"]
mod builder;

#[cfg(feature = "live-recording")]
#[path = "integration/live/recording.rs"]
mod live_recording;

#[cfg(feature = "live-streaming")]
#[path = "integration/live/streaming.rs"]
mod live_streaming;

#[path = "integration/media-seek/mod.rs"]
mod media_seek;

#[path = "integration/metadata.rs"]
mod metadata;

#[path = "integration/download/progress_tracker.rs"]
mod progress_tracker;

#[path = "integration/utils/http.rs"]
mod http_client;

#[path = "integration/client/deps/github.rs"]
mod github;
