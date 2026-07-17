# Deploying the TunnelX relay

The relay terminates TLS itself and listens on port `8443` inside its container. Docker publishes that port as public HTTPS port `443`.

## Prerequisites

- A Linux VPS with Docker Compose v2 and public ports `80` and `443` available.
- DNS records pointing at the VPS:
  - `relay.darsha.dev` for the tunnel client WebSocket endpoint.
  - `*.darsha.dev` for generated public tunnel URLs.
- A TLS certificate whose subject names include both `darsha.dev` and `*.darsha.dev`. Wildcard certificates require a DNS-01 ACME challenge.

The default client endpoint is `wss://relay.darsha.dev/tunnel`, while generated URLs are `https://<random>.darsha.dev`.

## Start the relay

```bash
cp .env.example .env
# Edit .env with the domain and certificate paths for the VPS.
docker compose up --build --detach
docker compose logs --follow relay
```

Confirm that the process is accepting traffic:

```bash
curl --insecure --head https://127.0.0.1/healthz
```

Use `--insecure` only for this local-IP health check; normal clients must validate the certificate through the configured DNS name.

## Firewall and certificate renewal

Allow inbound TCP ports `80` and `443`. Port `80` is needed only while your certificate-renewal tooling performs an HTTP challenge; DNS-01 renewal does not require it.

Renew certificates with your chosen ACME client, then restart the relay so it reloads the new files:

```bash
docker compose restart relay
```

## Development without TLS

For local testing, run the relay directly without `TLS_CERT` and `TLS_KEY`:

```bash
RELAY_BIND=127.0.0.1:8443 RELAY_DOMAIN=localhost cargo run --package relay
tunnelx 3000 --relay ws://127.0.0.1:8443/tunnel
```

Local public requests need a matching `Host` header because generated subdomains do not exist in public DNS. For example, replace `<subdomain>` with the value printed by the client:

```bash
curl -H 'Host: <subdomain>.localhost' http://127.0.0.1:8443/
```
