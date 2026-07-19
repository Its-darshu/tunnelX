//! Local registry of running tunnels.
//!
//! Each `tunnelx <port>` invocation is a standalone foreground process that holds a
//! single tunnel until it exits — there is no daemon. To make `tunnelx list` and
//! `tunnelx status` report reality, every running process records itself here in a
//! shared JSON file and refreshes a heartbeat timestamp. Readers prune entries whose
//! heartbeat has gone stale, so a crashed process (that never got to unregister)
//! disappears on its own.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// An entry is considered dead if its heartbeat is older than this. The tunnel
/// process refreshes every ~5s, giving three missed beats of grace.
const STALE_SECS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub subdomain: String,
    pub port: u16,
    pub public_url: String,
    pub pid: u32,
    /// Unix seconds when the tunnel came up.
    pub started_at: u64,
    /// Unix seconds of the last heartbeat.
    pub heartbeat: u64,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn registry_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("tunnelx").join("tunnels.json"))
}

fn read_all() -> Vec<RegistryEntry> {
    let Some(path) = registry_path() else {
        return Vec::new();
    };
    let Ok(data) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

fn write_all(entries: &[RegistryEntry]) {
    let Some(path) = registry_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string_pretty(entries) else {
        return;
    };
    // Write to a temp file then rename so a concurrent reader never sees a
    // half-written file.
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, json).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

/// Record this process's tunnel, replacing any stale entry for the same subdomain
/// or a previous entry from this PID.
pub fn register(subdomain: &str, port: u16, public_url: &str) {
    let mut entries = read_all();
    let pid = std::process::id();
    let now = now_secs();
    entries.retain(|e| e.subdomain != subdomain && e.pid != pid);
    entries.push(RegistryEntry {
        subdomain: subdomain.to_string(),
        port,
        public_url: public_url.to_string(),
        pid,
        started_at: now,
        heartbeat: now,
    });
    write_all(&entries);
}

/// Refresh the heartbeat for this process's tunnel so readers keep it as active.
pub fn heartbeat(subdomain: &str) {
    let mut entries = read_all();
    let now = now_secs();
    let mut changed = false;
    for e in entries.iter_mut() {
        if e.subdomain == subdomain {
            e.heartbeat = now;
            changed = true;
        }
    }
    if changed {
        write_all(&entries);
    }
}

/// Remove this process's tunnel on clean shutdown.
pub fn unregister(subdomain: &str) {
    let mut entries = read_all();
    let before = entries.len();
    entries.retain(|e| e.subdomain != subdomain);
    if entries.len() != before {
        write_all(&entries);
    }
}

/// Return currently active tunnels, pruning stale entries (e.g. from crashed
/// processes) from the file as a side effect.
pub fn list_active() -> Vec<RegistryEntry> {
    let now = now_secs();
    let (alive, stale): (Vec<_>, Vec<_>) = read_all()
        .into_iter()
        .partition(|e| now.saturating_sub(e.heartbeat) <= STALE_SECS);
    if !stale.is_empty() {
        write_all(&alive);
    }
    alive
}

/// Look up a single active tunnel by subdomain.
pub fn get_active(subdomain: &str) -> Option<RegistryEntry> {
    list_active().into_iter().find(|e| e.subdomain == subdomain)
}

/// Format a duration in seconds as a compact human string, e.g. "2h 15m" or "10m 30s".
pub fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}
