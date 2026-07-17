use thiserror::Error;

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("relay unavailable: {0}")]
    RelayUnavailable(String),

    #[error("localhost unavailable on port {port}: {reason}")]
    LocalhostDown { port: u16, reason: String },

    #[error("session expired or invalid")]
    SessionExpired,

    #[error("protocol error: {0}")]
    ProtocolError(String),

    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
