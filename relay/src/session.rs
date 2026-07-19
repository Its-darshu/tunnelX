use std::sync::OnceLock;
use std::time::Instant;

use dashmap::DashMap;
use protocol::TunnelFrame;
use tokio::sync::mpsc;
use uuid::Uuid;

static SESSIONS: OnceLock<DashMap<String, Session>> = OnceLock::new();
/// Token to session_id mapping for verification (token_hash -> session_id)
static TOKEN_INDEX: OnceLock<DashMap<String, String>> = OnceLock::new();

pub fn sessions() -> &'static DashMap<String, Session> {
    SESSIONS.get_or_init(DashMap::new)
}

pub fn token_index() -> &'static DashMap<String, String> {
    TOKEN_INDEX.get_or_init(DashMap::new)
}

pub struct Session {
    pub id: Uuid,
    pub subdomain: String,
    /// HMAC-SHA256 hash of the token for secure verification
    pub token_hash: String,
    pub client_tx: mpsc::Sender<TunnelFrame>,
    pub created_at: Instant,
    pub last_heartbeat: Instant,
    pub last_request: Instant,
    pub request_count: u64,
    /// Tunnel duration limit. `None` means no expiry (server default).
    pub duration_secs: Option<u64>,
    /// Client IP for rate limiting and logging
    pub client_addr: Option<String>,
}

impl Session {
    pub fn touch_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    pub fn touch_request(&mut self) {
        self.last_request = Instant::now();
        self.request_count += 1;
    }

    pub fn is_rate_limited(&self) -> bool {
        let secs_since_request = self.last_request.elapsed().as_secs();
        if secs_since_request == 0 {
            self.request_count > shared::RATE_LIMIT_PER_CLIENT as u64
        } else {
            false
        }
    }
}

pub fn remove_session(subdomain: &str) {
    if let Some((_, session)) = sessions().remove(subdomain) {
        token_index().remove(&session.token_hash);
        tracing::info!(
            subdomain,
            session_id = %session.id,
            duration_secs = session.created_at.elapsed().as_secs(),
            requests = session.request_count,
            "session removed"
        );
    }
}

pub fn get_session(subdomain: &str) -> Option<dashmap::mapref::one::Ref<'_, String, Session>> {
    sessions().get(subdomain)
}

pub fn insert_session(session: Session) {
    let subdomain = session.subdomain.clone();
    let session_id = session.id;
    let token_hash = session.token_hash.clone();
    sessions().insert(subdomain.clone(), session);
    token_index().insert(token_hash, subdomain.clone());
    tracing::info!(subdomain, %session_id, "session created");
}

pub fn verify_and_get_session(
    subdomain: &str,
    token_hash: &str,
) -> bool {
    match sessions().get(subdomain) {
        Some(session_ref) => {
            if session_ref.token_hash == token_hash {
                true
            } else {
                tracing::warn!(subdomain, "token verification failed");
                false
            }
        }
        None => false,
    }
}

pub fn session_count() -> usize {
    sessions().len()
}
