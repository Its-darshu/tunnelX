use crate::registry::{self, format_uptime};

pub async fn handle_status(
    subdomain: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let entry = registry::get_active(subdomain);

    if format == "json" {
        match entry {
            Some(t) => println!(
                r#"{{"subdomain": {:?}, "status": "active", "uptime_secs": {}, "port": {}, "public_url": {:?}}}"#,
                t.subdomain,
                now.saturating_sub(t.started_at),
                t.port,
                t.public_url
            ),
            None => println!(
                r#"{{"subdomain": {:?}, "status": "not_found"}}"#,
                subdomain
            ),
        }
        return Ok(());
    }

    match entry {
        Some(t) => {
            println!("📊 Tunnel Status");
            println!("  Subdomain: {}", t.subdomain);
            println!("  Status:    ✅ Active");
            println!("  Uptime:    {}", format_uptime(now.saturating_sub(t.started_at)));
            println!("  Port:      localhost:{}", t.port);
            println!("  URL:       {}", t.public_url);
        }
        None => {
            println!("❔ No active tunnel named '{}'.", subdomain);
            println!("\n💡 Run 'tunnelx list' to see active tunnels.");
        }
    }
    Ok(())
}
