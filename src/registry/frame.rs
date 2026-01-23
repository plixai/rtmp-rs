//! Broadcast frame types for stream routing
//!
//! This module defines the key types for identifying streams and the frames
//! that are broadcast to subscribers.

use bytes::Bytes;

use crate::media::flv::{FlvTag, FlvTagType};

/// Unique identifier for a stream (app + stream name)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamKey {
    /// Application name (e.g., "live")
    pub app: String,
    /// Stream name/key (e.g., "stream_key_123")
    pub name: String,
}

impl StreamKey {
    /// Create a new stream key
    pub fn new(app: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            name: name.into(),
        }
    }
}

impl std::fmt::Display for StreamKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.app, self.name)
    }
}

/// Type of broadcast frame
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// Video frame
    Video,
    /// Audio frame
    Audio,
    /// Metadata (onMetaData)
    Metadata,
}

/// A frame to be broadcast to subscribers
///
/// This is designed to be cheap to clone due to `Bytes` reference counting.
#[derive(Debug, Clone)]
pub struct BroadcastFrame {
    /// Type of frame
    pub frame_type: FrameType,
    /// Timestamp in milliseconds
    pub timestamp: u32,
    /// Frame data (zero-copy via reference counting)
    pub data: Bytes,
    /// Whether this is a keyframe (video only)
    pub is_keyframe: bool,
    /// Whether this is a sequence header
    pub is_header: bool,
}

impl BroadcastFrame {
    /// Create a video frame
    pub fn video(timestamp: u32, data: Bytes, is_keyframe: bool, is_header: bool) -> Self {
        Self {
            frame_type: FrameType::Video,
            timestamp,
            data,
            is_keyframe,
            is_header,
        }
    }

    /// Create an audio frame
    pub fn audio(timestamp: u32, data: Bytes, is_header: bool) -> Self {
        Self {
            frame_type: FrameType::Audio,
            timestamp,
            data,
            is_keyframe: false,
            is_header,
        }
    }

    /// Create a metadata frame
    pub fn metadata(data: Bytes) -> Self {
        Self {
            frame_type: FrameType::Metadata,
            timestamp: 0,
            data,
            is_keyframe: false,
            is_header: false,
        }
    }

    /// Convert from FLV tag
    pub fn from_flv_tag(tag: &FlvTag) -> Self {
        match tag.tag_type {
            FlvTagType::Video => {
                let is_keyframe = tag.is_keyframe();
                let is_header = tag.is_avc_sequence_header();
                Self::video(tag.timestamp, tag.data.clone(), is_keyframe, is_header)
            }
            FlvTagType::Audio => {
                let is_header = tag.is_aac_sequence_header();
                Self::audio(tag.timestamp, tag.data.clone(), is_header)
            }
            FlvTagType::Script => Self::metadata(tag.data.clone()),
        }
    }
}
