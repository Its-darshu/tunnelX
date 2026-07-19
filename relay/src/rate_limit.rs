use std::sync::Arc;
use std::time::Instant;
use dashmap::DashMap;
use shared::RATE_LIMIT_PER_CLIENT;

/// Per-client rate limiter tracking requests in the current second.
pub struct RateLimiter {
    /// Maps client_id -> (timestamp, count)
    clients: Arc<DashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
        }
    }

    /// Check if a client has exceeded the rate limit.
    /// Returns true if the request should be allowed, false if rate-limited.
    pub fn check_and_update(&self, client_id: &str) -> bool {
        let now = Instant::now();
        let limit = RATE_LIMIT_PER_CLIENT;

        if let Some(mut entry) = self.clients.get_mut(client_id) {
            let (timestamp, count) = entry.value_mut();
            let elapsed = now.duration_since(*timestamp).as_secs();

            if elapsed >= 1 {
                *timestamp = now;
                *count = 1;
                true
            } else if *count < limit {
                *count += 1;
                true
            } else {
                false
            }
        } else {
            self.clients.insert(client_id.to_string(), (now, 1));
            true
        }
    }

    /// Clean up old entries (optional, call periodically).
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.clients.retain(|_, (timestamp, _)| {
            now.duration_since(*timestamp).as_secs() < 60
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_requests_below_limit() {
        let limiter = RateLimiter::new();
        for _ in 0..100 {
            assert!(limiter.check_and_update("client1"));
        }
    }

    #[test]
    fn blocks_requests_above_limit() {
        let limiter = RateLimiter::new();
        for _ in 0..RATE_LIMIT_PER_CLIENT {
            assert!(limiter.check_and_update("client2"));
        }
        assert!(!limiter.check_and_update("client2"));
    }

    #[test]
    fn resets_after_one_second() {
        let limiter = RateLimiter::new();
        limiter.clients.insert("client3".to_string(), (Instant::now(), 1000));
        assert!(!limiter.check_and_update("client3"));

        let past = Instant::now() - std::time::Duration::from_secs(1);
        if let Some(mut entry) = limiter.clients.get_mut("client3") {
            entry.value_mut().0 = past;
        }
        assert!(limiter.check_and_update("client3"));
    }
}
