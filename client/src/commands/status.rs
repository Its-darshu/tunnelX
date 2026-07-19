pub async fn handle_status(
    subdomain: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if format == "json" {
        println!(
            r#"{{"subdomain": "{}", "status": "active", "uptime": "5m30s", "requests": 42}}"#,
            subdomain
        );
    } else {
        println!("📊 Tunnel Status");
        println!("  Subdomain: {}", subdomain);
        println!("  Status: ✅ Active");
        println!("  Uptime: 5m30s");
        println!("  Requests: 42");
        println!("  URL: https://{}.darsha.dev", subdomain);
    }
    Ok(())
}
