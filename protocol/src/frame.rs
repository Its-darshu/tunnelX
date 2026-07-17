use serde::{Deserialize, Serialize};

/// Wire protocol version sent during registration.
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum chunk size for streaming HTTP bodies (64 KB).
pub const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TunnelFrame {
    // Session lifecycle
    Register {
        token: String,
        version: u8,
        /// Client-requested subdomain (None = auto-generate).
        requested_subdomain: Option<String>,
        /// Requested tunnel duration in seconds (None = server default).
        duration_secs: Option<u64>,
    },
    Registered {
        subdomain: String,
        public_url: String,
        /// How many seconds until this tunnel expires.
        expires_in_secs: Option<u64>,
    },
    Heartbeat,
    HeartbeatAck,
    Disconnect,

    // HTTP proxying
    HttpRequest {
        request_id: u64,
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    HttpResponseHeader {
        request_id: u64,
        status: u16,
        headers: Vec<(String, String)>,
    },
    HttpResponseBody {
        request_id: u64,
        chunk: Vec<u8>,
        finished: bool,
    },

    // WebSocket upgrade
    WsOpen {
        request_id: u64,
        path: String,
        headers: Vec<(String, String)>,
    },
    WsFrame {
        request_id: u64,
        data: Vec<u8>,
        opcode: u8,
    },
    WsClose {
        request_id: u64,
    },

    // Errors
    Error {
        code: u16,
        message: String,
    },
}

impl TunnelFrame {
    pub fn register(token: String) -> Self {
        Self::Register {
            token,
            version: PROTOCOL_VERSION,
            requested_subdomain: None,
            duration_secs: None,
        }
    }

    pub fn register_with_options(
        token: String,
        subdomain: Option<String>,
        duration_secs: Option<u64>,
    ) -> Self {
        Self::Register {
            token,
            version: PROTOCOL_VERSION,
            requested_subdomain: subdomain,
            duration_secs,
        }
    }

    pub fn error(code: u16, message: impl Into<String>) -> Self {
        Self::Error {
            code,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{decode_frame, encode_frame};

    #[test]
    fn roundtrip_register() {
        let frame = TunnelFrame::register("abc123".into());
        let bytes = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn roundtrip_register_with_subdomain() {
        let frame =
            TunnelFrame::register_with_options("tok".into(), Some("my-app".into()), Some(300));
        let bytes = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }

    #[test]
    fn roundtrip_http_request() {
        let frame = TunnelFrame::HttpRequest {
            request_id: 42,
            method: "GET".into(),
            path: "/api/users".into(),
            headers: vec![("Host".into(), "localhost".into())],
            body: vec![],
        };
        let bytes = encode_frame(&frame).unwrap();
        let decoded = decode_frame(&bytes).unwrap();
        assert_eq!(frame, decoded);
    }
}
