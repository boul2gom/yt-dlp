fn main() {
    // Declare the custom cfg so rustc doesn't warn (required since Rust 1.80)
    println!("cargo:rustc-check-cfg=cfg(cache_backend, values(\"sqlite\", \"json\", \"memory\"))");

    let has_sqlite = std::env::var("CARGO_FEATURE_CACHE_SQLITE").is_ok();
    let has_json = std::env::var("CARGO_FEATURE_CACHE_JSON").is_ok();
    let has_memory = std::env::var("CARGO_FEATURE_CACHE").is_ok();

    // Priority: sqlite > json > memory.
    // Exactly one cfg is emitted, regardless of how many feature flags are active simultaneously.
    if has_sqlite {
        println!("cargo:rustc-cfg=cache_backend=\"sqlite\"");
    } else if has_json {
        println!("cargo:rustc-cfg=cache_backend=\"json\"");
    } else if has_memory {
        println!("cargo:rustc-cfg=cache_backend=\"memory\"");
    }
    // If none active: no cfg emitted → no backend compiled (compile_error in cache/mod.rs fires).
}
