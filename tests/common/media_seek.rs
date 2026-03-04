use std::io;

/// A mock [`media_seek::RangeFetcher`] backed by an in-memory byte buffer.
///
/// Clamps requested ranges to the buffer length so tests never panic
/// on out-of-bounds accesses.
#[derive(Debug, Clone)]
pub struct MockRangeFetcher {
    data: Vec<u8>,
}

impl MockRangeFetcher {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl media_seek::RangeFetcher for MockRangeFetcher {
    type Error = io::Error;

    async fn fetch(&self, start: u64, end: u64) -> Result<Vec<u8>, Self::Error> {
        let start = start as usize;
        let end = (end as usize).min(self.data.len().saturating_sub(1));

        if start > end || start >= self.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Range {}-{} out of bounds for {} bytes", start, end, self.data.len()),
            ));
        }

        // Inclusive end
        Ok(self.data[start..=end].to_vec())
    }
}
