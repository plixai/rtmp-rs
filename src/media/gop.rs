//! GOP (Group of Pictures) buffer for late-joiner support
//!
//! When a new client connects to an existing stream, they need to receive:
//! 1. The sequence headers (SPS/PPS for video, AudioSpecificConfig for audio)
//! 2. The most recent keyframe
//! 3. All frames since that keyframe
//!
//! This allows the decoder to start decoding from the keyframe without
//! waiting for the next one.

use bytes::Bytes;
use std::collections::VecDeque;

use super::flv::FlvTag;

/// A buffered media frame
#[derive(Debug, Clone)]
struct BufferedFrame {
    /// The FLV tag
    tag: FlvTag,
    /// Size in bytes
    size: usize,
}

/// GOP buffer for late-joiner support
#[derive(Debug)]
pub struct GopBuffer {
    /// Maximum buffer size in bytes
    max_size: usize,
    /// Current buffer size in bytes
    current_size: usize,
    /// Video sequence header
    video_header: Option<FlvTag>,
    /// Audio sequence header
    audio_header: Option<FlvTag>,
    /// Metadata
    metadata: Option<Bytes>,
    /// Buffered frames since last keyframe
    frames: VecDeque<BufferedFrame>,
    /// Whether we have a complete GOP (started with keyframe)
    has_complete_gop: bool,
}

impl GopBuffer {
    /// Create a new GOP buffer with default max size (4MB)
    pub fn new() -> Self {
        Self::with_max_size(4 * 1024 * 1024)
    }

    /// Create a new GOP buffer with specified max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            current_size: 0,
            video_header: None,
            audio_header: None,
            metadata: None,
            frames: VecDeque::new(),
            has_complete_gop: false,
        }
    }

    /// Set the video sequence header
    pub fn set_video_header(&mut self, tag: FlvTag) {
        self.video_header = Some(tag);
    }

    /// Set the audio sequence header
    pub fn set_audio_header(&mut self, tag: FlvTag) {
        self.audio_header = Some(tag);
    }

    /// Set metadata
    pub fn set_metadata(&mut self, metadata: Bytes) {
        self.metadata = Some(metadata);
    }

    /// Add a frame to the buffer
    ///
    /// If this is a keyframe, clears the buffer first.
    /// Returns true if the frame was added, false if buffer is full.
    pub fn push(&mut self, tag: FlvTag) -> bool {
        let size = tag.size();

        // Handle keyframes - start a new GOP
        if tag.is_keyframe() {
            self.clear_frames();
            self.has_complete_gop = true;
        }

        // Check size limit
        if self.current_size + size > self.max_size {
            // Try to make room by dropping oldest frames
            while self.current_size + size > self.max_size && !self.frames.is_empty() {
                if let Some(old) = self.frames.pop_front() {
                    self.current_size -= old.size;
                }
            }

            // If still too big, reject
            if self.current_size + size > self.max_size {
                return false;
            }
        }

        self.frames.push_back(BufferedFrame { tag, size });
        self.current_size += size;
        true
    }

    /// Clear all buffered frames (but keep headers)
    pub fn clear_frames(&mut self) {
        self.frames.clear();
        self.current_size = 0;
        self.has_complete_gop = false;
    }

    /// Clear everything including headers
    pub fn clear(&mut self) {
        self.clear_frames();
        self.video_header = None;
        self.audio_header = None;
        self.metadata = None;
    }

    /// Get the video sequence header
    pub fn video_header(&self) -> Option<&FlvTag> {
        self.video_header.as_ref()
    }

    /// Get the audio sequence header
    pub fn audio_header(&self) -> Option<&FlvTag> {
        self.audio_header.as_ref()
    }

    /// Get metadata
    pub fn metadata(&self) -> Option<&Bytes> {
        self.metadata.as_ref()
    }

    /// Check if we have a complete GOP
    pub fn has_complete_gop(&self) -> bool {
        self.has_complete_gop
    }

    /// Check if the buffer is ready for late-joiners
    ///
    /// Returns true if we have at least the video header and a complete GOP.
    pub fn is_ready(&self) -> bool {
        self.video_header.is_some() && self.has_complete_gop
    }

    /// Get all buffered frames for a late-joiner
    ///
    /// Returns sequence headers followed by all buffered frames.
    pub fn get_catchup_data(&self) -> Vec<FlvTag> {
        let mut result = Vec::with_capacity(self.frames.len() + 3);

        // Add metadata first (if available)
        // Note: Metadata is stored as Bytes, not FlvTag - skip for now

        // Add sequence headers
        if let Some(h) = &self.video_header {
            result.push(h.clone());
        }
        if let Some(h) = &self.audio_header {
            result.push(h.clone());
        }

        // Add all buffered frames
        for frame in &self.frames {
            result.push(frame.tag.clone());
        }

        result
    }

    /// Get the number of buffered frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get the current buffer size in bytes
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Get buffer utilization as a percentage
    pub fn utilization(&self) -> f32 {
        if self.max_size > 0 {
            (self.current_size as f32 / self.max_size as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Get the timestamp range of buffered frames
    pub fn timestamp_range(&self) -> Option<(u32, u32)> {
        if self.frames.is_empty() {
            return None;
        }

        let first = self.frames.front()?.tag.timestamp;
        let last = self.frames.back()?.tag.timestamp;
        Some((first, last))
    }

    /// Get GOP duration in milliseconds
    pub fn gop_duration(&self) -> Option<u32> {
        self.timestamp_range().map(|(first, last)| last - first)
    }
}

impl Default for GopBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tag(timestamp: u32, is_keyframe: bool, size: usize) -> FlvTag {
        let mut data = vec![0u8; size];
        if is_keyframe {
            data[0] = 0x17; // Keyframe + AVC
        } else {
            data[0] = 0x27; // Inter frame + AVC
        }
        FlvTag::video(timestamp, Bytes::from(data))
    }

    #[test]
    fn test_gop_buffer_basic() {
        let mut buffer = GopBuffer::new();

        // Not ready initially
        assert!(!buffer.is_ready());

        // Add video header
        buffer.set_video_header(make_tag(0, true, 100));
        assert!(!buffer.is_ready()); // Still need GOP

        // Add keyframe - starts GOP
        buffer.push(make_tag(0, true, 500));
        assert!(buffer.is_ready());
        assert!(buffer.has_complete_gop());

        // Add inter frames
        buffer.push(make_tag(33, false, 200));
        buffer.push(make_tag(66, false, 200));

        assert_eq!(buffer.frame_count(), 3);
    }

    #[test]
    fn test_gop_buffer_keyframe_clears() {
        let mut buffer = GopBuffer::new();

        // Add some frames
        buffer.push(make_tag(0, true, 500));
        buffer.push(make_tag(33, false, 200));
        buffer.push(make_tag(66, false, 200));
        assert_eq!(buffer.frame_count(), 3);

        // New keyframe clears old frames
        buffer.push(make_tag(100, true, 500));
        assert_eq!(buffer.frame_count(), 1);
    }

    #[test]
    fn test_gop_buffer_size_limit() {
        let mut buffer = GopBuffer::with_max_size(500);

        buffer.push(make_tag(0, true, 200));
        buffer.push(make_tag(33, false, 200));

        // This succeeds by dropping the keyframe (200 + 200 = 400 < 500)
        assert!(buffer.push(make_tag(66, false, 200)));

        // Single frame larger than max_size should fail
        assert!(!buffer.push(make_tag(99, false, 600)));
    }

    #[test]
    fn test_gop_duration() {
        let mut buffer = GopBuffer::new();

        buffer.push(make_tag(0, true, 100));
        buffer.push(make_tag(33, false, 100));
        buffer.push(make_tag(66, false, 100));

        assert_eq!(buffer.gop_duration(), Some(66));
    }

    #[test]
    fn test_gop_buffer_default() {
        let buffer = GopBuffer::default();
        assert!(!buffer.is_ready());
        assert_eq!(buffer.frame_count(), 0);
        assert_eq!(buffer.size(), 0);
    }

    #[test]
    fn test_gop_buffer_headers() {
        let mut buffer = GopBuffer::new();

        // Set headers
        let video_header = FlvTag::video(0, Bytes::from_static(&[0x17, 0x00]));
        let audio_header = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x00]));

        buffer.set_video_header(video_header.clone());
        buffer.set_audio_header(audio_header.clone());

        assert!(buffer.video_header().is_some());
        assert!(buffer.audio_header().is_some());
    }

    #[test]
    fn test_gop_buffer_metadata() {
        let mut buffer = GopBuffer::new();

        let metadata = Bytes::from_static(b"test metadata");
        buffer.set_metadata(metadata.clone());

        assert!(buffer.metadata().is_some());
        assert_eq!(buffer.metadata().unwrap(), &metadata);
    }

    #[test]
    fn test_gop_buffer_clear() {
        let mut buffer = GopBuffer::new();

        buffer.set_video_header(make_tag(0, true, 50));
        buffer.set_audio_header(FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x00])));
        buffer.set_metadata(Bytes::from_static(b"meta"));
        buffer.push(make_tag(0, true, 100));
        buffer.push(make_tag(33, false, 100));

        buffer.clear();

        assert!(buffer.video_header().is_none());
        assert!(buffer.audio_header().is_none());
        assert!(buffer.metadata().is_none());
        assert_eq!(buffer.frame_count(), 0);
        assert!(!buffer.has_complete_gop());
    }

    #[test]
    fn test_gop_buffer_clear_frames_only() {
        let mut buffer = GopBuffer::new();

        buffer.set_video_header(make_tag(0, true, 50));
        buffer.push(make_tag(0, true, 100));
        buffer.push(make_tag(33, false, 100));

        buffer.clear_frames();

        // Headers should still be there
        assert!(buffer.video_header().is_some());
        // Frames should be gone
        assert_eq!(buffer.frame_count(), 0);
        assert!(!buffer.has_complete_gop());
    }

    #[test]
    fn test_gop_buffer_utilization() {
        let mut buffer = GopBuffer::with_max_size(1000);

        assert_eq!(buffer.utilization(), 0.0);

        buffer.push(make_tag(0, true, 500));
        assert!((buffer.utilization() - 50.0).abs() < 1.0); // ~50%
    }

    #[test]
    fn test_gop_buffer_timestamp_range() {
        let mut buffer = GopBuffer::new();

        // Empty buffer
        assert!(buffer.timestamp_range().is_none());

        // Single frame
        buffer.push(make_tag(100, true, 50));
        assert_eq!(buffer.timestamp_range(), Some((100, 100)));

        // Multiple frames
        buffer.push(make_tag(133, false, 50));
        buffer.push(make_tag(166, false, 50));
        assert_eq!(buffer.timestamp_range(), Some((100, 166)));
    }

    #[test]
    fn test_gop_buffer_get_catchup_data() {
        let mut buffer = GopBuffer::new();

        buffer.set_video_header(FlvTag::video(0, Bytes::from_static(&[0x17, 0x00])));
        buffer.set_audio_header(FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x00])));
        buffer.push(make_tag(0, true, 100));
        buffer.push(make_tag(33, false, 50));
        buffer.push(make_tag(66, false, 50));

        let catchup = buffer.get_catchup_data();

        // Should have: video header + audio header + 3 frames = 5 items
        assert_eq!(catchup.len(), 5);

        // First should be video header
        assert!(catchup[0].is_avc_sequence_header());
        // Second should be audio header
        assert!(catchup[1].is_aac_sequence_header());
        // Rest are frames
        assert!(catchup[2].is_keyframe());
    }

    #[test]
    fn test_gop_buffer_get_catchup_data_without_headers() {
        let mut buffer = GopBuffer::new();

        buffer.push(make_tag(0, true, 100));
        buffer.push(make_tag(33, false, 50));

        let catchup = buffer.get_catchup_data();

        // Just the frames, no headers
        assert_eq!(catchup.len(), 2);
    }

    #[test]
    fn test_gop_buffer_drops_old_frames_to_fit() {
        let mut buffer = GopBuffer::with_max_size(500);

        // Add frames that total more than max
        buffer.push(make_tag(0, true, 200));
        buffer.push(make_tag(33, false, 200));
        // Total: 400, still fits

        // This will push us over, causing oldest frame(s) to be dropped
        buffer.push(make_tag(66, false, 200));

        // Should have dropped oldest to make room
        assert!(buffer.size() <= 500);
    }

    #[test]
    fn test_gop_buffer_no_complete_gop_without_keyframe() {
        let mut buffer = GopBuffer::new();

        // Add non-keyframes only
        buffer.push(make_tag(0, false, 100));
        buffer.push(make_tag(33, false, 100));

        assert!(!buffer.has_complete_gop());

        // Now add keyframe
        buffer.push(make_tag(66, true, 100));
        assert!(buffer.has_complete_gop());
    }

    #[test]
    fn test_gop_buffer_is_ready_requirements() {
        let mut buffer = GopBuffer::new();

        // Not ready: no header, no GOP
        assert!(!buffer.is_ready());

        // Add video header but no GOP
        buffer.set_video_header(FlvTag::video(0, Bytes::from_static(&[0x17, 0x00])));
        assert!(!buffer.is_ready());

        // Add non-keyframe - still not ready
        buffer.push(make_tag(0, false, 100));
        assert!(!buffer.is_ready());

        // Add keyframe - now ready
        buffer.push(make_tag(33, true, 100));
        assert!(buffer.is_ready());
    }

    #[test]
    fn test_gop_buffer_audio_only_stream() {
        let mut buffer = GopBuffer::new();

        // Audio-only stream might set audio header
        buffer.set_audio_header(FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x00])));

        // is_ready requires video_header, so audio-only won't be "ready"
        // This is intentional - late joiners need video keyframe typically
        assert!(!buffer.is_ready());
    }
}
