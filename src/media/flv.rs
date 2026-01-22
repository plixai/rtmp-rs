//! FLV tag parsing
//!
//! FLV (Flash Video) is the container format used by RTMP for audio/video data.
//! Each RTMP audio/video message is essentially an FLV tag without the tag header.
//!
//! FLV Tag Structure (for reference, RTMP messages don't include this header):
//! ```text
//! +--------+-------------+-----------+
//! | Type(1)| DataSize(3) | TS(3+1)   | StreamID(3) | Data(N) |
//! +--------+-------------+-----------+
//! ```
//!
//! RTMP Video Data:
//! ```text
//! +----------+----------+
//! | FrameType| CodecID  | CodecData...
//! | (4 bits) | (4 bits) |
//! +----------+----------+
//! ```
//!
//! RTMP Audio Data:
//! ```text
//! +----------+----------+----------+----------+
//! |SoundFormat|SoundRate|SoundSize |SoundType | AudioData...
//! | (4 bits)  | (2 bits)| (1 bit)  | (1 bit)  |
//! +----------+----------+----------+----------+
//! ```

use bytes::Bytes;

/// FLV tag type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlvTagType {
    Audio,
    Video,
    Script,
}

/// Parsed FLV tag
#[derive(Debug, Clone)]
pub struct FlvTag {
    /// Tag type
    pub tag_type: FlvTagType,
    /// Timestamp in milliseconds
    pub timestamp: u32,
    /// Raw tag data (including codec headers)
    pub data: Bytes,
}

/// Video frame type (upper 4 bits of first byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoFrameType {
    /// Keyframe (for AVC, a seekable frame)
    Keyframe = 1,
    /// Inter frame (for AVC, a non-seekable frame)
    InterFrame = 2,
    /// Disposable inter frame (H.263 only)
    DisposableInterFrame = 3,
    /// Generated keyframe (reserved for server use)
    GeneratedKeyframe = 4,
    /// Video info/command frame
    VideoInfoFrame = 5,
}

impl VideoFrameType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match (b >> 4) & 0x0F {
            1 => Some(VideoFrameType::Keyframe),
            2 => Some(VideoFrameType::InterFrame),
            3 => Some(VideoFrameType::DisposableInterFrame),
            4 => Some(VideoFrameType::GeneratedKeyframe),
            5 => Some(VideoFrameType::VideoInfoFrame),
            _ => None,
        }
    }

    pub fn is_keyframe(&self) -> bool {
        matches!(
            self,
            VideoFrameType::Keyframe | VideoFrameType::GeneratedKeyframe
        )
    }
}

/// Video codec ID (lower 4 bits of first byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    /// Sorenson H.263
    SorensonH263 = 2,
    /// Screen video
    ScreenVideo = 3,
    /// VP6
    Vp6 = 4,
    /// VP6 with alpha
    Vp6Alpha = 5,
    /// Screen video v2
    ScreenVideoV2 = 6,
    /// AVC (H.264)
    Avc = 7,
    /// HEVC (H.265) - enhanced RTMP extension
    Hevc = 12,
    /// AV1 - enhanced RTMP extension
    Av1 = 13,
}

impl VideoCodec {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b & 0x0F {
            2 => Some(VideoCodec::SorensonH263),
            3 => Some(VideoCodec::ScreenVideo),
            4 => Some(VideoCodec::Vp6),
            5 => Some(VideoCodec::Vp6Alpha),
            6 => Some(VideoCodec::ScreenVideoV2),
            7 => Some(VideoCodec::Avc),
            12 => Some(VideoCodec::Hevc),
            13 => Some(VideoCodec::Av1),
            _ => None,
        }
    }
}

/// Audio format (upper 4 bits of first byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// Linear PCM, platform endian
    LinearPcmPlatform = 0,
    /// ADPCM
    Adpcm = 1,
    /// MP3
    Mp3 = 2,
    /// Linear PCM, little endian
    LinearPcmLe = 3,
    /// Nellymoser 16kHz mono
    Nellymoser16kMono = 4,
    /// Nellymoser 8kHz mono
    Nellymoser8kMono = 5,
    /// Nellymoser
    Nellymoser = 6,
    /// G.711 A-law
    G711ALaw = 7,
    /// G.711 mu-law
    G711MuLaw = 8,
    /// AAC
    Aac = 10,
    /// Speex
    Speex = 11,
    /// MP3 8kHz
    Mp38k = 14,
    /// Device-specific sound
    DeviceSpecific = 15,
}

impl AudioFormat {
    pub fn from_byte(b: u8) -> Option<Self> {
        match (b >> 4) & 0x0F {
            0 => Some(AudioFormat::LinearPcmPlatform),
            1 => Some(AudioFormat::Adpcm),
            2 => Some(AudioFormat::Mp3),
            3 => Some(AudioFormat::LinearPcmLe),
            4 => Some(AudioFormat::Nellymoser16kMono),
            5 => Some(AudioFormat::Nellymoser8kMono),
            6 => Some(AudioFormat::Nellymoser),
            7 => Some(AudioFormat::G711ALaw),
            8 => Some(AudioFormat::G711MuLaw),
            10 => Some(AudioFormat::Aac),
            11 => Some(AudioFormat::Speex),
            14 => Some(AudioFormat::Mp38k),
            15 => Some(AudioFormat::DeviceSpecific),
            _ => None,
        }
    }
}

/// Audio sample rate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSampleRate {
    Rate5512 = 0,
    Rate11025 = 1,
    Rate22050 = 2,
    Rate44100 = 3,
}

impl AudioSampleRate {
    pub fn from_byte(b: u8) -> Self {
        match (b >> 2) & 0x03 {
            0 => AudioSampleRate::Rate5512,
            1 => AudioSampleRate::Rate11025,
            2 => AudioSampleRate::Rate22050,
            _ => AudioSampleRate::Rate44100,
        }
    }

    pub fn to_hz(&self) -> u32 {
        match self {
            AudioSampleRate::Rate5512 => 5512,
            AudioSampleRate::Rate11025 => 11025,
            AudioSampleRate::Rate22050 => 22050,
            AudioSampleRate::Rate44100 => 44100,
        }
    }
}

impl FlvTag {
    /// Create a new video tag
    pub fn video(timestamp: u32, data: Bytes) -> Self {
        Self {
            tag_type: FlvTagType::Video,
            timestamp,
            data,
        }
    }

    /// Create a new audio tag
    pub fn audio(timestamp: u32, data: Bytes) -> Self {
        Self {
            tag_type: FlvTagType::Audio,
            timestamp,
            data,
        }
    }

    /// Check if this is a video tag
    pub fn is_video(&self) -> bool {
        self.tag_type == FlvTagType::Video
    }

    /// Check if this is an audio tag
    pub fn is_audio(&self) -> bool {
        self.tag_type == FlvTagType::Audio
    }

    /// For video tags, get the frame type
    pub fn video_frame_type(&self) -> Option<VideoFrameType> {
        if self.is_video() && !self.data.is_empty() {
            VideoFrameType::from_byte(self.data[0])
        } else {
            None
        }
    }

    /// For video tags, get the codec
    pub fn video_codec(&self) -> Option<VideoCodec> {
        if self.is_video() && !self.data.is_empty() {
            VideoCodec::from_byte(self.data[0])
        } else {
            None
        }
    }

    /// For audio tags, get the format
    pub fn audio_format(&self) -> Option<AudioFormat> {
        if self.is_audio() && !self.data.is_empty() {
            AudioFormat::from_byte(self.data[0])
        } else {
            None
        }
    }

    /// Check if this is a keyframe
    pub fn is_keyframe(&self) -> bool {
        self.video_frame_type()
            .map(|ft| ft.is_keyframe())
            .unwrap_or(false)
    }

    /// Check if this is an AVC sequence header
    pub fn is_avc_sequence_header(&self) -> bool {
        if self.is_video() && self.data.len() >= 2 {
            let codec = VideoCodec::from_byte(self.data[0]);
            codec == Some(VideoCodec::Avc) && self.data[1] == 0
        } else {
            false
        }
    }

    /// Check if this is an AAC sequence header
    pub fn is_aac_sequence_header(&self) -> bool {
        if self.is_audio() && self.data.len() >= 2 {
            let format = AudioFormat::from_byte(self.data[0]);
            format == Some(AudioFormat::Aac) && self.data[1] == 0
        } else {
            false
        }
    }

    /// Get the size of the tag data
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_type() {
        // Keyframe + AVC
        assert_eq!(
            VideoFrameType::from_byte(0x17),
            Some(VideoFrameType::Keyframe)
        );
        assert_eq!(VideoCodec::from_byte(0x17), Some(VideoCodec::Avc));

        // Inter frame + AVC
        assert_eq!(
            VideoFrameType::from_byte(0x27),
            Some(VideoFrameType::InterFrame)
        );
    }

    #[test]
    fn test_avc_sequence_header() {
        let header = FlvTag::video(0, Bytes::from_static(&[0x17, 0x00, 0x00, 0x00, 0x00]));
        assert!(header.is_avc_sequence_header());
        assert!(header.is_keyframe());

        let frame = FlvTag::video(0, Bytes::from_static(&[0x17, 0x01, 0x00, 0x00, 0x00]));
        assert!(!frame.is_avc_sequence_header());
    }

    #[test]
    fn test_aac_sequence_header() {
        let header = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x00, 0x12, 0x10]));
        assert!(header.is_aac_sequence_header());

        let frame = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x01, 0x21, 0x00]));
        assert!(!frame.is_aac_sequence_header());
    }

    #[test]
    fn test_video_frame_type_all_values() {
        assert_eq!(
            VideoFrameType::from_byte(0x10),
            Some(VideoFrameType::Keyframe)
        );
        assert_eq!(
            VideoFrameType::from_byte(0x20),
            Some(VideoFrameType::InterFrame)
        );
        assert_eq!(
            VideoFrameType::from_byte(0x30),
            Some(VideoFrameType::DisposableInterFrame)
        );
        assert_eq!(
            VideoFrameType::from_byte(0x40),
            Some(VideoFrameType::GeneratedKeyframe)
        );
        assert_eq!(
            VideoFrameType::from_byte(0x50),
            Some(VideoFrameType::VideoInfoFrame)
        );
        assert_eq!(VideoFrameType::from_byte(0x00), None);
        assert_eq!(VideoFrameType::from_byte(0x60), None);
    }

    #[test]
    fn test_video_frame_type_is_keyframe() {
        assert!(VideoFrameType::Keyframe.is_keyframe());
        assert!(VideoFrameType::GeneratedKeyframe.is_keyframe());
        assert!(!VideoFrameType::InterFrame.is_keyframe());
        assert!(!VideoFrameType::DisposableInterFrame.is_keyframe());
        assert!(!VideoFrameType::VideoInfoFrame.is_keyframe());
    }

    #[test]
    fn test_video_codec_all_values() {
        assert_eq!(VideoCodec::from_byte(0x02), Some(VideoCodec::SorensonH263));
        assert_eq!(VideoCodec::from_byte(0x03), Some(VideoCodec::ScreenVideo));
        assert_eq!(VideoCodec::from_byte(0x04), Some(VideoCodec::Vp6));
        assert_eq!(VideoCodec::from_byte(0x05), Some(VideoCodec::Vp6Alpha));
        assert_eq!(VideoCodec::from_byte(0x06), Some(VideoCodec::ScreenVideoV2));
        assert_eq!(VideoCodec::from_byte(0x07), Some(VideoCodec::Avc));
        assert_eq!(VideoCodec::from_byte(0x0C), Some(VideoCodec::Hevc));
        assert_eq!(VideoCodec::from_byte(0x0D), Some(VideoCodec::Av1));
        assert_eq!(VideoCodec::from_byte(0x00), None);
        assert_eq!(VideoCodec::from_byte(0x01), None);
        assert_eq!(VideoCodec::from_byte(0x08), None);
    }

    #[test]
    fn test_audio_format_all_values() {
        assert_eq!(
            AudioFormat::from_byte(0x00),
            Some(AudioFormat::LinearPcmPlatform)
        );
        assert_eq!(AudioFormat::from_byte(0x10), Some(AudioFormat::Adpcm));
        assert_eq!(AudioFormat::from_byte(0x20), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_byte(0x30), Some(AudioFormat::LinearPcmLe));
        assert_eq!(
            AudioFormat::from_byte(0x40),
            Some(AudioFormat::Nellymoser16kMono)
        );
        assert_eq!(
            AudioFormat::from_byte(0x50),
            Some(AudioFormat::Nellymoser8kMono)
        );
        assert_eq!(AudioFormat::from_byte(0x60), Some(AudioFormat::Nellymoser));
        assert_eq!(AudioFormat::from_byte(0x70), Some(AudioFormat::G711ALaw));
        assert_eq!(AudioFormat::from_byte(0x80), Some(AudioFormat::G711MuLaw));
        assert_eq!(AudioFormat::from_byte(0xA0), Some(AudioFormat::Aac));
        assert_eq!(AudioFormat::from_byte(0xB0), Some(AudioFormat::Speex));
        assert_eq!(AudioFormat::from_byte(0xE0), Some(AudioFormat::Mp38k));
        assert_eq!(
            AudioFormat::from_byte(0xF0),
            Some(AudioFormat::DeviceSpecific)
        );
        assert_eq!(AudioFormat::from_byte(0x90), None); // 9 is not defined
    }

    #[test]
    fn test_audio_sample_rate() {
        assert_eq!(AudioSampleRate::from_byte(0x00).to_hz(), 5512);
        assert_eq!(AudioSampleRate::from_byte(0x04).to_hz(), 11025);
        assert_eq!(AudioSampleRate::from_byte(0x08).to_hz(), 22050);
        assert_eq!(AudioSampleRate::from_byte(0x0C).to_hz(), 44100);
        // Test masking
        assert_eq!(AudioSampleRate::from_byte(0xFF).to_hz(), 44100);
    }

    #[test]
    fn test_flv_tag_video_construction() {
        let tag = FlvTag::video(1000, Bytes::from_static(&[0x17, 0x01]));
        assert!(tag.is_video());
        assert!(!tag.is_audio());
        assert_eq!(tag.tag_type, FlvTagType::Video);
        assert_eq!(tag.timestamp, 1000);
    }

    #[test]
    fn test_flv_tag_audio_construction() {
        let tag = FlvTag::audio(2000, Bytes::from_static(&[0xAF, 0x01]));
        assert!(tag.is_audio());
        assert!(!tag.is_video());
        assert_eq!(tag.tag_type, FlvTagType::Audio);
        assert_eq!(tag.timestamp, 2000);
    }

    #[test]
    fn test_flv_tag_video_frame_type() {
        // Keyframe AVC
        let keyframe = FlvTag::video(0, Bytes::from_static(&[0x17, 0x01]));
        assert_eq!(keyframe.video_frame_type(), Some(VideoFrameType::Keyframe));
        assert!(keyframe.is_keyframe());

        // Inter frame AVC
        let interframe = FlvTag::video(0, Bytes::from_static(&[0x27, 0x01]));
        assert_eq!(
            interframe.video_frame_type(),
            Some(VideoFrameType::InterFrame)
        );
        assert!(!interframe.is_keyframe());

        // Audio tag should return None
        let audio = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x01]));
        assert!(audio.video_frame_type().is_none());
    }

    #[test]
    fn test_flv_tag_video_codec() {
        // AVC codec
        let avc = FlvTag::video(0, Bytes::from_static(&[0x17, 0x01]));
        assert_eq!(avc.video_codec(), Some(VideoCodec::Avc));

        // HEVC codec (enhanced RTMP)
        let hevc = FlvTag::video(0, Bytes::from_static(&[0x1C, 0x01]));
        assert_eq!(hevc.video_codec(), Some(VideoCodec::Hevc));

        // Audio should return None
        let audio = FlvTag::audio(0, Bytes::from_static(&[0xAF]));
        assert!(audio.video_codec().is_none());
    }

    #[test]
    fn test_flv_tag_audio_format() {
        // AAC audio
        let aac = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x01]));
        assert_eq!(aac.audio_format(), Some(AudioFormat::Aac));

        // MP3 audio
        let mp3 = FlvTag::audio(0, Bytes::from_static(&[0x2F]));
        assert_eq!(mp3.audio_format(), Some(AudioFormat::Mp3));

        // Video should return None
        let video = FlvTag::video(0, Bytes::from_static(&[0x17]));
        assert!(video.audio_format().is_none());
    }

    #[test]
    fn test_flv_tag_empty_data() {
        let empty_video = FlvTag::video(0, Bytes::new());
        assert!(empty_video.video_frame_type().is_none());
        assert!(empty_video.video_codec().is_none());
        assert!(!empty_video.is_keyframe());
        assert!(!empty_video.is_avc_sequence_header());

        let empty_audio = FlvTag::audio(0, Bytes::new());
        assert!(empty_audio.audio_format().is_none());
        assert!(!empty_audio.is_aac_sequence_header());
    }

    #[test]
    fn test_flv_tag_size() {
        let tag = FlvTag::video(0, Bytes::from_static(&[0x17, 0x00, 0x00, 0x00, 0x00]));
        assert_eq!(tag.size(), 5);

        let empty_tag = FlvTag::audio(0, Bytes::new());
        assert_eq!(empty_tag.size(), 0);
    }

    #[test]
    fn test_is_avc_sequence_header_non_avc() {
        // Non-AVC codec (e.g., HEVC)
        let hevc = FlvTag::video(0, Bytes::from_static(&[0x1C, 0x00, 0x00, 0x00, 0x00]));
        assert!(!hevc.is_avc_sequence_header());

        // AVC but not sequence header (packet type 1)
        let avc_nalu = FlvTag::video(0, Bytes::from_static(&[0x17, 0x01, 0x00, 0x00, 0x00]));
        assert!(!avc_nalu.is_avc_sequence_header());
    }

    #[test]
    fn test_is_aac_sequence_header_non_aac() {
        // Non-AAC format (e.g., MP3)
        let mp3 = FlvTag::audio(0, Bytes::from_static(&[0x2F, 0x00]));
        assert!(!mp3.is_aac_sequence_header());

        // AAC but raw frame (packet type 1)
        let aac_raw = FlvTag::audio(0, Bytes::from_static(&[0xAF, 0x01]));
        assert!(!aac_raw.is_aac_sequence_header());
    }

    #[test]
    fn test_flv_tag_type_enum() {
        assert_eq!(FlvTagType::Audio, FlvTagType::Audio);
        assert_ne!(FlvTagType::Audio, FlvTagType::Video);
        assert_ne!(FlvTagType::Video, FlvTagType::Script);
    }

    #[test]
    fn test_combined_video_byte() {
        // Test decoding both frame type and codec from same byte
        // 0x17 = keyframe (1) + AVC (7)
        let tag = FlvTag::video(0, Bytes::from_static(&[0x17]));
        assert_eq!(tag.video_frame_type(), Some(VideoFrameType::Keyframe));
        assert_eq!(tag.video_codec(), Some(VideoCodec::Avc));

        // 0x27 = inter frame (2) + AVC (7)
        let tag = FlvTag::video(0, Bytes::from_static(&[0x27]));
        assert_eq!(tag.video_frame_type(), Some(VideoFrameType::InterFrame));
        assert_eq!(tag.video_codec(), Some(VideoCodec::Avc));

        // 0x14 = keyframe (1) + VP6 (4)
        let tag = FlvTag::video(0, Bytes::from_static(&[0x14]));
        assert_eq!(tag.video_frame_type(), Some(VideoFrameType::Keyframe));
        assert_eq!(tag.video_codec(), Some(VideoCodec::Vp6));
    }

    #[test]
    fn test_short_video_data() {
        // Only 1 byte - enough for frame type and codec
        let tag = FlvTag::video(0, Bytes::from_static(&[0x17]));
        assert!(tag.video_frame_type().is_some());
        assert!(tag.video_codec().is_some());

        // But not enough for sequence header check (needs 2 bytes)
        assert!(!tag.is_avc_sequence_header());
    }

    #[test]
    fn test_short_audio_data() {
        // Only 1 byte - enough for format
        let tag = FlvTag::audio(0, Bytes::from_static(&[0xAF]));
        assert!(tag.audio_format().is_some());

        // But not enough for sequence header check (needs 2 bytes)
        assert!(!tag.is_aac_sequence_header());
    }
}
