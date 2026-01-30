//! RTMP stream publisher
//!
//! High-level API for publishing audio (and optionally video) streams to RTMP servers.

use bytes::Bytes;
use tokio::sync::mpsc;

use crate::error::{Error, Result};

use super::config::ClientConfig;
use super::connector::RtmpConnector;

/// Events from the RTMP publisher
#[derive(Debug)]
pub enum PublishEvent {
    /// Connected and ready to publish
    Connected,

    /// Publishing started on the server
    Publishing,

    /// Error occurred
    Error(String),

    /// Disconnected
    Disconnected,
}

/// RTMP stream publisher
///
/// Publishes audio-only (or audio+video) streams to an RTMP server.
///
/// # Example
/// ```no_run
/// use rtmp_rs::client::{ClientConfig, RtmpPublisher};
///
/// # async fn example() -> rtmp_rs::error::Result<()> {
/// let config = ClientConfig::new("rtmp://localhost/live/stream_key");
/// let (mut publisher, mut events) = RtmpPublisher::new(config);
///
/// // Spawn event handler
/// tokio::spawn(async move {
///     while let Some(event) = events.recv().await {
///         println!("Event: {:?}", event);
///     }
/// });
///
/// // Connect and start publishing
/// publisher.connect().await?;
/// # Ok(())
/// # }
/// ```
pub struct RtmpPublisher {
    config: ClientConfig,
    event_tx: mpsc::Sender<PublishEvent>,
    connector: Option<RtmpConnector>,
}

impl RtmpPublisher {
    /// Create a new publisher.
    ///
    /// Returns the publisher and a receiver for events.
    pub fn new(config: ClientConfig) -> (Self, mpsc::Receiver<PublishEvent>) {
        let (tx, rx) = mpsc::channel(256);

        let publisher = Self {
            config,
            event_tx: tx,
            connector: None,
        };

        (publisher, rx)
    }

    /// Connect to the RTMP server and start publishing.
    ///
    /// After this returns successfully, you can call `send_audio()` to
    /// send audio frames.
    pub async fn connect(&mut self) -> Result<()> {
        let mut connector = RtmpConnector::connect(self.config.clone()).await?;
        let _ = self.event_tx.send(PublishEvent::Connected).await;

        let stream_name = self
            .config
            .parse_url()
            .and_then(|u| u.stream_key)
            .unwrap_or_default();

        connector.publish(&stream_name).await?;
        let _ = self.event_tx.send(PublishEvent::Publishing).await;

        self.connector = Some(connector);
        Ok(())
    }

    /// Send an AAC audio frame.
    ///
    /// The `data` should be the raw FLV audio tag body:
    /// - First byte: audio tag header (e.g., `0xAF` for AAC, 44100Hz, stereo, 16-bit)
    /// - For sequence header: `0xAF 0x00` + AudioSpecificConfig
    /// - For raw AAC frames: `0xAF 0x01` + raw AAC data (no ADTS)
    ///
    /// `timestamp` is in milliseconds.
    pub async fn send_audio(&mut self, data: Bytes, timestamp: u32) -> Result<()> {
        let connector = self
            .connector
            .as_mut()
            .ok_or_else(|| Error::Protocol(crate::error::ProtocolError::UnexpectedMessage(
                "Not connected".into(),
            )))?;

        connector.send_audio_data(data, timestamp).await
    }

    /// Send the AAC sequence header.
    ///
    /// This must be sent before any audio frames. It contains the
    /// AudioSpecificConfig that the server needs to decode the stream.
    ///
    /// `audio_specific_config` is typically 2 bytes describing the AAC profile,
    /// sample rate, and channel configuration.
    pub async fn send_aac_sequence_header(
        &mut self,
        audio_specific_config: &[u8],
    ) -> Result<()> {
        let mut data = Vec::with_capacity(2 + audio_specific_config.len());
        // FLV audio tag header: AAC (0xA=10 shifted left 4), 44100Hz (3<<2), stereo (1<<1), 16-bit (1)
        // = 0xAF
        data.push(0xAF);
        // AAC packet type: sequence header
        data.push(0x00);
        data.extend_from_slice(audio_specific_config);

        self.send_audio(Bytes::from(data), 0).await
    }

    /// Send a raw AAC audio frame (without ADTS header).
    ///
    /// `timestamp` is in milliseconds.
    pub async fn send_aac_raw(&mut self, raw_aac: Bytes, timestamp: u32) -> Result<()> {
        let mut data = Vec::with_capacity(2 + raw_aac.len());
        // FLV audio tag header: AAC
        data.push(0xAF);
        // AAC packet type: raw
        data.push(0x01);
        data.extend_from_slice(&raw_aac);

        self.send_audio(Bytes::from(data), timestamp).await
    }

    /// Disconnect from the server.
    pub async fn disconnect(&mut self) {
        self.connector.take();
        let _ = self.event_tx.send(PublishEvent::Disconnected).await;
    }

    /// Check if currently connected and publishing.
    pub fn is_connected(&self) -> bool {
        self.connector.is_some()
    }
}
