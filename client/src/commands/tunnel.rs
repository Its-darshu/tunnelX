use crate::connector;
use shared::ClientConfig;

pub async fn handle_tunnel(
    port: u16,
    subdomain: Option<String>,
    duration: Option<u64>,
    relay: Option<String>,
    profile: Option<String>,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load config from profile if specified
    let config = ClientConfig::new(port, relay);

    // Use provided duration or prompt
    let duration_secs = duration.unwrap_or(1800); // Default 30 minutes

    // Start tunnel
    if format == "json" {
        println!(
            r#"{{"status": "starting", "port": {}, "subdomain": {:?}, "duration": {}}}"#,
            port, subdomain, duration_secs
        );
    }

    connector::run(config, subdomain, duration_secs).await?;

    Ok(())
}
