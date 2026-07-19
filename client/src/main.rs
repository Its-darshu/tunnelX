mod commands;
mod connector;
mod heartbeat;
mod proxy;
mod registry;
mod ui;

use clap::Parser;
use commands::Commands;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "tunnelx",
    version = VERSION,
    about = "🚀 Expose localhost to the internet securely",
    long_about = "TunnelX: Share your local development server with anyone in the world via a secure HTTPS tunnel. \
                  No account required, just run and share!"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Positional port argument for backward compatibility (default tunnel command)
    #[arg(value_name = "PORT")]
    port: Option<u16>,

    /// Requested subdomain
    #[arg(long, short)]
    subdomain: Option<String>,

    /// Tunnel duration in seconds
    #[arg(long, short)]
    duration: Option<u64>,

    /// Relay WebSocket URL
    #[arg(long, env = "TUNNELX_RELAY")]
    relay: Option<String>,

    /// Profile to use
    #[arg(long)]
    profile: Option<String>,

    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    format: String,

    /// Enable debug logging
    #[arg(long, env = "TUNNELX_DEBUG")]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args = Cli::parse();

    let log_level = if args.debug { "debug" } else { "warn" };
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| log_level.into()))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    match args.command {
        Some(cmd) => handle_command(cmd).await,
        None => {
            if let Some(port) = args.port {
                ui::print_banner();
                ui::print_step_start(1, 3, "Establishing connection...");

                commands::tunnel::handle_tunnel(
                    port,
                    args.subdomain,
                    args.duration,
                    args.relay,
                    args.profile,
                    &args.format,
                )
                .await
            } else {
                Cli::parse_from(&["tunnelx", "--help"]);
                Ok(())
            }
        }
    }
}

async fn handle_command(cmd: Commands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        Commands::Tunnel {
            port,
            subdomain,
            duration,
            relay,
            profile,
            format,
        } => {
            ui::print_banner();
            ui::print_step_start(1, 3, "Establishing connection...");
            commands::tunnel::handle_tunnel(port, subdomain, duration, relay, profile, &format).await
        }
        Commands::Config(config_cmd) => commands::config::handle_config(config_cmd).await,
        Commands::Status { subdomain, format } => {
            commands::status::handle_status(&subdomain, &format).await
        }
        Commands::List { format, detailed } => commands::list::handle_list(&format, detailed).await,
        Commands::Completions { shell } => commands::completions::generate_completions(shell),
        Commands::Version => {
            println!("TunnelX v{}", VERSION);
            Ok(())
        }
    }
}
