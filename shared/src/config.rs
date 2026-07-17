use crate::TunnelError;

pub const DEFAULT_RELAY_URL: &str = "wss://tunnelx.darsha.dev/tunnel";
pub const DEFAULT_DOMAIN: &str = "tunnelx.darsha.dev";
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8443";

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub bind_addr: String,
    pub domain: String,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub path_routing: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
            domain: DEFAULT_DOMAIN.to_string(),
            tls_cert_path: None,
            tls_key_path: None,
            path_routing: false,
        }
    }
}

impl RelayConfig {
    pub fn from_env() -> Result<Self, TunnelError> {
        let mut config = Self::default();

        if let Ok(bind) = std::env::var("RELAY_BIND") {
            config.bind_addr = bind;
        }
        if let Ok(domain) = std::env::var("RELAY_DOMAIN") {
            config.domain = domain;
        }
        if let Ok(cert) = std::env::var("TLS_CERT") {
            config.tls_cert_path = Some(cert);
        }
        if let Ok(key) = std::env::var("TLS_KEY") {
            config.tls_key_path = Some(key);
        }
        if let Ok(path_routing) = std::env::var("PATH_ROUTING") {
            config.path_routing = path_routing.parse().unwrap_or(false);
        }

        Ok(config)
    }

    pub fn tls_enabled(&self) -> bool {
        self.tls_cert_path.is_some() && self.tls_key_path.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub port: u16,
    pub relay_url: String,
}

impl ClientConfig {
    pub fn new(port: u16, relay_url: Option<String>) -> Self {
        Self {
            port,
            relay_url: relay_url.unwrap_or_else(|| DEFAULT_RELAY_URL.to_string()),
        }
    }
}
