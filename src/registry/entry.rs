//! Stream entry and state types
//!
//! This module defines the per-stream state stored in the registry.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use tokio::sync::broadcast;

use crate::media::flv::FlvTag;
use crate::media::gop::GopBuffer;

use super::config::RegistryConfig;
use super::frame::BroadcastFrame;

/// State of a stream entry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream has an active publisher
    Active,
    /// Publisher disconnected, within grace period
    GracePeriod,
    /// No publisher, waiting for cleanup
    Idle,
}

/// Entry for a single stream in the registry
pub struct StreamEntry {
    /// GOP buffer for late-joiner support
    pub gop_buffer: GopBuffer,

    /// Cached video sequence header for fast subscriber catchup
    pub video_header: Option<BroadcastFrame>,

    /// Cached audio sequence header for fast subscriber catchup
    pub audio_header: Option<BroadcastFrame>,

    /// Cached metadata
    pub metadata: Option<BroadcastFrame>,

    /// Current publisher's session ID (None if no publisher)
    pub publisher_id: Option<u64>,

    /// Broadcast sender for fan-out to subscribers
    pub(super) tx: broadcast::Sender<BroadcastFrame>,

    /// Number of active subscribers
    pub subscriber_count: AtomicU32,

    /// When the publisher disconnected (for grace period tracking)
    pub publisher_disconnected_at: Option<Instant>,

    /// When the stream was created
    pub created_at: Instant,

    /// Current stream state
    pub state: StreamState,
}

impl StreamEntry {
    /// Create a new stream entry
    pub(super) fn new(config: &RegistryConfig) -> Self {
        let (tx, _) = broadcast::channel(config.broadcast_capacity);

        Self {
            gop_buffer: GopBuffer::with_max_size(config.max_gop_size),
            video_header: None,
            audio_header: None,
            metadata: None,
            publisher_id: None,
            tx,
            subscriber_count: AtomicU32::new(0),
            publisher_disconnected_at: None,
            created_at: Instant::now(),
            state: StreamState::Idle,
        }
    }

    /// Get the number of subscribers
    pub fn subscriber_count(&self) -> u32 {
        self.subscriber_count.load(Ordering::Relaxed)
    }

    /// Check if the stream has an active publisher
    pub fn has_publisher(&self) -> bool {
        self.publisher_id.is_some()
    }

    /// Get catchup frames for a new subscriber
    ///
    /// Returns sequence headers followed by GOP buffer contents.
    pub fn get_catchup_frames(&self) -> Vec<BroadcastFrame> {
        let mut frames = Vec::new();

        // Add metadata first
        if let Some(ref meta) = self.metadata {
            frames.push(meta.clone());
        }

        // Add sequence headers
        if let Some(ref video) = self.video_header {
            frames.push(video.clone());
        }
        if let Some(ref audio) = self.audio_header {
            frames.push(audio.clone());
        }

        // Add GOP buffer contents
        for tag in self.gop_buffer.get_catchup_data() {
            frames.push(BroadcastFrame::from_flv_tag(&tag));
        }

        frames
    }

    /// Subscribe to this stream's broadcast channel
    pub(super) fn subscribe(&self) -> broadcast::Receiver<BroadcastFrame> {
        self.tx.subscribe()
    }

    /// Send a frame to all subscribers
    ///
    /// Returns the number of receivers that received the message, or 0 if there are no receivers.
    pub(super) fn send(&self, frame: BroadcastFrame) -> usize {
        self.tx.send(frame).unwrap_or(0)
    }

    /// Update cached headers and GOP buffer based on frame type
    pub(super) fn update_caches(&mut self, frame: &BroadcastFrame) {
        use super::frame::FrameType;

        match frame.frame_type {
            FrameType::Video if frame.is_header => {
                self.video_header = Some(frame.clone());
            }
            FrameType::Audio if frame.is_header => {
                self.audio_header = Some(frame.clone());
            }
            FrameType::Metadata => {
                self.metadata = Some(frame.clone());
            }
            _ => {}
        }

        // Update GOP buffer for video frames (non-headers)
        if frame.frame_type == FrameType::Video && !frame.is_header {
            let tag = FlvTag::video(frame.timestamp, frame.data.clone());
            self.gop_buffer.push(tag);
        }
    }
}

/// Statistics for a stream
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Number of active subscribers
    pub subscriber_count: u32,
    /// Whether the stream has an active publisher
    pub has_publisher: bool,
    /// Current stream state
    pub state: StreamState,
    /// Number of frames in GOP buffer
    pub gop_frame_count: usize,
    /// Size of GOP buffer in bytes
    pub gop_size_bytes: usize,
}
