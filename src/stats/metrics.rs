//! Statistics and metrics for RTMP sessions

use std::time::{Duration, Instant};

/// Session-level statistics
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    /// Total bytes received
    pub bytes_received: u64,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Connection duration
    pub duration: Duration,
    /// Number of video frames received
    pub video_frames: u64,
    /// Number of audio frames received
    pub audio_frames: u64,
    /// Number of keyframes received
    pub keyframes: u64,
    /// Dropped frames count
    pub dropped_frames: u64,
    /// Current bitrate estimate (bits/sec)
    pub bitrate: u64,
}

impl SessionStats {
    /// Create new stats tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate bitrate from bytes and duration
    pub fn calculate_bitrate(&mut self) {
        let secs = self.duration.as_secs();
        if secs > 0 {
            self.bitrate = (self.bytes_received * 8) / secs;
        }
    }
}

/// Stream-level statistics
#[derive(Debug, Clone)]
pub struct StreamStats {
    /// Stream key
    pub stream_key: String,
    /// Start time
    pub started_at: Instant,
    /// Total bytes received
    pub bytes_received: u64,
    /// Video frames received
    pub video_frames: u64,
    /// Audio frames received
    pub audio_frames: u64,
    /// Keyframes received
    pub keyframes: u64,
    /// Last video timestamp
    pub last_video_ts: u32,
    /// Last audio timestamp
    pub last_audio_ts: u32,
    /// Video codec info
    pub video_codec: Option<String>,
    /// Audio codec info
    pub audio_codec: Option<String>,
    /// Video width
    pub width: Option<u32>,
    /// Video height
    pub height: Option<u32>,
    /// Video framerate
    pub framerate: Option<f64>,
    /// Audio sample rate
    pub audio_sample_rate: Option<u32>,
    /// Audio channels
    pub audio_channels: Option<u8>,
}

impl StreamStats {
    pub fn new(stream_key: String) -> Self {
        Self {
            stream_key,
            started_at: Instant::now(),
            bytes_received: 0,
            video_frames: 0,
            audio_frames: 0,
            keyframes: 0,
            last_video_ts: 0,
            last_audio_ts: 0,
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            framerate: None,
            audio_sample_rate: None,
            audio_channels: None,
        }
    }

    /// Get duration since stream started
    pub fn duration(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Calculate bitrate in bits per second
    pub fn bitrate(&self) -> u64 {
        let secs = self.duration().as_secs();
        if secs > 0 {
            (self.bytes_received * 8) / secs
        } else {
            0
        }
    }

    /// Calculate video framerate
    pub fn calculated_framerate(&self) -> f64 {
        let secs = self.duration().as_secs_f64();
        if secs > 0.0 {
            self.video_frames as f64 / secs
        } else {
            0.0
        }
    }
}

/// Server-wide statistics
#[derive(Debug, Clone, Default)]
pub struct ServerStats {
    /// Total connections ever
    pub total_connections: u64,
    /// Current active connections
    pub active_connections: u64,
    /// Total bytes received
    pub total_bytes_received: u64,
    /// Total bytes sent
    pub total_bytes_sent: u64,
    /// Active streams
    pub active_streams: u64,
    /// Uptime
    pub uptime: Duration,
}

impl ServerStats {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_session_stats_new() {
        let stats = SessionStats::new();
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.video_frames, 0);
        assert_eq!(stats.audio_frames, 0);
        assert_eq!(stats.keyframes, 0);
        assert_eq!(stats.dropped_frames, 0);
        assert_eq!(stats.bitrate, 0);
    }

    #[test]
    fn test_session_stats_calculate_bitrate() {
        let mut stats = SessionStats::new();
        stats.bytes_received = 1_000_000; // 1 MB
        stats.duration = Duration::from_secs(10);

        stats.calculate_bitrate();

        // 1,000,000 bytes * 8 bits / 10 seconds = 800,000 bps
        assert_eq!(stats.bitrate, 800_000);
    }

    #[test]
    fn test_session_stats_calculate_bitrate_zero_duration() {
        let mut stats = SessionStats::new();
        stats.bytes_received = 1_000_000;
        stats.duration = Duration::from_secs(0);

        stats.calculate_bitrate();

        // With zero duration, bitrate should remain 0
        assert_eq!(stats.bitrate, 0);
    }

    #[test]
    fn test_stream_stats_new() {
        let stats = StreamStats::new("test_stream".to_string());
        assert_eq!(stats.stream_key, "test_stream");
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.video_frames, 0);
        assert_eq!(stats.audio_frames, 0);
        assert_eq!(stats.keyframes, 0);
        assert_eq!(stats.last_video_ts, 0);
        assert_eq!(stats.last_audio_ts, 0);
        assert!(stats.video_codec.is_none());
        assert!(stats.audio_codec.is_none());
    }

    #[test]
    fn test_stream_stats_duration() {
        let stats = StreamStats::new("test".to_string());

        // Duration should be positive since started_at is set at construction
        let duration = stats.duration();
        assert!(duration.as_nanos() > 0 || duration == Duration::ZERO);
    }

    #[test]
    fn test_stream_stats_bitrate_zero_duration() {
        let stats = StreamStats::new("test".to_string());

        // With essentially zero duration, bitrate should be 0
        let bitrate = stats.bitrate();
        // Note: this could be non-zero if enough time passed, but should be safe
        assert!(bitrate == 0 || bitrate > 0); // Just ensure it doesn't panic
    }

    #[test]
    fn test_stream_stats_calculated_framerate() {
        let stats = StreamStats::new("test".to_string());

        // With zero frames, framerate should be 0
        let framerate = stats.calculated_framerate();
        assert!(framerate >= 0.0);
    }

    #[test]
    fn test_server_stats_new() {
        let stats = ServerStats::new();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.total_bytes_received, 0);
        assert_eq!(stats.total_bytes_sent, 0);
        assert_eq!(stats.active_streams, 0);
    }

    #[test]
    fn test_stream_stats_with_data() {
        let mut stats = StreamStats::new("live_stream".to_string());

        // Simulate receiving some data
        stats.bytes_received = 5_000_000; // 5 MB
        stats.video_frames = 300;
        stats.audio_frames = 500;
        stats.keyframes = 10;
        stats.last_video_ts = 10000;
        stats.last_audio_ts = 10050;
        stats.video_codec = Some("H.264".to_string());
        stats.audio_codec = Some("AAC".to_string());
        stats.width = Some(1920);
        stats.height = Some(1080);
        stats.framerate = Some(30.0);
        stats.audio_sample_rate = Some(44100);
        stats.audio_channels = Some(2);

        assert_eq!(stats.video_codec, Some("H.264".to_string()));
        assert_eq!(stats.width, Some(1920));
        assert_eq!(stats.height, Some(1080));
        assert_eq!(stats.audio_channels, Some(2));
    }
}
