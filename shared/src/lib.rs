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

/// Security constants
/// Minimum token length (bytes)
pub const MIN_TOKEN_LENGTH: usize = 32;

/// HMAC key length for frame integrity (32 bytes = 256 bits)
pub const HMAC_KEY_LENGTH: usize = 32;

/// Maximum headers per request to prevent DOS
pub const MAX_HEADERS_PER_REQUEST: usize = 128;

/// Maximum header name length (bytes)
pub const MAX_HEADER_NAME_LEN: usize = 256;

/// Maximum header value length (bytes)
pub const MAX_HEADER_VALUE_LEN: usize = 8192;

/// Request rate limit per client: max requests per second
pub const RATE_LIMIT_PER_CLIENT: u32 = 1000;

/// Global rate limit: max requests per second across all clients
pub const GLOBAL_RATE_LIMIT: u32 = 10000;

/// Nonce cache duration in seconds for replay protection
pub const NONCE_CACHE_DURATION_SECS: u64 = 300;
