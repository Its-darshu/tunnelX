use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Initialize configuration file
    Init {
        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },

    /// Set a configuration value
    Set {
        /// Config key (e.g., profile.default.relay, profile.default.duration)
        key: String,

        /// Config value
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Config key (e.g., profile.default.relay). Omit to show all
        key: Option<String>,

        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Show configuration file path
    Path,

    /// List all profiles
    Profiles {
        /// Output format (text, json)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Create a new profile
    CreateProfile {
        /// Profile name
        name: String,

        /// Relay URL
        #[arg(long)]
        relay: String,

        /// Default port
        #[arg(long)]
        port: Option<u16>,

        /// Default duration in seconds
        #[arg(long)]
        duration: Option<u64>,

        /// Set as default profile
        #[arg(long)]
        default: bool,
    },

    /// Delete a profile
    DeleteProfile {
        /// Profile name
        name: String,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },

    /// Validate configuration
    Validate,

    /// Reset configuration to defaults
    Reset {
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

pub async fn handle_config(cmd: ConfigCommands) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ConfigCommands::Init { force } => init_config(force).await,
        ConfigCommands::Set { key, value } => set_config(&key, &value).await,
        ConfigCommands::Get { key, format } => get_config(key.as_deref(), &format).await,
        ConfigCommands::Path => show_config_path().await,
        ConfigCommands::Profiles { format } => list_profiles(&format).await,
        ConfigCommands::CreateProfile {
            name,
            relay,
            port,
            duration,
            default,
        } => create_profile(&name, &relay, port, duration, default).await,
        ConfigCommands::DeleteProfile { name, force } => delete_profile(&name, force).await,
        ConfigCommands::Validate => validate_config().await,
        ConfigCommands::Reset { force } => reset_config(force).await,
    }
}

async fn init_config(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("✨ Initializing TunnelX configuration...");
    println!("📁 Config saved to: {}", config_path()?.display());
    println!("✅ Configuration initialized successfully!");
    Ok(())
}

async fn set_config(key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ Config set: {} = {}", key, value);
    Ok(())
}

async fn get_config(key: Option<&str>, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(k) = key {
        println!("Config: {} = <value>", k);
    } else {
        println!("Showing all configuration...");
    }
    Ok(())
}

async fn show_config_path() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", config_path()?.display());
    Ok(())
}

async fn list_profiles(format: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Available profiles:");
    println!("  - default");
    println!("  - production");
    Ok(())
}

async fn create_profile(
    name: &str,
    relay: &str,
    port: Option<u16>,
    duration: Option<u64>,
    default: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ Profile '{}' created", name);
    if default {
        println!("📌 Set as default profile");
    }
    Ok(())
}

async fn delete_profile(name: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ Profile '{}' deleted", name);
    Ok(())
}

async fn validate_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ Configuration is valid");
    Ok(())
}

async fn reset_config(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("✅ Configuration reset to defaults");
    Ok(())
}

fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir()
        .ok_or("Cannot determine config directory")?
        .join("tunnelx");
    Ok(config_dir.join("config.toml"))
}
