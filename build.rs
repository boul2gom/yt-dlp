fn main() {
    // Declare custom cfgs so rustc doesn't warn (required since Rust 1.80)
    println!("cargo::rustc-check-cfg=cfg(cache)");
    println!("cargo::rustc-check-cfg=cfg(persistent_cache)");

    let has_memory = std::env::var("CARGO_FEATURE_CACHE_MEMORY").is_ok();
    let has_json = std::env::var("CARGO_FEATURE_CACHE_JSON").is_ok();
    let has_redb = std::env::var("CARGO_FEATURE_CACHE_REDB").is_ok();
    let has_redis = std::env::var("CARGO_FEATURE_CACHE_REDIS").is_ok();

    // Emit `cache` cfg if any cache backend is enabled (replaces the old umbrella feature)
    if has_memory || has_json || has_redb || has_redis {
        println!("cargo::rustc-cfg=cache");
    }

    let persistent_count = [has_json, has_redb, has_redis].iter().filter(|&&b| b).count();

    // Emit cfg if any persistent backend is enabled
    if persistent_count > 0 {
        println!("cargo::rustc-cfg=persistent_cache");
    }
}
