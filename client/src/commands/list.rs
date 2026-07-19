pub async fn handle_list(format: &str, detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    if format == "json" {
        println!(
            r#"{{"tunnels": [{{"subdomain": "my-app", "status": "active", "uptime": "10m", "port": 3000}}]}}"#
        );
    } else {
        println!("🌐 Active Tunnels\n");

        if detailed {
            println!("{:<20} {:<10} {:<15} {:<10} {:<30}", "Subdomain", "Status", "Uptime", "Port", "Public URL");
            println!("{}", "─".repeat(85));
            println!("{:<20} {:<10} {:<15} {:<10} {:<30}", "my-app", "✅ Active", "10m 30s", "3000", "https://my-app.darsha.dev");
            println!("{:<20} {:<10} {:<15} {:<10} {:<30}", "api-dev", "✅ Active", "2h 15m", "8080", "https://api-dev.darsha.dev");
        } else {
            println!("  1. my-app     (port 3000) - ✅ Active - 10m 30s");
            println!("  2. api-dev    (port 8080) - ✅ Active - 2h 15m");
        }

        println!("\n💡 Tip: Use 'tunnelx status <subdomain>' for detailed info");
    }
    Ok(())
}
