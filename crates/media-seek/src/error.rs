//! Error types for the `media-seek` crate.

/// A type alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors that can be produced by `media-seek`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The container format is not recognised (MHTML, plaintext, or unknown magic bytes).
    #[error("Unsupported or unrecognized container format")]
    UnsupportedFormat,

    /// The container index could not be parsed (truncated data, invalid structure, etc.).
    #[error("Container index parse failed: {reason}")]
    ParseFailed { reason: String },

    /// The container format was detected but no seek index was found in the probe.
    ///
    /// This may occur when a classic MP4's `moov` box is beyond the probe window, or when
    /// neither a SIDX box nor a `moov` box are present within the probed bytes.
    #[error("Seek index not found in probe: {reason}")]
    IndexNotFound { reason: String },

    /// An extra `Range` fetch required by the parser failed (WebM Cues, AVI idx1, MPEG-TS PCR, OGG bisection).
    #[error("Extra Range fetch failed: {0}")]
    FetchFailed(Box<dyn std::error::Error + Send + Sync>),
}

impl Error {
    /// Convenience constructor for `ParseFailed`.
    pub(crate) fn parse(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        tracing::warn!(reason = %reason, "Container index parse failed");
        Self::ParseFailed { reason }
    }

    /// Convenience constructor for `IndexNotFound`.
    pub(crate) fn index_not_found(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        tracing::warn!(reason = %reason, "Seek index not found in probe");
        Self::IndexNotFound { reason }
    }

    /// Convenience constructor for `FetchFailed`.
    pub(crate) fn fetch<E: std::error::Error + Send + Sync + 'static>(source: E) -> Self {
        tracing::warn!(error = %source, "Extra Range fetch failed");
        Self::FetchFailed(Box::new(source))
    }
}
