//! Enhanced RTMP (E-RTMP) capability types
//!
//! This module defines the capability flags and structures used for
//! E-RTMP capability negotiation during the connect handshake.
//!
//! Reference: E-RTMP v2 specification - "Enhancing NetConnection connect Command"

use std::collections::HashMap;

use crate::media::fourcc::{AudioFourCc, VideoFourCc};

/// Extended capabilities bitmask (capsEx field in connect command).
///
/// These flags indicate support for various E-RTMP features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapsEx(u32);

impl CapsEx {
    /// Support for NetConnection.Connect.ReconnectRequest
    pub const RECONNECT: u32 = 0x01;
    /// Support for multitrack audio/video
    pub const MULTITRACK: u32 = 0x02;
    /// Support for ModEx signal parsing
    pub const MODEX: u32 = 0x04;
    /// Support for nanosecond timestamp offsets
    pub const TIMESTAMP_NANO_OFFSET: u32 = 0x08;

    /// Create empty capabilities.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create from raw u32 value.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Check if a capability is set.
    pub const fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a capability flag.
    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    /// Remove a capability flag.
    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    /// Compute intersection of two capability sets.
    pub const fn intersection(&self, other: &Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Check if reconnect is supported.
    pub const fn supports_reconnect(&self) -> bool {
        self.contains(Self::RECONNECT)
    }

    /// Check if multitrack is supported.
    pub const fn supports_multitrack(&self) -> bool {
        self.contains(Self::MULTITRACK)
    }

    /// Check if ModEx signal parsing is supported.
    pub const fn supports_modex(&self) -> bool {
        self.contains(Self::MODEX)
    }

    /// Check if nanosecond timestamp offset is supported.
    pub const fn supports_timestamp_nano_offset(&self) -> bool {
        self.contains(Self::TIMESTAMP_NANO_OFFSET)
    }
}

/// Codec capability flags for FOURCC info maps.
///
/// These flags indicate what operations a peer can perform with a codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FourCcCapability(u32);

impl FourCcCapability {
    /// Can decode this codec
    pub const CAN_DECODE: u32 = 0x01;
    /// Can encode this codec
    pub const CAN_ENCODE: u32 = 0x02;
    /// Can forward/relay this codec without transcoding
    pub const CAN_FORWARD: u32 = 0x04;

    /// Create empty capability.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create from raw u32 value.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Create capability indicating decode support.
    pub const fn decode() -> Self {
        Self(Self::CAN_DECODE)
    }

    /// Create capability indicating encode support.
    pub const fn encode() -> Self {
        Self(Self::CAN_ENCODE)
    }

    /// Create capability indicating forward/relay support.
    pub const fn forward() -> Self {
        Self(Self::CAN_FORWARD)
    }

    /// Create capability indicating full support (decode + encode + forward).
    pub const fn full() -> Self {
        Self(Self::CAN_DECODE | Self::CAN_ENCODE | Self::CAN_FORWARD)
    }

    /// Check if decode is supported.
    pub const fn can_decode(&self) -> bool {
        (self.0 & Self::CAN_DECODE) != 0
    }

    /// Check if encode is supported.
    pub const fn can_encode(&self) -> bool {
        (self.0 & Self::CAN_ENCODE) != 0
    }

    /// Check if forward is supported.
    pub const fn can_forward(&self) -> bool {
        (self.0 & Self::CAN_FORWARD) != 0
    }
}

/// Video function flags (videoFunction field in connect command).
///
/// These flags indicate support for specific video features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VideoFunctionFlags(u32);

impl VideoFunctionFlags {
    /// Client can perform frame-accurate seeks
    pub const CLIENT_SEEK: u32 = 1;

    /// Create empty flags.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Create from raw u32 value.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Get raw bits.
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Check if client seek is supported.
    pub const fn supports_client_seek(&self) -> bool {
        (self.0 & Self::CLIENT_SEEK) != 0
    }
}

/// Negotiated E-RTMP capabilities for a session.
///
/// This structure holds the result of capability negotiation between
/// client and server during the connect handshake.
#[derive(Debug, Clone, Default)]
pub struct EnhancedCapabilities {
    /// Whether E-RTMP mode is enabled for this session.
    pub enabled: bool,

    /// Extended capabilities flags (intersection of client and server).
    pub caps_ex: CapsEx,

    /// Supported video codecs with their capabilities.
    pub video_codecs: HashMap<VideoFourCc, FourCcCapability>,

    /// Supported audio codecs with their capabilities.
    pub audio_codecs: HashMap<AudioFourCc, FourCcCapability>,

    /// Video function flags.
    pub video_function: VideoFunctionFlags,
}

impl EnhancedCapabilities {
    /// Create new empty capabilities (E-RTMP disabled).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create capabilities with E-RTMP enabled and default codec support.
    pub fn with_defaults() -> Self {
        let mut caps = Self {
            enabled: true,
            caps_ex: CapsEx::from_bits(CapsEx::MODEX),
            video_codecs: HashMap::new(),
            audio_codecs: HashMap::new(),
            video_function: VideoFunctionFlags::empty(),
        };

        // Default video codec support (forward-only for relay servers)
        caps.video_codecs
            .insert(VideoFourCc::Avc, FourCcCapability::forward());
        caps.video_codecs
            .insert(VideoFourCc::Hevc, FourCcCapability::forward());
        caps.video_codecs
            .insert(VideoFourCc::Av1, FourCcCapability::forward());
        caps.video_codecs
            .insert(VideoFourCc::Vp9, FourCcCapability::forward());

        // Default audio codec support
        caps.audio_codecs
            .insert(AudioFourCc::Aac, FourCcCapability::forward());
        caps.audio_codecs
            .insert(AudioFourCc::Opus, FourCcCapability::forward());

        caps
    }

    /// Check if a video codec is supported.
    pub fn supports_video_codec(&self, codec: VideoFourCc) -> bool {
        self.video_codecs.contains_key(&codec)
    }

    /// Check if an audio codec is supported.
    pub fn supports_audio_codec(&self, codec: AudioFourCc) -> bool {
        self.audio_codecs.contains_key(&codec)
    }

    /// Get capability for a video codec.
    pub fn video_codec_capability(&self, codec: VideoFourCc) -> Option<FourCcCapability> {
        self.video_codecs.get(&codec).copied()
    }

    /// Get capability for an audio codec.
    pub fn audio_codec_capability(&self, codec: AudioFourCc) -> Option<FourCcCapability> {
        self.audio_codecs.get(&codec).copied()
    }

    /// Check if multitrack is supported.
    pub fn supports_multitrack(&self) -> bool {
        self.enabled && self.caps_ex.supports_multitrack()
    }

    /// Check if reconnect is supported.
    pub fn supports_reconnect(&self) -> bool {
        self.enabled && self.caps_ex.supports_reconnect()
    }

    /// Compute intersection with another capability set.
    ///
    /// Used to negotiate common capabilities between client and server.
    pub fn intersect(&self, other: &Self) -> Self {
        if !self.enabled || !other.enabled {
            return Self::new();
        }

        let mut result = Self {
            enabled: true,
            caps_ex: self.caps_ex.intersection(&other.caps_ex),
            video_codecs: HashMap::new(),
            audio_codecs: HashMap::new(),
            video_function: VideoFunctionFlags::from_bits(
                self.video_function.bits() & other.video_function.bits(),
            ),
        };

        // Intersect video codecs
        for (codec, self_cap) in &self.video_codecs {
            if let Some(other_cap) = other.video_codecs.get(codec) {
                let common = FourCcCapability::from_bits(self_cap.bits() & other_cap.bits());
                if common.bits() != 0 {
                    result.video_codecs.insert(*codec, common);
                }
            }
        }

        // Intersect audio codecs
        for (codec, self_cap) in &self.audio_codecs {
            if let Some(other_cap) = other.audio_codecs.get(codec) {
                let common = FourCcCapability::from_bits(self_cap.bits() & other_cap.bits());
                if common.bits() != 0 {
                    result.audio_codecs.insert(*codec, common);
                }
            }
        }

        result
    }
}

/// E-RTMP mode configuration.
///
/// Controls how the server/client handles E-RTMP capability negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnhancedRtmpMode {
    /// Automatically negotiate E-RTMP if peer supports it (default).
    #[default]
    Auto,

    /// Use legacy RTMP only, even if peer supports E-RTMP.
    LegacyOnly,

    /// Require E-RTMP; reject connections from legacy peers.
    EnhancedOnly,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caps_ex_empty() {
        let caps = CapsEx::empty();
        assert_eq!(caps.bits(), 0);
        assert!(!caps.supports_reconnect());
        assert!(!caps.supports_multitrack());
        assert!(!caps.supports_modex());
        assert!(!caps.supports_timestamp_nano_offset());
    }

    #[test]
    fn test_caps_ex_flags() {
        let caps = CapsEx::from_bits(CapsEx::RECONNECT | CapsEx::MULTITRACK);
        assert!(caps.supports_reconnect());
        assert!(caps.supports_multitrack());
        assert!(!caps.supports_modex());
        assert!(!caps.supports_timestamp_nano_offset());
    }

    #[test]
    fn test_caps_ex_insert_remove() {
        let mut caps = CapsEx::empty();

        caps.insert(CapsEx::RECONNECT);
        assert!(caps.supports_reconnect());

        caps.insert(CapsEx::MODEX);
        assert!(caps.supports_modex());

        caps.remove(CapsEx::RECONNECT);
        assert!(!caps.supports_reconnect());
        assert!(caps.supports_modex());
    }

    #[test]
    fn test_caps_ex_intersection() {
        let client = CapsEx::from_bits(CapsEx::RECONNECT | CapsEx::MULTITRACK | CapsEx::MODEX);
        let server = CapsEx::from_bits(CapsEx::MULTITRACK | CapsEx::MODEX);

        let common = client.intersection(&server);
        assert!(!common.supports_reconnect());
        assert!(common.supports_multitrack());
        assert!(common.supports_modex());
    }

    #[test]
    fn test_fourcc_capability_flags() {
        let cap = FourCcCapability::full();
        assert!(cap.can_decode());
        assert!(cap.can_encode());
        assert!(cap.can_forward());

        let forward_only = FourCcCapability::forward();
        assert!(!forward_only.can_decode());
        assert!(!forward_only.can_encode());
        assert!(forward_only.can_forward());
    }

    #[test]
    fn test_fourcc_capability_from_bits() {
        let cap = FourCcCapability::from_bits(
            FourCcCapability::CAN_DECODE | FourCcCapability::CAN_FORWARD,
        );
        assert!(cap.can_decode());
        assert!(!cap.can_encode());
        assert!(cap.can_forward());
    }

    #[test]
    fn test_enhanced_capabilities_default() {
        let caps = EnhancedCapabilities::new();
        assert!(!caps.enabled);
        assert!(caps.video_codecs.is_empty());
        assert!(caps.audio_codecs.is_empty());
    }

    #[test]
    fn test_enhanced_capabilities_with_defaults() {
        let caps = EnhancedCapabilities::with_defaults();
        assert!(caps.enabled);
        assert!(caps.supports_video_codec(VideoFourCc::Avc));
        assert!(caps.supports_video_codec(VideoFourCc::Hevc));
        assert!(caps.supports_video_codec(VideoFourCc::Av1));
        assert!(caps.supports_audio_codec(AudioFourCc::Aac));
        assert!(caps.supports_audio_codec(AudioFourCc::Opus));

        // VP8 is also in defaults
        assert!(caps.supports_video_codec(VideoFourCc::Vp9));
    }

    #[test]
    fn test_enhanced_capabilities_codec_lookup() {
        let caps = EnhancedCapabilities::with_defaults();

        let avc_cap = caps.video_codec_capability(VideoFourCc::Avc).unwrap();
        assert!(avc_cap.can_forward());

        let vp8_cap = caps.video_codec_capability(VideoFourCc::Vp8);
        // VP8 might not be in defaults, depending on implementation
        assert!(vp8_cap.is_none() || vp8_cap.unwrap().can_forward());
    }

    #[test]
    fn test_enhanced_capabilities_intersect() {
        let mut client = EnhancedCapabilities::with_defaults();
        client.caps_ex = CapsEx::from_bits(CapsEx::RECONNECT | CapsEx::MODEX);
        client
            .video_codecs
            .insert(VideoFourCc::Avc, FourCcCapability::full());
        client
            .video_codecs
            .insert(VideoFourCc::Vp8, FourCcCapability::decode());

        let mut server = EnhancedCapabilities::with_defaults();
        server.caps_ex = CapsEx::from_bits(CapsEx::MODEX);
        server
            .video_codecs
            .insert(VideoFourCc::Avc, FourCcCapability::forward());
        // Server doesn't support VP8

        let common = client.intersect(&server);
        assert!(common.enabled);
        assert!(!common.caps_ex.supports_reconnect()); // Client only
        assert!(common.caps_ex.supports_modex()); // Both

        // AVC: intersection of full and forward = forward
        let avc_cap = common.video_codec_capability(VideoFourCc::Avc).unwrap();
        assert!(avc_cap.can_forward());
        assert!(!avc_cap.can_encode()); // Server can't encode

        // VP8: not in common (server doesn't support)
        assert!(!common.supports_video_codec(VideoFourCc::Vp8));
    }

    #[test]
    fn test_enhanced_capabilities_intersect_disabled() {
        let client = EnhancedCapabilities::with_defaults();
        let server = EnhancedCapabilities::new(); // disabled

        let common = client.intersect(&server);
        assert!(!common.enabled);
    }

    #[test]
    fn test_enhanced_rtmp_mode_default() {
        let mode = EnhancedRtmpMode::default();
        assert_eq!(mode, EnhancedRtmpMode::Auto);
    }

    #[test]
    fn test_video_function_flags() {
        let flags = VideoFunctionFlags::from_bits(VideoFunctionFlags::CLIENT_SEEK);
        assert!(flags.supports_client_seek());

        let empty = VideoFunctionFlags::empty();
        assert!(!empty.supports_client_seek());
    }

    #[test]
    fn test_multitrack_support() {
        let mut caps = EnhancedCapabilities::with_defaults();
        assert!(!caps.supports_multitrack()); // Not enabled by default

        caps.caps_ex.insert(CapsEx::MULTITRACK);
        assert!(caps.supports_multitrack());

        caps.enabled = false;
        assert!(!caps.supports_multitrack()); // Disabled overall
    }

    #[test]
    fn test_reconnect_support() {
        let mut caps = EnhancedCapabilities::with_defaults();
        assert!(!caps.supports_reconnect());

        caps.caps_ex.insert(CapsEx::RECONNECT);
        assert!(caps.supports_reconnect());
    }
}
