//! Handler context
//!
//! Context passed to handler callbacks containing session information
//! and methods to interact with the connection.

use std::net::SocketAddr;
use std::sync::Arc;

use crate::protocol::message::ConnectParams;
use crate::protocol::quirks::EncoderType;
use crate::stats::SessionStats;

/// Context passed to RtmpHandler callbacks
///
/// Provides read-only access to session information. For operations
/// that modify state, use the return values from handler methods.
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// Unique session ID
    pub session_id: u64,

    /// Remote peer address
    pub peer_addr: SocketAddr,

    /// Application name (from connect)
    pub app: String,

    /// Detected encoder type
    pub encoder_type: EncoderType,

    /// Connect parameters (if available)
    pub connect_params: Option<Arc<ConnectParams>>,

    /// Current session statistics
    pub stats: SessionStats,
}

impl SessionContext {
    /// Create a new context
    pub fn new(session_id: u64, peer_addr: SocketAddr) -> Self {
        Self {
            session_id,
            peer_addr,
            app: String::new(),
            encoder_type: EncoderType::Unknown,
            connect_params: None,
            stats: SessionStats::default(),
        }
    }

    /// Update with connect parameters
    pub fn with_connect(&mut self, params: ConnectParams, encoder_type: EncoderType) {
        self.app = params.app.clone();
        self.encoder_type = encoder_type;
        self.connect_params = Some(Arc::new(params));
    }

    /// Get the TC URL if available
    pub fn tc_url(&self) -> Option<&str> {
        self.connect_params
            .as_ref()
            .and_then(|p| p.tc_url.as_deref())
    }

    /// Get the page URL if available
    pub fn page_url(&self) -> Option<&str> {
        self.connect_params
            .as_ref()
            .and_then(|p| p.page_url.as_deref())
    }

    /// Get the flash version string if available
    pub fn flash_ver(&self) -> Option<&str> {
        self.connect_params
            .as_ref()
            .and_then(|p| p.flash_ver.as_deref())
    }
}

/// Stream context passed to media callbacks
#[derive(Debug, Clone)]
pub struct StreamContext {
    /// Parent session context
    pub session: SessionContext,

    /// Message stream ID
    pub stream_id: u32,

    /// Stream key
    pub stream_key: String,

    /// Whether this is a publishing or playing stream
    pub is_publishing: bool,
}

impl StreamContext {
    /// Create a new stream context
    pub fn new(session: SessionContext, stream_id: u32, stream_key: String, is_publishing: bool) -> Self {
        Self {
            session,
            stream_id,
            stream_key,
            is_publishing,
        }
    }
}
