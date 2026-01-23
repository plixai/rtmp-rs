//! Stream registry for pub/sub routing
//!
//! The registry manages active streams and routes media from publishers to subscribers.
//! It uses `tokio::sync::broadcast` for efficient zero-copy fan-out to multiple subscribers.
//!
//! # Architecture
//!
//! ```text
//!                          Arc<StreamRegistry>
//!                     ┌─────────────────────────┐
//!                     │ streams: HashMap<Key,   │
//!                     │   StreamEntry {         │
//!                     │     gop_buffer,         │
//!                     │     tx: broadcast::Tx,  │
//!                     │   }                     │
//!                     │ >                       │
//!                     └───────────┬─────────────┘
//!                                 │
//!         ┌───────────────────────┼───────────────────────┐
//!         │                       │                       │
//!         ▼                       ▼                       ▼
//!    [Publisher]            [Subscriber]            [Subscriber]
//!    handle_video()         frame_rx.recv()         frame_rx.recv()
//!         │                       │                       │
//!         └──► registry.broadcast()──► send_video() ──► TCP
//! ```
//!
//! # Zero-Copy Design
//!
//! `bytes::Bytes` uses reference counting, so all subscribers share the same
//! memory allocation. The broadcast channel clones the `BroadcastFrame`, but
//! the inner `Bytes` data is only reference-counted, not copied.

pub mod config;
pub mod entry;
pub mod error;
pub mod frame;
pub mod store;

pub use config::RegistryConfig;
pub use entry::{StreamEntry, StreamState, StreamStats};
pub use error::RegistryError;
pub use frame::{BroadcastFrame, FrameType, StreamKey};
pub use store::StreamRegistry;
