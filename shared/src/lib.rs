pub mod config;
pub mod error;

pub use config::{ClientConfig, RelayConfig, DEFAULT_RELAY_URL};
pub use error::TunnelError;

/// Heartbeat interval in seconds.
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Session timeout if no heartbeat received (seconds).
pub const SESSION_TIMEOUT_SECS: u64 = 90;

/// Maximum request/response body size (10 MB).
pub const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Heartbeat ack timeout in seconds.
pub const HEARTBEAT_ACK_TIMEOUT_SECS: u64 = 10;

/// Maximum reconnect backoff in seconds.
pub const MAX_RECONNECT_BACKOFF_SECS: u64 = 30;

/// Initial reconnect backoff in seconds.
pub const INITIAL_RECONNECT_BACKOFF_SECS: u64 = 1;

/// URL generation retry limit.
pub const URL_GEN_MAX_RETRIES: u32 = 5;
