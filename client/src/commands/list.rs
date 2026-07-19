use crate::registry::{self, format_uptime};

pub async fn handle_list(format: &str, detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let tunnels = registry::list_active();

    if format == "json" {
        let items: Vec<String> = tunnels
            .iter()
            .map(|t| {
                format!(
                    r#"{{"subdomain": {:?}, "status": "active", "uptime_secs": {}, "port": {}, "public_url": {:?}}}"#,
                    t.subdomain,
                    now.saturating_sub(t.started_at),
                    t.port,
                    t.public_url
                )
            })
            .collect();
        println!("{{\"tunnels\": [{}]}}", items.join(", "));
        return Ok(());
    }

    println!("🌐 Active Tunnels\n");

    if tunnels.is_empty() {
        println!("  No active tunnels.");
        println!("\n💡 Start one with 'tunnelx <port>' (e.g. 'tunnelx 5173').");
        return Ok(());
    }

    if detailed {
        println!(
            "{:<20} {:<10} {:<15} {:<8} {:<40}",
            "Subdomain", "Status", "Uptime", "Port", "Public URL"
        );
        println!("{}", "─".repeat(93));
        for t in &tunnels {
            println!(
                "{:<20} {:<10} {:<15} {:<8} {:<40}",
                t.subdomain,
                "✅ Active",
                format_uptime(now.saturating_sub(t.started_at)),
                t.port,
                t.public_url
            );
        }
    } else {
        for (i, t) in tunnels.iter().enumerate() {
            println!(
                "  {}. {:<12} (port {}) - ✅ Active - {}",
                i + 1,
                t.subdomain,
                t.port,
                format_uptime(now.saturating_sub(t.started_at))
            );
        }
    }

    println!("\n💡 Tip: Use 'tunnelx status <subdomain>' for detailed info");
    Ok(())
}
