use std::time::Duration;

use shared::SESSION_TIMEOUT_SECS;

use crate::session::remove_session;

pub fn spawn_cleanup_task(store: &'static dashmap::DashMap<String, crate::session::Session>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let heartbeat_timeout = Duration::from_secs(SESSION_TIMEOUT_SECS);
            let expired: Vec<String> = store
                .iter()
                .filter(|entry| {
                    let session = entry.value();
                    // Heartbeat timeout
                    if session.last_heartbeat.elapsed() > heartbeat_timeout {
                        return true;
                    }
                    // Duration-based expiry
                    if let Some(dur) = session.duration_secs {
                        if session.created_at.elapsed() > Duration::from_secs(dur) {
                            return true;
                        }
                    }
                    false
                })
                .map(|entry| entry.key().clone())
                .collect();

            for subdomain in expired {
                tracing::warn!(subdomain, "session expired");
                remove_session(&subdomain);
            }
        }
    });
}
