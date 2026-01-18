//! Media handling for RTMP
//!
//! This module provides:
//! - FLV tag parsing and generation
//! - H.264/AVC NALU parsing
//! - AAC frame parsing
//! - GOP buffering for late-joiner support

pub mod aac;
pub mod flv;
pub mod gop;
pub mod h264;

pub use aac::{AacData, AacPacketType, AudioSpecificConfig};
pub use flv::{FlvTag, FlvTagType};
pub use gop::GopBuffer;
pub use h264::{AvcPacketType, H264Data, NaluType};
