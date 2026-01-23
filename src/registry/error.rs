//! Registry error types
//!
//! Error types for stream registry operations.

use super::frame::StreamKey;

/// Error type for registry operations
#[derive(Debug, Clone)]
pub enum RegistryError {
    /// Stream not found
    StreamNotFound(StreamKey),
    /// Stream already has a publisher
    StreamAlreadyPublishing(StreamKey),
    /// Publisher ID mismatch
    PublisherMismatch,
    /// Stream is not active (e.g., in grace period without publisher)
    StreamNotActive(StreamKey),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::StreamNotFound(key) => write!(f, "Stream not found: {}", key),
            RegistryError::StreamAlreadyPublishing(key) => {
                write!(f, "Stream already has a publisher: {}", key)
            }
            RegistryError::PublisherMismatch => write!(f, "Publisher ID mismatch"),
            RegistryError::StreamNotActive(key) => write!(f, "Stream not active: {}", key),
        }
    }
}

impl std::error::Error for RegistryError {}
