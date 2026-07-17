# TunnelX

Lightweight, secure, and fast localhost tunneling. Share your local web applications with anyone on the internet using a single command.

```bash
tunnelx 3000
```

```
╭──────────────────────────────────────────╮
│           🚀 TunnelX  v0.1.0            │
╰──────────────────────────────────────────╯

[1/3] Establishing connection...
  ✔ Connected to TunnelX server (relay.darsha.dev)

[2/3] Configure your tunnel
  Enter subdomain (or press Enter for random): my-app
  Select duration: 30 minutes

  ✔ Subdomain "my-app" registered!

  🌐 Tunnel is live!

  https://my-app.darsha.dev  →  localhost:3000

  █▀▀▀▀▀▀█ ▄ █▀▄▀▄ █▀▀▀▀▀▀█    (QR code for
  █ ███ █ ▀▄█▀█▄▄▀ █ ███ █     mobile sharing)
  ▀▀▀▀▀▀▀ ▄ ▄ ▄▀▄ ▀▀▀▀▀▀▀

  📋 Share this URL with your friends!
  ⏱  Expires in: 30m 00s

─── Request Log ──────────────────────────────
  12:30:01  GET     /                   200   12ms
  12:30:05  GET     /style.css          200    8ms
  12:30:05  POST    /api/login          201   89ms
  12:30:06  WS      /ws                 101    0ms

─── [c] Copy URL  [q] Quit  ⏱ 28m 42s ───
```

## Features

- **One command** to expose localhost to the internet
- **Custom subdomains** — choose your own `my-app.darsha.dev`
- **Time-limited tunnels** — 5 min, 20 min, 30 min, or 1 hour
- **HTTPS public URLs** with valid TLS
- **No account required** — just run and share
- **Live request log** — see traffic flowing through your tunnel in real-time
- **QR code** — scan from your phone to test mobile
- **WebSocket & SSE** — full support for real-time apps (Vite HMR, Socket.IO, etc.)
- **Cross-platform** — Linux, macOS, and Windows

## Quick Start

### Install the client

```bash
cargo install --path client
```

### Basic usage

```bash
# Interactive mode — prompts for subdomain and duration
tunnelx 3000

# Non-interactive — specify everything upfront
tunnelx 3000 --subdomain my-app --duration 1800

# Custom relay server
tunnelx 3000 --relay ws://127.0.0.1:8443/tunnel
```

### CLI options

```
tunnelx [PORT]                        # expose localhost on PORT
tunnelx --port 3000                   # explicit port flag
tunnelx --subdomain my-app            # request a custom subdomain
tunnelx --duration 1800               # tunnel duration in seconds
tunnelx --relay wss://relay.example/tunnel  # custom relay URL
```

### Keyboard shortcuts (while tunnel is active)

| Key | Action |
|-----|--------|
| `c` | Copy public URL to clipboard |
| `q` | Quit and close tunnel |
| `Ctrl+C` | Quit and close tunnel |

## Running the relay (local development)

```bash
RELAY_BIND=127.0.0.1:8443 RELAY_DOMAIN=localhost cargo run --package relay
tunnelx 3000 --relay ws://127.0.0.1:8443/tunnel
```

In development, use the generated subdomain as the `Host` header when testing:

```bash
curl -H 'Host: <subdomain>.localhost' http://127.0.0.1:8443/
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_BIND` | `0.0.0.0:8443` | Relay listen address |
| `RELAY_DOMAIN` | `darsha.dev` | Base domain for tunnel URLs |
| `TLS_CERT` | — | Path to TLS certificate (optional for dev) |
| `TLS_KEY` | — | Path to TLS private key (optional for dev) |
| `RUST_LOG` | `info` | Log level |
| `TUNNELX_RELAY` | `wss://relay.darsha.dev/tunnel` | Client relay URL (env override) |

## Architecture

```
Browser → HTTPS → Relay Server → WSS → Tunnel Client → localhost
```

All communication between client and relay uses **bincode-serialized frames over WebSocket binary messages**, supporting concurrent requests via `request_id` correlation.

## Development

```bash
cargo build          # build all crates
cargo test           # run unit + integration tests
cargo run -p relay   # start the relay
cargo run -p client -- 3000  # start the client
```

## Deployment

See [docs/deployment.md](docs/deployment.md) for Docker deployment on a Linux VPS with wildcard DNS and TLS certificates for `*.darsha.dev`.

## License

MIT
