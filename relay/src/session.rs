use std::sync::OnceLock;
use std::time::Instant;

use dashmap::DashMap;
use protocol::TunnelFrame;
use tokio::sync::mpsc;
use uuid::Uuid;

static SESSIONS: OnceLock<DashMap<String, Session>> = OnceLock::new();

pub fn sessions() -> &'static DashMap<String, Session> {
    SESSIONS.get_or_init(DashMap::new)
}

pub struct Session {
    pub id: Uuid,
    pub subdomain: String,
    #[allow(dead_code)]
    pub token: String,
    pub client_tx: mpsc::Sender<TunnelFrame>,
    pub created_at: Instant,
    pub last_heartbeat: Instant,
    /// Tunnel duration limit. `None` means no expiry (server default).
    pub duration_secs: Option<u64>,
}

impl Session {
    pub fn touch_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
}

pub fn remove_session(subdomain: &str) {
    if let Some((_, session)) = sessions().remove(subdomain) {
        tracing::info!(
            subdomain,
            session_id = %session.id,
            duration_secs = session.created_at.elapsed().as_secs(),
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
    sessions().insert(subdomain.clone(), session);
    tracing::info!(subdomain, %session_id, "session created");
}

pub fn session_count() -> usize {
    sessions().len()
}
