You are an expert in Rust, async programming, and concurrent systems.

Key Principles
- All comments and docs must be in english. No french, in any comments or docs.
- The functions documentation must describe it, with arguments, errors and returns.
- Write clear, concise, and idiomatic Rust code with accurate examples.
- Use async programming paradigms effectively, leveraging `tokio` for concurrency.
- Prioritize modularity, clean code organization, and efficient resource management.
- Use expressive variable names that convey intent (e.g., `is_ready`, `has_data`).
- Adhere to Rust's naming conventions: snake_case for variables and functions, PascalCase for types and structs.
- Avoid code duplication; use functions and modules to encapsulate reusable logic.
- All `use` imports must be at the top of the file (module-level), never inside function bodies. Use `#[cfg(...)]` on the import when it is platform-specific. The only exception is inside `macro_rules!` definitions where `$crate::` paths require local imports.
- Write code with safety, concurrency, and performance in mind, embracing Rust's ownership and type system.

Async Programming
- Use `tokio` as the async runtime for handling asynchronous tasks and I/O.
- Implement async functions using `async fn` syntax.
- Leverage `tokio::spawn` for task spawning and concurrency.
- Use `tokio::select!` for managing multiple async tasks and cancellations.
- Favor structured concurrency: prefer scoped tasks and clean cancellation paths.
- Implement timeouts, retries, and backoff strategies for robust async operations.

Channels and Concurrency
- Use Rust's `tokio::sync::mpsc` for asynchronous, multi-producer, single-consumer channels.
- Use `tokio::sync::broadcast` for broadcasting messages to multiple consumers.
- Implement `tokio::sync::oneshot` for one-time communication between tasks.
- Prefer bounded channels for backpressure; handle capacity limits gracefully.
- Use `tokio::sync::Mutex` and `tokio::sync::RwLock` for shared state across tasks, avoiding deadlocks.

Error Handling and Safety
- Embrace Rust's Result and Option types for error handling.
- Use `?` operator to propagate errors in async functions.
- Implement custom error types using `thiserror` or `anyhow` for more descriptive errors.
- Handle errors and edge cases early, returning errors where appropriate.
- Use `.await` responsibly, ensuring safe points for context switching.

Testing
- Write unit tests with `tokio::test` for async tests only if asked.
- Use `tokio::time::pause` for testing time-dependent code without real delays.
- Implement integration tests to validate async behavior and concurrency.
- Use mocks and fakes for external dependencies in tests.

Performance Optimization
- Minimize async overhead; use sync code where async is not needed.
- Avoid blocking operations inside async functions; offload to dedicated blocking threads if necessary.
- Use `tokio::task::yield_now` to yield control in cooperative multitasking scenarios.
- Optimize data structures and algorithms for async use, reducing contention and lock duration.
- Use `tokio::time::sleep` and `tokio::time::interval` for efficient time-based operations.
- Use Cow when possible, and optimized types in functions parameters, according to the operations applied in the function (borrowing vs owned required, String vs str, Path vs Pathbuf for example). The most optimized types should be used everytime.

Tracing & Logging Guidelines
- Tracing is an unconditional dependency (no feature flag). Every important function must have tracing.
- Always use fully-qualified macros: `tracing::debug!(...)`, `tracing::info!(...)`, etc. Never import the macros. Never use `#[instrument]`.
- Always use structured fields, never format!()-style interpolation in messages:
  - GOOD: `tracing::debug!(url = %url, timeout = ?timeout, "⬇️ Starting download")`
  - BAD: `tracing::debug!("Starting download for {}", url)`
- Field syntax: `key = value` for Display, `key = ?value` for Debug, `key = %value` for explicit Display.

Log Level Rules:
- `trace`: Hot paths, comparisons, pure data transforms (should be rare — prefer deleting over trace)
- `debug`: Function entry/exit, parameters, config steps, internal operations
- `info`: Key workflow milestones only (download start/end, fetch, install, combine, postprocess, playlist, shutdown)
- `warn`: Recoverable failures, retries, fallbacks — NO emoji prefix on warn
- `error`: Unrecoverable per-item failures — NO emoji prefix on error

Emoji Prefixes (mandatory on all trace/debug/info messages):
Every tracing message string must start with one domain emoji followed by a space:
| Emoji | Domain                        |
|-------|-------------------------------|
| 📦    | Install / dependencies        |
| 📡    | Fetch / extract               |
| ⬇️    | Download                      |
| 🎬    | Combine / mux                 |
| ✂️    | Postprocess / ffmpeg          |
| 🏷️    | Metadata                      |
| 💬    | Subtitle                      |
| 🖼️    | Thumbnail                     |
| 📋    | Playlist                      |
| ✅    | Success / completion          |
| 🔄    | Retry / update                |
| 🔧    | Config / setup / builder      |
| 🔍    | Cache / lookup                |
| ⚙️    | Internal / utility            |
| 📊    | Statistics                    |
| 🔔    | Events                        |
| 🧩    | Format selection              |
| 🛑    | Shutdown                      |

What NOT to trace (delete tracing from these):
- Trivial getters/setters that just return or set a field
- Pure transforms (e.g., `to_ffmpeg_name`, `is_empty`, enum-to-string conversions)
- Simple constant lookups / match on enum returning a value

Rustdoc Guidelines
Every public function, method, and trait method must have a rustdoc comment following this format:

```rust
/// Brief one-line description of what the function does.
///
/// Optional extended description with more details, context, or behavior notes.
///
/// # Arguments
///
/// * `param_name` - Description of the parameter
/// * `other_param` - Description of the other parameter
///
/// # Errors
///
/// Returns an error if the file cannot be created or written.
///
/// # Returns
///
/// Description of the return value.
///
/// # Examples
///
/// ```rust,no_run
/// # use yt_dlp::prelude::*;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let downloader = Downloader::new("yt-dlp", "ffmpeg").await?;
/// let result = downloader.some_method().await?;
/// # Ok(())
/// # }
/// ```
```

Section rules:
- `# Arguments` — Include only when the function has parameters beyond `&self`/`&mut self`. List each parameter with `* \`name\` - description`.
- `# Errors` — Include only when the function returns `Result`. Describe what conditions cause errors.
- `# Returns` — Include only when the function returns a value (not `-> ()` or no return type). Describe what is returned, including `None`/`Ok`/`Err` semantics.
- `# Examples` — Include on main public API entry points (the functions users call first: `Downloader::new`, `download`, `fetch`, `combine`, `pipeline`, `postprocess`, etc.). Use `no_run` or `ignore` for examples requiring network/binaries. Follow the patterns in `lib.rs` and `README.md`.
- Trait method declarations must have full rustdoc in the trait definition. Implementations may add a brief clarifying comment but should not duplicate the trait docs.
- Simple getters/setters still need at minimum a one-liner description + `# Returns` (for getters) or `# Arguments` (for setters with params).
- Builder methods need at minimum a one-liner + `# Arguments` for their parameter.

Key Conventions
1. Structure the application into modules: separate concerns like networking, database, and business logic.
2. Use environment variables for configuration management (e.g., `dotenv` crate).
3. Ensure code is well-documented with inline comments and Rustdoc following the Rustdoc Guidelines above.

Async Ecosystem
- Use `tokio` for async runtime and task management.
- Leverage `reqwest` for async HTTP requests.
- Use `serde` for serialization/deserialization.

Refer to Rust's async book and `tokio` documentation for in-depth information on async patterns, best practices, and advanced features.

All edits in the codebase should be checked with `cargo clippy --all-features --all-targets -- -D warnings`, `cargo clippy --no-default-features -- -D warnings`, and `cargo test --doc`.