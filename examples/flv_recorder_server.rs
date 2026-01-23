//! FLV Recorder Server - Records incoming RTMP streams to FLV files
//!
//! Run with: cargo run --example flv_recorder_server -- [output_dir]
//!
//! This example demonstrates:
//! - Using `RtmpHandler` to intercept incoming publish streams
//! - Recording multiple concurrent streams to separate FLV files
//! - Thread-safe state management with `Arc<Mutex<...>>`
//! - Proper resource cleanup when streams end
//!
//! For client-side recording, see `flv_recorder_client.rs`.
//!
//! # Architecture
//!
//! ```text
//!                            +------------------+
//!   OBS/ffmpeg ─────────────>│  RTMP Server     │
//!      publish               │                  │
//!                            │  RecorderHandler │
//!                            │    │             │
//!                            │    ▼             │
//!                            │  writers: Map    │
//!                            │    stream_key -> BufWriter<File>
//!                            +--------│---------+
//!                                     │
//!                                     ▼
//!                            +------------------+
//!                            │  test_key.flv   │
//!                            │  other_key.flv  │
//!                            +------------------+
//! ```
//!
//! # FLV File Format
//!
//! ```text
//! +============+==================+==============+==================+
//! | FLV Header | PrevTagSize0 (0) | Tag 1        | PrevTagSize1 ... |
//! | (9 bytes)  | (4 bytes)        | (11+N bytes) | (4 bytes)        |
//! +============+==================+==============+==================+
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rtmp_rs::media::{FlvTag, FlvTagType};
use rtmp_rs::protocol::message::PublishParams;
use rtmp_rs::server::handler::{AuthResult, MediaDeliveryMode, RtmpHandler};
use rtmp_rs::session::{SessionContext, StreamContext};
use rtmp_rs::{RtmpServer, ServerConfig};

// ============================================================================
// FLV Writing Utilities
// ============================================================================

/// FLV file signature: "FLV" in ASCII
const FLV_SIGNATURE: [u8; 3] = [0x46, 0x4C, 0x56];

/// FLV version (always 1)
const FLV_VERSION: u8 = 0x01;

/// Type flags: bit 0 = video, bit 2 = audio. 0x05 = both
const FLV_TYPE_FLAGS_AV: u8 = 0x05;

/// FLV header is always 9 bytes
const FLV_HEADER_SIZE: u32 = 9;

/// FLV tag type codes
const FLV_TAG_AUDIO: u8 = 8;
const FLV_TAG_VIDEO: u8 = 9;

/// Writes the FLV file header (9 bytes) plus initial PreviousTagSize0 (4 bytes)
fn write_flv_header(writer: &mut impl Write) -> std::io::Result<()> {
    writer.write_all(&FLV_SIGNATURE)?;
    writer.write_all(&[FLV_VERSION])?;
    writer.write_all(&[FLV_TYPE_FLAGS_AV])?;
    writer.write_all(&FLV_HEADER_SIZE.to_be_bytes())?;
    writer.write_all(&0u32.to_be_bytes())?; // PreviousTagSize0 = 0
    Ok(())
}

/// Writes an FLV tag with header, data, and trailing PreviousTagSize
///
/// Tag structure:
/// - Type (1B) + DataSize (3B BE) + Timestamp (3B + 1B ext) + StreamID (3B) + Data
/// - Followed by PreviousTagSize (4B BE) = 11 + data.len()
fn write_flv_tag(
    writer: &mut impl Write,
    tag_type: u8,
    timestamp: u32,
    data: &[u8],
) -> std::io::Result<()> {
    let data_size = data.len() as u32;

    // Tag type
    writer.write_all(&[tag_type])?;

    // Data size (24-bit BE)
    writer.write_all(&[
        ((data_size >> 16) & 0xFF) as u8,
        ((data_size >> 8) & 0xFF) as u8,
        (data_size & 0xFF) as u8,
    ])?;

    // Timestamp: lower 24 bits, then upper 8 bits (extension byte)
    writer.write_all(&[
        ((timestamp >> 16) & 0xFF) as u8,
        ((timestamp >> 8) & 0xFF) as u8,
        (timestamp & 0xFF) as u8,
        ((timestamp >> 24) & 0xFF) as u8,
    ])?;

    // Stream ID (always 0 in FLV files)
    writer.write_all(&[0, 0, 0])?;

    // Tag data
    writer.write_all(data)?;

    // PreviousTagSize = 11 (header) + data length
    let prev_tag_size = 11 + data_size;
    writer.write_all(&prev_tag_size.to_be_bytes())?;

    Ok(())
}

// ============================================================================
// Per-Stream Recording State
// ============================================================================

/// State for a single recording stream
struct StreamRecorder {
    writer: BufWriter<File>,
    video_tags: u64,
    audio_tags: u64,
    first_timestamp: Option<u32>,
    output_path: PathBuf,
}

impl StreamRecorder {
    /// Create a new recorder for the given stream key
    fn new(stream_key: &str, output_dir: &PathBuf) -> std::io::Result<Self> {
        // Sanitize stream key for use as filename (replace unsafe chars)
        let safe_name: String = stream_key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let output_path = output_dir.join(format!("{}.flv", safe_name));
        let file = File::create(&output_path)?;
        let mut writer = BufWriter::new(file);

        // Write FLV header
        write_flv_header(&mut writer)?;

        Ok(Self {
            writer,
            video_tags: 0,
            audio_tags: 0,
            first_timestamp: None,
            output_path,
        })
    }

    /// Write a media tag to the file, normalizing timestamps to start at 0
    fn write_tag(&mut self, tag: &FlvTag) -> std::io::Result<()> {
        // Normalize timestamp relative to first received tag
        if self.first_timestamp.is_none() {
            self.first_timestamp = Some(tag.timestamp);
        }
        let relative_ts = tag
            .timestamp
            .saturating_sub(self.first_timestamp.unwrap_or(0));

        // Convert FlvTagType to FLV tag type byte
        let tag_type = match tag.tag_type {
            FlvTagType::Audio => FLV_TAG_AUDIO,
            FlvTagType::Video => FLV_TAG_VIDEO,
            FlvTagType::Script => 18, // Script data
        };

        write_flv_tag(&mut self.writer, tag_type, relative_ts, &tag.data)?;

        // Update stats
        match tag.tag_type {
            FlvTagType::Video => self.video_tags += 1,
            FlvTagType::Audio => self.audio_tags += 1,
            FlvTagType::Script => {}
        }

        Ok(())
    }

    /// Flush and finalize the recording
    fn finish(mut self) -> std::io::Result<(PathBuf, u64, u64)> {
        self.writer.flush()?;
        Ok((self.output_path, self.video_tags, self.audio_tags))
    }
}

// ============================================================================
// Recording Handler
// ============================================================================

/// RTMP handler that records incoming streams to FLV files
///
/// This handler demonstrates:
/// - Per-stream state management using a HashMap
/// - Thread-safe access with Arc<Mutex<...>>
/// - Creating files on publish, closing on stream end
struct RecorderHandler {
    /// Output directory for FLV files
    output_dir: PathBuf,

    /// Active recorders keyed by stream key
    /// Using Mutex for interior mutability (handler methods take &self)
    recorders: Mutex<HashMap<String, StreamRecorder>>,
}

impl RecorderHandler {
    fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            recorders: Mutex::new(HashMap::new()),
        }
    }
}

impl RtmpHandler for RecorderHandler {
    /// Called when a client starts publishing
    ///
    /// Create a new FLV file for this stream
    async fn on_publish(&self, ctx: &SessionContext, params: &PublishParams) -> AuthResult {
        println!(
            "[{}] New publish: stream_key='{}'",
            ctx.session_id, params.stream_key
        );

        // Create a new recorder for this stream
        match StreamRecorder::new(&params.stream_key, &self.output_dir) {
            Ok(recorder) => {
                println!(
                    "[{}] Recording to: {}",
                    ctx.session_id,
                    recorder.output_path.display()
                );

                let mut recorders = self.recorders.lock().unwrap();
                recorders.insert(params.stream_key.clone(), recorder);

                AuthResult::Accept
            }
            Err(e) => {
                eprintln!(
                    "[{}] Failed to create recording file: {}",
                    ctx.session_id, e
                );
                AuthResult::Reject(format!("Recording failed: {}", e))
            }
        }
    }

    /// Called for each media tag (audio/video)
    ///
    /// Write the tag to the appropriate FLV file
    async fn on_media_tag(&self, ctx: &StreamContext, tag: &FlvTag) -> bool {
        let mut recorders = self.recorders.lock().unwrap();

        if let Some(recorder) = recorders.get_mut(&ctx.stream_key) {
            if let Err(e) = recorder.write_tag(tag) {
                eprintln!(
                    "[{}] Write error for '{}': {}",
                    ctx.session.session_id, ctx.stream_key, e
                );
                return false; // Signal to stop processing
            }

            // Log keyframes for progress indication
            if tag.is_keyframe() && recorder.video_tags % 30 == 1 {
                println!(
                    "[{}] '{}': {} video, {} audio tags",
                    ctx.session.session_id,
                    ctx.stream_key,
                    recorder.video_tags,
                    recorder.audio_tags
                );
            }
        }

        true // Continue processing
    }

    /// Called when publishing stops
    ///
    /// Finalize the FLV file
    async fn on_publish_stop(&self, ctx: &StreamContext) {
        let mut recorders = self.recorders.lock().unwrap();

        if let Some(recorder) = recorders.remove(&ctx.stream_key) {
            match recorder.finish() {
                Ok((path, video, audio)) => {
                    println!(
                        "[{}] Recording complete: {} ({} video, {} audio tags)",
                        ctx.session.session_id,
                        path.display(),
                        video,
                        audio
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[{}] Error finalizing '{}': {}",
                        ctx.session.session_id, ctx.stream_key, e
                    );
                }
            }
        }
    }

    /// Use RawFlv mode since we only need raw tags for recording
    ///
    /// This is more efficient than parsing frames we don't need
    fn media_delivery_mode(&self) -> MediaDeliveryMode {
        MediaDeliveryMode::RawFlv
    }

    /// Log new connections
    async fn on_connection(&self, ctx: &SessionContext) -> bool {
        println!("[{}] Connection from {}", ctx.session_id, ctx.peer_addr);
        true
    }

    /// Log disconnections
    async fn on_disconnect(&self, ctx: &SessionContext) {
        println!("[{}] Disconnected", ctx.session_id);
    }
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rtmp_rs=info".parse()?)
                .add_directive("flv_recorder=info".parse()?),
        )
        .init();

    // Output directory for recordings (current directory by default)
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Ensure output directory exists
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir)?;
    }

    let config = ServerConfig::default();

    println!("RTMP Recording Server");
    println!("=====================");
    println!("Listening on: {}", config.bind_addr);
    println!("Output dir:   {}", output_dir.display());
    println!();
    println!("Publish a stream to start recording:");
    println!("  ffmpeg -re -i input.mp4 -c copy -f flv rtmp://localhost/live/test_key");
    println!("  -> Creates: {}/test_key.flv", output_dir.display());
    println!();
    println!("Press Ctrl+C to stop the server...");
    println!();

    // Create handler and server
    let handler = RecorderHandler::new(output_dir);
    let server = RtmpServer::new(config, handler);
    let server = Arc::new(server);

    // Run until Ctrl+C
    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                eprintln!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down...");
        }
    }

    Ok(())
}
