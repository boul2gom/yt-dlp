//! Format-specific metadata writers.
//!
//! This module contains metadata writers for different audio/video formats:
//! FFmpeg-based writing, Lofty-based writing, MP3-specific, and MP4-specific.

pub mod ffmpeg;
pub mod lofty;
pub mod mp3;
pub mod mp4;
