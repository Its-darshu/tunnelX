pub mod config;
pub mod tunnel;
pub mod status;
pub mod list;
pub mod completions;

use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start a new tunnel to expose localhost
    #[command(alias = "t")]
    Tunnel {
        /// Local port to expose
        port: u16,

        /// Requested subdomain (skip interactive prompt)
        #[arg(long, short)]
        subdomain: Option<String>,

        /// Tunnel duration in seconds (skip interactive prompt)
        #[arg(long, short)]
        duration: Option<u64>,

        /// Relay WebSocket URL
        #[arg(long)]
        relay: Option<String>,

        /// Profile to use
        #[arg(long)]
        profile: Option<String>,

        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Manage configuration and profiles
    #[command(subcommand)]
    Config(config::ConfigCommands),

    /// Show tunnel status
    #[command(alias = "st")]
    Status {
        /// Subdomain or tunnel ID
        subdomain: String,

        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// List active tunnels
    #[command(alias = "ls")]
    List {
        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,

        /// Show full details
        #[arg(long, short)]
        detailed: bool,
    },

    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: clap_complete::Shell,
    },

    /// Show version
    #[command(hide = true)]
    Version,
}
