mod connector;
mod heartbeat;
mod proxy;
mod ui;

use clap::Parser;
use shared::ClientConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug)]
#[command(name = "tunnelx", about = "Expose localhost to the internet")]
struct Args {
    /// Local port to expose
    #[arg(value_name = "PORT")]
    port: Option<u16>,

    /// Local port to expose (alternative)
    #[arg(long = "port", value_name = "PORT", conflicts_with = "port")]
    port_flag: Option<u16>,

    /// Relay WebSocket URL
    #[arg(long, env = "TUNNELX_RELAY")]
    relay: Option<String>,

    /// Requested subdomain (skip interactive prompt)
    #[arg(long, short)]
    subdomain: Option<String>,

    /// Tunnel duration in seconds (skip interactive prompt)
    #[arg(long)]
    duration: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install default crypto provider for rustls (required in rustls 0.23+)
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let args = Args::parse();
    let port = args.port.or(args.port_flag).ok_or("port is required")?;

    // ── Interactive UI flow ────────────────────────────────────────
    ui::print_banner();

    // Step 1: Connection
    ui::print_step_start(1, 3, "Establishing connection...");

    let config = ClientConfig::new(port, args.relay);

    // Step 2: Subdomain + duration (interactive or from CLI args)
    let subdomain = if let Some(s) = args.subdomain {
        Some(s)
    } else {
        ui::prompt_subdomain()
    };

    let duration_secs = if let Some(d) = args.duration {
        d
    } else {
        ui::prompt_duration()
    };

    // Step 3: Connect and run
    connector::run(config, subdomain, duration_secs).await?;

    Ok(())
}
