//! Session state machine
//!
//! Tracks the overall state of an RTMP session from connection to disconnection.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use super::stream::StreamState;
use crate::protocol::message::ConnectParams;
use crate::protocol::quirks::EncoderType;

/// Session lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// TCP connected, handshake not started
    Connected,
    /// Handshake in progress
    Handshaking,
    /// Handshake complete, waiting for connect command
    WaitingConnect,
    /// Connect command received and accepted
    Active,
    /// Session is closing
    Closing,
    /// Session closed
    Closed,
}

/// Complete session state
#[derive(Debug)]
pub struct SessionState {
    /// Unique session ID
    pub id: u64,

    /// Remote peer address
    pub peer_addr: SocketAddr,

    /// Current phase
    pub phase: SessionPhase,

    /// Connection start time
    pub connected_at: Instant,

    /// Time when handshake completed
    pub handshake_completed_at: Option<Instant>,

    /// Connect parameters (after connect command)
    pub connect_params: Option<ConnectParams>,

    /// Detected encoder type
    pub encoder_type: EncoderType,

    /// Per-stream states (keyed by message stream ID)
    pub streams: HashMap<u32, StreamState>,

    /// Next message stream ID to allocate
    next_stream_id: u32,

    /// Negotiated chunk size (incoming)
    pub in_chunk_size: u32,

    /// Negotiated chunk size (outgoing)
    pub out_chunk_size: u32,

    /// Window acknowledgement size
    pub window_ack_size: u32,

    /// Bytes received since last acknowledgement
    pub bytes_received: u64,

    /// Bytes sent
    pub bytes_sent: u64,

    /// Last acknowledgement sequence
    pub last_ack_sequence: u32,
}

impl SessionState {
    /// Create a new session state
    pub fn new(id: u64, peer_addr: SocketAddr) -> Self {
        Self {
            id,
            peer_addr,
            phase: SessionPhase::Connected,
            connected_at: Instant::now(),
            handshake_completed_at: None,
            connect_params: None,
            encoder_type: EncoderType::Unknown,
            streams: HashMap::new(),
            next_stream_id: 1, // Stream 0 is reserved for NetConnection
            in_chunk_size: 128,
            out_chunk_size: 128,
            window_ack_size: 2_500_000,
            bytes_received: 0,
            bytes_sent: 0,
            last_ack_sequence: 0,
        }
    }

    /// Transition to handshaking phase
    pub fn start_handshake(&mut self) {
        if self.phase == SessionPhase::Connected {
            self.phase = SessionPhase::Handshaking;
        }
    }

    /// Complete handshake
    pub fn complete_handshake(&mut self) {
        if self.phase == SessionPhase::Handshaking {
            self.phase = SessionPhase::WaitingConnect;
            self.handshake_completed_at = Some(Instant::now());
        }
    }

    /// Handle connect command
    pub fn on_connect(&mut self, params: ConnectParams, encoder_type: EncoderType) {
        self.connect_params = Some(params);
        self.encoder_type = encoder_type;
        self.phase = SessionPhase::Active;
    }

    /// Allocate a new message stream ID
    pub fn allocate_stream_id(&mut self) -> u32 {
        let id = self.next_stream_id;
        self.next_stream_id += 1;
        self.streams.insert(id, StreamState::new(id));
        id
    }

    /// Get a stream by ID
    pub fn get_stream(&self, stream_id: u32) -> Option<&StreamState> {
        self.streams.get(&stream_id)
    }

    /// Get a mutable stream by ID
    pub fn get_stream_mut(&mut self, stream_id: u32) -> Option<&mut StreamState> {
        self.streams.get_mut(&stream_id)
    }

    /// Remove a stream
    pub fn remove_stream(&mut self, stream_id: u32) -> Option<StreamState> {
        self.streams.remove(&stream_id)
    }

    /// Update bytes received and check if acknowledgement needed
    pub fn add_bytes_received(&mut self, bytes: u64) -> bool {
        self.bytes_received += bytes;

        // Check if we need to send acknowledgement
        let delta = self.bytes_received as u32 - self.last_ack_sequence;
        delta >= self.window_ack_size
    }

    /// Mark acknowledgement sent
    pub fn mark_ack_sent(&mut self) {
        self.last_ack_sequence = self.bytes_received as u32;
    }

    /// Get session duration
    pub fn duration(&self) -> std::time::Duration {
        self.connected_at.elapsed()
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        self.phase == SessionPhase::Active
    }

    /// Start closing the session
    pub fn close(&mut self) {
        self.phase = SessionPhase::Closing;
    }

    /// Get the application name
    pub fn app(&self) -> Option<&str> {
        self.connect_params.as_ref().map(|p| p.app.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_session_lifecycle() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1935);
        let mut state = SessionState::new(1, addr);

        assert_eq!(state.phase, SessionPhase::Connected);

        state.start_handshake();
        assert_eq!(state.phase, SessionPhase::Handshaking);

        state.complete_handshake();
        assert_eq!(state.phase, SessionPhase::WaitingConnect);
        assert!(state.handshake_completed_at.is_some());

        let params = ConnectParams::default();
        state.on_connect(params, EncoderType::Obs);
        assert_eq!(state.phase, SessionPhase::Active);
        assert!(state.is_active());
    }

    #[test]
    fn test_stream_allocation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1935);
        let mut state = SessionState::new(1, addr);

        let id1 = state.allocate_stream_id();
        let id2 = state.allocate_stream_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert!(state.get_stream(1).is_some());
        assert!(state.get_stream(2).is_some());
    }
}
