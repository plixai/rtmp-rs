//! OBS and encoder compatibility quirks
//!
//! Different RTMP encoders (OBS, ffmpeg, Wirecast, etc.) have various
//! non-standard behaviors. This module documents known quirks and provides
//! helpers for handling them.
//!
//! # Known Quirks
//!
//! ## OBS Studio
//! - Sends FCPublish before publish (Twitch/YouTube compatibility)
//! - May send releaseStream before connect completes
//! - Sometimes omits object end markers in AMF
//! - Sends @setDataFrame with onMetaData as nested name
//!
//! ## ffmpeg
//! - Uses different transaction IDs than expected
//! - May send createStream before connect response
//! - Duration in metadata may be 0 for live streams
//!
//! ## Flash Media Encoder
//! - Uses legacy AMF0 object encoding
//! - May send duplicate metadata
//!
//! ## Wirecast
//! - Sends multiple audio/video sequence headers
//! - May have timestamp discontinuities

use crate::protocol::message::Command;

/// Configuration for handling encoder quirks
#[derive(Debug, Clone)]
pub struct QuirksConfig {
    /// Accept commands before handshake completes
    pub allow_early_commands: bool,

    /// Accept FCPublish/releaseStream before connect
    pub allow_fc_before_connect: bool,

    /// Accept malformed AMF (missing end markers)
    pub lenient_amf: bool,

    /// Accept timestamp regression
    pub allow_timestamp_regression: bool,

    /// Accept duplicate metadata
    pub allow_duplicate_metadata: bool,

    /// Accept empty app names
    pub allow_empty_app: bool,

    /// Accept oversized chunks (larger than negotiated)
    pub allow_oversized_chunks: bool,
}

impl Default for QuirksConfig {
    fn default() -> Self {
        Self {
            // Default to lenient for maximum compatibility
            allow_early_commands: true,
            allow_fc_before_connect: true,
            lenient_amf: true,
            allow_timestamp_regression: true,
            allow_duplicate_metadata: true,
            allow_empty_app: true,
            allow_oversized_chunks: true,
        }
    }
}

impl QuirksConfig {
    /// Strict mode - reject non-conformant streams
    pub fn strict() -> Self {
        Self {
            allow_early_commands: false,
            allow_fc_before_connect: false,
            lenient_amf: false,
            allow_timestamp_regression: false,
            allow_duplicate_metadata: false,
            allow_empty_app: false,
            allow_oversized_chunks: false,
        }
    }
}

/// Detected encoder type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderType {
    Unknown,
    Obs,
    Ffmpeg,
    Wirecast,
    FlashMediaEncoder,
    Xsplit,
    Larix,
    Other,
}

impl EncoderType {
    /// Detect encoder from connect command's flashVer
    pub fn from_flash_ver(flash_ver: &str) -> Self {
        let lower = flash_ver.to_lowercase();

        if lower.contains("obs") {
            EncoderType::Obs
        } else if lower.contains("fmle") || lower.contains("flash media") {
            EncoderType::FlashMediaEncoder
        } else if lower.contains("wirecast") {
            EncoderType::Wirecast
        } else if lower.contains("xsplit") {
            EncoderType::Xsplit
        } else if lower.contains("larix") {
            EncoderType::Larix
        } else if lower.contains("lavf") || lower.contains("librtmp") {
            EncoderType::Ffmpeg
        } else {
            EncoderType::Other
        }
    }
}

/// OBS/Twitch command sequence helper
///
/// Many streaming platforms expect a specific command sequence:
/// 1. connect -> _result
/// 2. releaseStream (optional)
/// 3. FCPublish
/// 4. createStream -> _result
/// 5. publish -> onStatus
pub struct CommandSequence {
    state: CommandSequenceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandSequenceState {
    Initial,
    Connected,
    StreamCreated,
    Publishing,
    Playing,
}

impl CommandSequence {
    pub fn new() -> Self {
        Self {
            state: CommandSequenceState::Initial,
        }
    }

    /// Check if a command is valid in the current state
    pub fn is_valid_command(&self, cmd: &Command) -> bool {
        match cmd.name.as_str() {
            "connect" => self.state == CommandSequenceState::Initial,
            "releaseStream" | "FCPublish" => {
                // These can come before or after connect completes (OBS quirk)
                true
            }
            "createStream" => {
                self.state == CommandSequenceState::Connected
                    || self.state == CommandSequenceState::Initial // OBS quirk
            }
            "publish" => self.state == CommandSequenceState::StreamCreated,
            "play" => self.state == CommandSequenceState::StreamCreated,
            "FCUnpublish" | "deleteStream" | "closeStream" => {
                self.state == CommandSequenceState::Publishing
                    || self.state == CommandSequenceState::Playing
            }
            _ => true, // Allow unknown commands
        }
    }

    /// Transition state based on command response
    pub fn on_command(&mut self, cmd_name: &str) {
        match cmd_name {
            "connect" => self.state = CommandSequenceState::Connected,
            "createStream" => self.state = CommandSequenceState::StreamCreated,
            "publish" => self.state = CommandSequenceState::Publishing,
            "play" => self.state = CommandSequenceState::Playing,
            "FCUnpublish" | "deleteStream" | "closeStream" => {
                self.state = CommandSequenceState::Connected;
            }
            _ => {}
        }
    }

    /// Get current state
    pub fn state(&self) -> &'static str {
        match self.state {
            CommandSequenceState::Initial => "initial",
            CommandSequenceState::Connected => "connected",
            CommandSequenceState::StreamCreated => "stream_created",
            CommandSequenceState::Publishing => "publishing",
            CommandSequenceState::Playing => "playing",
        }
    }
}

impl Default for CommandSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalize timestamp to handle regression
///
/// Some encoders have timestamp discontinuities or regressions.
/// This function adjusts timestamps to be monotonically increasing.
pub struct TimestampNormalizer {
    last_timestamp: u32,
    offset: u32,
}

impl TimestampNormalizer {
    pub fn new() -> Self {
        Self {
            last_timestamp: 0,
            offset: 0,
        }
    }

    /// Normalize a timestamp, handling regression
    pub fn normalize(&mut self, timestamp: u32) -> u32 {
        // Check for significant regression (more than 1 second)
        if timestamp < self.last_timestamp && self.last_timestamp - timestamp > 1000 {
            // Timestamp regressed significantly, adjust offset
            self.offset = self.last_timestamp + 1;
        }

        let normalized = timestamp.wrapping_add(self.offset);
        self.last_timestamp = normalized;
        normalized
    }

    /// Reset normalizer state
    pub fn reset(&mut self) {
        self.last_timestamp = 0;
        self.offset = 0;
    }
}

impl Default for TimestampNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amf::AmfValue;

    #[test]
    fn test_encoder_detection() {
        assert_eq!(
            EncoderType::from_flash_ver("OBS-Studio/29.1.3"),
            EncoderType::Obs
        );
        assert_eq!(
            EncoderType::from_flash_ver("FMLE/3.0"),
            EncoderType::FlashMediaEncoder
        );
        assert_eq!(
            EncoderType::from_flash_ver("Lavf58.76.100"),
            EncoderType::Ffmpeg
        );
    }

    #[test]
    fn test_timestamp_normalizer() {
        let mut normalizer = TimestampNormalizer::new();

        assert_eq!(normalizer.normalize(0), 0);
        assert_eq!(normalizer.normalize(1000), 1000);
        assert_eq!(normalizer.normalize(2000), 2000);

        // Small regression (within 1 second) - allow it (no offset adjustment)
        assert_eq!(normalizer.normalize(1500), 1500);

        // Large regression (> 1 second) - adjust offset
        // last_timestamp is now 1500, regression to 100 is 1400ms > 1000ms
        // So offset becomes 1500 + 1 = 1501
        // Result = 100 + 1501 = 1601
        assert_eq!(normalizer.normalize(100), 1601);
    }

    #[test]
    fn test_quirks_config_default() {
        let config = QuirksConfig::default();

        // Default should be lenient
        assert!(config.allow_early_commands);
        assert!(config.allow_fc_before_connect);
        assert!(config.lenient_amf);
        assert!(config.allow_timestamp_regression);
        assert!(config.allow_duplicate_metadata);
        assert!(config.allow_empty_app);
        assert!(config.allow_oversized_chunks);
    }

    #[test]
    fn test_quirks_config_strict() {
        let config = QuirksConfig::strict();

        // Strict mode should reject everything
        assert!(!config.allow_early_commands);
        assert!(!config.allow_fc_before_connect);
        assert!(!config.lenient_amf);
        assert!(!config.allow_timestamp_regression);
        assert!(!config.allow_duplicate_metadata);
        assert!(!config.allow_empty_app);
        assert!(!config.allow_oversized_chunks);
    }

    #[test]
    fn test_encoder_type_detection() {
        // OBS detection
        assert_eq!(
            EncoderType::from_flash_ver("OBS-Studio/29.1.3"),
            EncoderType::Obs
        );
        assert_eq!(EncoderType::from_flash_ver("obs studio"), EncoderType::Obs);

        // ffmpeg detection
        assert_eq!(
            EncoderType::from_flash_ver("Lavf58.76.100"),
            EncoderType::Ffmpeg
        );
        assert_eq!(
            EncoderType::from_flash_ver("librtmp 2.4"),
            EncoderType::Ffmpeg
        );

        // FMLE detection
        assert_eq!(
            EncoderType::from_flash_ver("FMLE/3.0"),
            EncoderType::FlashMediaEncoder
        );
        assert_eq!(
            EncoderType::from_flash_ver("Flash Media Encoder"),
            EncoderType::FlashMediaEncoder
        );

        // Wirecast detection
        assert_eq!(
            EncoderType::from_flash_ver("Wirecast/13.1"),
            EncoderType::Wirecast
        );

        // XSplit detection
        assert_eq!(
            EncoderType::from_flash_ver("XSplit/4.0"),
            EncoderType::Xsplit
        );

        // Larix detection
        assert_eq!(
            EncoderType::from_flash_ver("Larix Broadcaster"),
            EncoderType::Larix
        );

        // Unknown/Other
        assert_eq!(
            EncoderType::from_flash_ver("SomeOtherEncoder"),
            EncoderType::Other
        );
        assert_eq!(EncoderType::from_flash_ver(""), EncoderType::Other);
    }

    #[test]
    fn test_encoder_type_case_insensitive() {
        assert_eq!(EncoderType::from_flash_ver("OBS"), EncoderType::Obs);
        assert_eq!(EncoderType::from_flash_ver("obs"), EncoderType::Obs);
        assert_eq!(EncoderType::from_flash_ver("LAVF"), EncoderType::Ffmpeg);
        assert_eq!(EncoderType::from_flash_ver("lavf"), EncoderType::Ffmpeg);
    }

    #[test]
    fn test_command_sequence_new() {
        let seq = CommandSequence::new();
        assert_eq!(seq.state(), "initial");
    }

    #[test]
    fn test_command_sequence_default() {
        let seq = CommandSequence::default();
        assert_eq!(seq.state(), "initial");
    }

    #[test]
    fn test_command_sequence_connect() {
        let mut seq = CommandSequence::new();

        // Connect valid in initial state
        let cmd = Command {
            name: "connect".to_string(),
            transaction_id: 1.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 0,
        };
        assert!(seq.is_valid_command(&cmd));

        // Transition to connected
        seq.on_command("connect");
        assert_eq!(seq.state(), "connected");

        // Connect no longer valid
        assert!(!seq.is_valid_command(&cmd));
    }

    #[test]
    fn test_command_sequence_create_stream() {
        let mut seq = CommandSequence::new();

        let create_stream = Command {
            name: "createStream".to_string(),
            transaction_id: 2.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 0,
        };

        // createStream valid in initial state (OBS quirk)
        assert!(seq.is_valid_command(&create_stream));

        // Transition to connected
        seq.on_command("connect");
        assert!(seq.is_valid_command(&create_stream));

        // Transition to stream created
        seq.on_command("createStream");
        assert_eq!(seq.state(), "stream_created");
    }

    #[test]
    fn test_command_sequence_publish() {
        let mut seq = CommandSequence::new();

        let publish = Command {
            name: "publish".to_string(),
            transaction_id: 0.0,
            command_object: AmfValue::Null,
            arguments: vec![AmfValue::String("stream_key".into())],
            stream_id: 1,
        };

        // publish not valid before stream created
        assert!(!seq.is_valid_command(&publish));

        seq.on_command("connect");
        assert!(!seq.is_valid_command(&publish));

        seq.on_command("createStream");
        assert!(seq.is_valid_command(&publish));

        seq.on_command("publish");
        assert_eq!(seq.state(), "publishing");
    }

    #[test]
    fn test_command_sequence_play() {
        let mut seq = CommandSequence::new();

        let play = Command {
            name: "play".to_string(),
            transaction_id: 0.0,
            command_object: AmfValue::Null,
            arguments: vec![AmfValue::String("stream_name".into())],
            stream_id: 1,
        };

        seq.on_command("connect");
        seq.on_command("createStream");

        assert!(seq.is_valid_command(&play));
        seq.on_command("play");
        assert_eq!(seq.state(), "playing");
    }

    #[test]
    fn test_command_sequence_fc_commands_always_valid() {
        let mut seq = CommandSequence::new();

        let release_stream = Command {
            name: "releaseStream".to_string(),
            transaction_id: 2.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 0,
        };

        let fc_publish = Command {
            name: "FCPublish".to_string(),
            transaction_id: 3.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 0,
        };

        // Should be valid in initial state (OBS quirk)
        assert!(seq.is_valid_command(&release_stream));
        assert!(seq.is_valid_command(&fc_publish));

        seq.on_command("connect");
        assert!(seq.is_valid_command(&release_stream));
        assert!(seq.is_valid_command(&fc_publish));
    }

    #[test]
    fn test_command_sequence_close_commands() {
        let mut seq = CommandSequence::new();

        let fc_unpublish = Command {
            name: "FCUnpublish".to_string(),
            transaction_id: 0.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 1,
        };

        let delete_stream = Command {
            name: "deleteStream".to_string(),
            transaction_id: 4.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 1,
        };

        // Not valid until publishing/playing
        assert!(!seq.is_valid_command(&fc_unpublish));
        assert!(!seq.is_valid_command(&delete_stream));

        seq.on_command("connect");
        seq.on_command("createStream");
        seq.on_command("publish");

        // Now valid
        assert!(seq.is_valid_command(&fc_unpublish));
        assert!(seq.is_valid_command(&delete_stream));

        // After close, return to connected
        seq.on_command("deleteStream");
        assert_eq!(seq.state(), "connected");
    }

    #[test]
    fn test_command_sequence_unknown_command_always_valid() {
        let seq = CommandSequence::new();

        let unknown = Command {
            name: "unknownCommand".to_string(),
            transaction_id: 0.0,
            command_object: AmfValue::Null,
            arguments: vec![],
            stream_id: 0,
        };

        // Unknown commands should be allowed
        assert!(seq.is_valid_command(&unknown));
    }

    #[test]
    fn test_timestamp_normalizer_reset() {
        let mut normalizer = TimestampNormalizer::new();

        normalizer.normalize(1000);
        normalizer.normalize(2000);

        normalizer.reset();

        // After reset, should behave like new
        assert_eq!(normalizer.normalize(0), 0);
        assert_eq!(normalizer.normalize(100), 100);
    }

    #[test]
    fn test_timestamp_normalizer_multiple_regressions() {
        let mut normalizer = TimestampNormalizer::new();

        // First regression
        normalizer.normalize(0);
        normalizer.normalize(5000);
        let after_first = normalizer.normalize(100); // regression > 1s
        assert!(after_first > 5000);

        // Continue normal
        let next = normalizer.normalize(200);
        assert!(next > after_first);
    }

    #[test]
    fn test_timestamp_normalizer_default() {
        let normalizer = TimestampNormalizer::default();
        assert_eq!(normalizer.last_timestamp, 0);
        assert_eq!(normalizer.offset, 0);
    }

    #[test]
    fn test_encoder_type_equality() {
        assert_eq!(EncoderType::Obs, EncoderType::Obs);
        assert_ne!(EncoderType::Obs, EncoderType::Ffmpeg);
        assert_ne!(EncoderType::Unknown, EncoderType::Other);
    }
}
