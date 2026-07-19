# TunnelX Universal CLI Guide

## Overview

TunnelX provides a modern, feature-rich CLI for sharing localhost to the internet securely. The CLI supports both interactive and non-interactive modes, configuration profiles, JSON output, and shell completions.

## Installation

```bash
cargo install --path client
```

## Quick Start

### Simple tunnel
```bash
tunnelx 3000
```

### Tunnel with options
```bash
tunnelx tunnel 3000 --subdomain my-app --duration 1800
```

### List active tunnels
```bash
tunnelx list
```

### Check tunnel status
```bash
tunnelx status my-app
```

---

## Commands

### `tunnel` - Start a new tunnel

Start exposing your localhost to the internet.

#### Usage
```bash
tunnelx tunnel <PORT> [OPTIONS]
```

#### Arguments
- `<PORT>` - Local port to expose (required)

#### Options
- `-s, --subdomain <SUBDOMAIN>` - Requested subdomain (skips interactive prompt)
- `-d, --duration <DURATION>` - Tunnel duration in seconds (skips interactive prompt)
- `--relay <RELAY>` - Custom relay WebSocket URL
- `--profile <PROFILE>` - Use a specific profile
- `--format <FORMAT>` - Output format: `text` or `json` (default: `text`)

#### Examples
```bash
# Interactive mode (prompts for subdomain and duration)
tunnelx tunnel 3000

# Full automation
tunnelx tunnel 3000 -s my-app -d 1800

# Custom relay server
tunnelx tunnel 3000 --relay wss://relay.example.com/tunnel

# JSON output for scripting
tunnelx tunnel 3000 -s my-app --format json
```

---

### `config` - Manage configuration and profiles

Manage TunnelX configuration, profiles, and settings.

#### Usage
```bash
tunnelx config <SUBCOMMAND>
```

#### Subcommands

##### `init` - Initialize configuration
```bash
tunnelx config init [--force]
```
Creates a default configuration file in `~/.config/tunnelx/config.toml`.

**Options:**
- `--force` - Overwrite existing configuration

##### `set` - Set configuration value
```bash
tunnelx config set <KEY> <VALUE>
```

**Examples:**
```bash
tunnelx config set profile.default.relay wss://relay.example.com/tunnel
tunnelx config set profile.default.duration 1800
tunnelx config set profile.production.relay wss://prod-relay.example.com/tunnel
```

##### `get` - Get configuration value
```bash
tunnelx config get [KEY] [--format text|json]
```

**Examples:**
```bash
tunnelx config get                           # Show all config
tunnelx config get profile.default.relay     # Get specific value
tunnelx config get --format json             # JSON output
```

##### `path` - Show configuration file path
```bash
tunnelx config path
```

##### `profiles` - List all profiles
```bash
tunnelx config profiles [--format text|json]
```

**Output:**
```
Available profiles:
  - default
  - production
  - staging
```

##### `create-profile` - Create a new profile
```bash
tunnelx config create-profile <NAME> --relay <URL> [OPTIONS]
```

**Options:**
- `--relay <RELAY>` - Relay URL (required)
- `--port <PORT>` - Default port
- `--duration <DURATION>` - Default duration in seconds
- `--default` - Set as default profile

**Example:**
```bash
tunnelx config create-profile production \
  --relay wss://prod-relay.example.com/tunnel \
  --duration 3600 \
  --default
```

##### `delete-profile` - Delete a profile
```bash
tunnelx config delete-profile <NAME> [--force]
```

##### `validate` - Validate configuration
```bash
tunnelx config validate
```

Checks if the configuration file is valid and all required fields are present.

##### `reset` - Reset to defaults
```bash
tunnelx config reset [--force]
```

---

### `list` - List active tunnels

Display all currently active tunnels with their status.

#### Usage
```bash
tunnelx list [OPTIONS]
```

#### Options
- `-d, --detailed` - Show detailed information
- `--format <FORMAT>` - Output format: `text` or `json` (default: `text`)

#### Examples
```bash
# Simple list
tunnelx list

# Detailed table view
tunnelx list --detailed

# JSON output
tunnelx list --format json
```

#### Output
```
🌐 Active Tunnels

Subdomain            Status     Uptime          Port       Public URL
─────────────────────────────────────────────────────────────────────
my-app               ✅ Active   10m 30s         3000       https://my-app.darsha.dev
api-dev              ✅ Active   2h 15m          8080       https://api-dev.darsha.dev
```

---

### `status` - Show tunnel status

Display detailed status and statistics for a specific tunnel.

#### Usage
```bash
tunnelx status <SUBDOMAIN> [OPTIONS]
```

#### Arguments
- `<SUBDOMAIN>` - Subdomain or tunnel ID (required)

#### Options
- `--format <FORMAT>` - Output format: `text` or `json` (default: `text`)

#### Examples
```bash
tunnelx status my-app
tunnelx status my-app --format json
```

#### Output
```
📊 Tunnel Status
  Subdomain: my-app
  Status: ✅ Active
  Uptime: 5m30s
  Requests: 42
  URL: https://my-app.darsha.dev
```

---

### `completions` - Generate shell completions

Generate shell completion scripts for bash, zsh, fish, powershell, or elvish.

#### Usage
```bash
tunnelx completions <SHELL>
```

#### Arguments
- `<SHELL>` - Shell type: `bash`, `zsh`, `fish`, `powershell`, `elvish`

#### Installation

##### Bash
```bash
tunnelx completions bash > ~/.bash_completions
echo "source ~/.bash_completions" >> ~/.bashrc
```

##### Zsh
```bash
tunnelx completions zsh > /usr/local/share/zsh/site-functions/_tunnelx
```

##### Fish
```bash
tunnelx completions fish | sudo tee /usr/share/fish/vendor_completions.d/tunnelx.fish
```

---

## Global Options

These options work with any command:

- `-h, --help` - Show help message
- `-V, --version` - Show version
- `--debug` - Enable debug logging (also: `TUNNELX_DEBUG=1`)
- `--format <FORMAT>` - Output format (text/json) where supported

---

## Environment Variables

```bash
# Relay server URL
export TUNNELX_RELAY="wss://relay.example.com/tunnel"

# Enable debug logging
export TUNNELX_DEBUG=1
```

---

## Configuration Files

### Location
```
~/.config/tunnelx/config.toml
```

### Default Structure
```toml
[profile.default]
relay = "wss://relay.darsha.dev/tunnel"
duration = 1800

[profile.production]
relay = "wss://prod-relay.example.com/tunnel"
duration = 3600
```

---

## Examples

### Example 1: Quick share
```bash
$ tunnelx 3000
🚀 TunnelX v0.1.0

[1/3] Establishing connection...
  ✔ Connected to TunnelX server
[2/3] Registering tunnel...
  ✔ Subdomain "abc123" registered!

🌐 Tunnel is live!

  https://abc123.darsha.dev  →  localhost:3000
  
📋 Share this URL with your friends!
```

### Example 2: Automated tunnel setup
```bash
#!/bin/bash
# Start tunnel with specific subdomain and duration
tunnelx tunnel 3000 -s myapp -d 3600 --format json > tunnel.json

# Extract URL from JSON
URL=$(jq -r '.public_url' tunnel.json)
echo "Tunnel URL: $URL"
```

### Example 3: Multiple profiles
```bash
# Create development profile
tunnelx config create-profile dev \
  --relay wss://dev-relay.example.com/tunnel \
  --duration 3600

# Create production profile
tunnelx config create-profile prod \
  --relay wss://prod-relay.example.com/tunnel \
  --duration 7200 \
  --default

# Use production profile
tunnelx tunnel 3000 --profile prod
```

### Example 4: Monitor tunnels
```bash
# Check all tunnels every 10 seconds
while true; do
  clear
  tunnelx list --detailed
  sleep 10
done
```

### Example 5: JSON scripting
```bash
# Get tunnel status as JSON
STATUS=$(tunnelx status my-app --format json)

# Parse with jq
UPTIME=$(echo $STATUS | jq -r '.uptime')
REQUESTS=$(echo $STATUS | jq -r '.requests')

echo "Tunnel uptime: $UPTIME"
echo "Total requests: $REQUESTS"
```

---

## Troubleshooting

### Connection issues
```bash
# Enable debug logging
tunnelx tunnel 3000 --debug

# Check relay connectivity
tunnelx config get profile.default.relay
```

### Configuration problems
```bash
# Validate configuration
tunnelx config validate

# Reset to defaults
tunnelx config reset --force
```

### Shell completions not working
```bash
# Regenerate completions
tunnelx completions bash > ~/.bash_completions
source ~/.bash_completions
```

---

## Tips & Tricks

### Auto-copy URL to clipboard
```bash
# Works with tunnel command
tunnelx tunnel 3000 -s myapp --format json | \
  jq -r '.public_url' | pbcopy  # macOS
```

### Create alias for common usage
```bash
# Add to ~/.bashrc or ~/.zshrc
alias txl="tunnelx list --detailed"
alias txs="tunnelx status"
```

### Continuous monitoring
```bash
watch -n 5 "tunnelx list --detailed"
```

---

## Performance

- **Startup time**: ~100ms (after first connection)
- **Memory usage**: ~15MB baseline
- **Maximum concurrent tunnels**: Limited by relay server

---

## Support

For issues, questions, or feature requests:
- GitHub Issues: https://github.com/Its-darshu/tunnelX/issues
- Security: darshan99806@gmail.com

---

**Version**: 0.1.0  
**Last Updated**: 2026-07-19
