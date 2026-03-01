//! Container seek index types returned by all format parsers.

/// A single entry in a container's seek index.
#[derive(Debug, Clone)]
pub struct SegmentEntry {
    /// Presentation timestamp of the first sample in the segment (seconds).
    pub start_secs: f64,
    /// Presentation timestamp just past the last sample in the segment (seconds).
    pub end_secs: f64,
    /// Byte offset of the segment's first byte within the stream.
    pub byte_offset: u64,
    /// Byte length of the segment.
    pub byte_size: u64,
}

/// Internal storage for the seek index.
#[derive(Debug, Clone)]
pub(crate) enum Inner {
    /// Explicit segment list (fMP4, WebM, OGG, FLAC, FLV, AVI, TS, MP3 VBR).
    Segments(Vec<SegmentEntry>),
    /// Linear PCM or CBR — byte offset derived from presentation time and a fixed rate.
    ///
    /// `byte_offset = floor(secs * byte_rate / block_align) * block_align`
    Linear { byte_rate: f64, block_align: u64 },
}

/// The parsed seek index for a media stream.
///
/// Returned by [`crate::parse`]. Call [`find_byte_range`](ContainerIndex::find_byte_range)
/// to translate a `[start_secs, end_secs]` window into the byte ranges needed for
/// a partial download.
#[derive(Debug, Clone)]
pub struct ContainerIndex {
    /// Last byte (inclusive) of the codec initialisation data (moov, EBML header, etc.).
    ///
    /// A partial download must always begin with `bytes 0..=init_end_byte` so that
    /// decoders have the necessary codec parameters before the content data.
    ///
    /// `0` for formats where no separate init segment exists (WAV, AIFF, …).
    pub init_end_byte: u64,
    pub(crate) inner: Inner,
}

impl ContainerIndex {
    /// Finds the content byte range that covers `[start_secs, end_secs]`.
    ///
    /// For segmented formats the range is expanded to the nearest segment boundaries
    /// so that the returned slice is always decodable.
    ///
    /// # Arguments
    ///
    /// * `start_secs` - Start of the desired window (seconds, inclusive).
    /// * `end_secs` - End of the desired window (seconds, exclusive).
    ///
    /// # Returns
    ///
    /// `Some((content_start_byte, content_end_byte))` (both inclusive) on success,
    /// or `None` if the index is empty.
    pub fn find_byte_range(&self, start_secs: f64, end_secs: f64) -> Option<(u64, u64)> {
        match &self.inner {
            Inner::Linear { byte_rate, block_align } => {
                let align = (*block_align).max(1);
                let start_byte = ((start_secs * byte_rate / align as f64).floor() as u64) * align;
                let end_byte = ((end_secs * byte_rate / align as f64).ceil() as u64) * align;
                Some((start_byte, end_byte.saturating_sub(1)))
            }
            Inner::Segments(segments) => {
                if segments.is_empty() {
                    return None;
                }

                // Binary search: last segment whose start_secs <= desired start (O(log n))
                let i = segments.partition_point(|s| s.start_secs <= start_secs);
                let first = if i > 0 { &segments[i - 1] } else { segments.first()? };

                // Binary search: first segment whose start_secs >= desired end (O(log n))
                let j = segments.partition_point(|s| s.start_secs < end_secs);
                let last = segments.get(j).or_else(|| segments.last())?;

                let content_start = first.byte_offset;
                let content_end = last.byte_offset + last.byte_size.saturating_sub(1);
                Some((content_start, content_end))
            }
        }
    }
}
