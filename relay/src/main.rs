mod cleanup;
mod router;
mod server;
mod session;
mod state;
mod url_gen;

use shared::RelayConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = RelayConfig::from_env()?;
    tracing::info!(bind = %config.bind_addr, domain = %config.domain, tls = config.tls_enabled(), "starting relay");

    cleanup::spawn_cleanup_task(session::sessions());
    server::run(config).await?;

    Ok(())
}
