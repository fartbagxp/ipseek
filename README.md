# IP Ranger

A fast, memory-efficient Rust application for identifying cloud providers, regions, and services from IP addresses. Designed for deployment on fly.io with minimal resource usage.

## Features

- 🚀 **Fast lookups** using optimized IP trie data structure
- 🌍 **74,000+ IP ranges** from major cloud providers (AWS, Azure, GCP, GitHub, Cloudflare, and more)
- 📊 **Rich metadata** including region, service, and provider-specific details
- 💾 **Minimal memory footprint** with gzip-compressed JSON database
- 🔧 **Dual interface** - HTTP API and CLI tool

## Supported Providers

AWS, Azure (Public/China/Germany/Gov), GCP, GitHub, Cloudflare, DigitalOcean, Fastly, Datadog, Atlassian, Oracle, Stripe, Zoom, Zscaler, Grafana, Linode, MaxCDN, Okta, Pingdom, Seqera, and more.

## Installation

### Build from Source

```bash
cargo build --release
```

The binary will be available at `./target/release/ip-ranger`

## Usage

### 1. Build the Database

Before using the tool, you need to build the IP database from the raw data files:

```bash
./target/release/ip-ranger update-local --path ./data/raw
```

This processes all provider files and creates a compressed `db.bin` database (~2.3 MB).

### 2. CLI Usage

Check individual IP addresses from the command line:

```bash
# Check an AWS IP
./target/release/ip-ranger check 13.32.1.1
# Output: 13.32.1.1 -> aws region=GLOBAL service=AMAZON

# Check an Azure IP
./target/release/ip-ranger check 40.72.175.176
# Output: 40.72.175.176 -> azure-china service=ActionGroup

# Check a GitHub IP
./target/release/ip-ranger check 192.30.252.1
# Output: 192.30.252.1 -> github service=hooks

# Check an unknown IP
./target/release/ip-ranger check 8.8.8.8
# Output: 8.8.8.8 -> unknown
```

### 3. HTTP Server

Run as an HTTP API server:

```bash
./target/release/ip-ranger serve
```

The server runs on `http://0.0.0.0:8080`

#### API Endpoints

**GET /** - Welcome message

**GET /{ip}** - Lookup IP address

Example requests:

```bash
# AWS IP
curl http://localhost:8080/13.32.1.1 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "provider": "aws",
    "prefix": "13.32.0.0/15",
    "region": "GLOBAL",
    "service": "AMAZON",
    "metadata": {
      "network_border_group": "GLOBAL"
    }
  }
}
```

```bash
# Azure IP
curl http://localhost:8080/40.72.175.176 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "provider": "azure-china",
    "prefix": "40.72.175.176/30",
    "region": "",
    "service": "ActionGroup",
    "metadata": {
      "cloud": "China",
      "platform": "Azure"
    }
  }
}
```

```bash
# Unknown IP
curl http://localhost:8080/8.8.8.8 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "provider": "unknown",
    "prefix": null,
    "region": null,
    "service": null,
    "metadata": {}
  }
}
```

## Commands

```bash
# Build database from local files
ip-ranger update-local --path ./data/raw

# Check a single IP
ip-ranger check <IP_ADDRESS>

# Run HTTP server
ip-ranger serve

# Specify custom database location
ip-ranger --db /path/to/db.bin serve
```

## Deployment on fly.io

The application is designed for minimal memory usage on fly.io:

1. Database is loaded once on startup
2. All lookups use in-memory trie structure
3. Gzip compression keeps database size small (~2.3 MB)
4. Supports auto-stop/start machines

See `fly.toml` for deployment configuration.

## Development

### Project Structure

- `src/main.rs` - CLI argument parsing and command routing
- `src/db.rs` - Database serialization/deserialization
- `src/updater.rs` - Parse provider files and build database
- `src/lookup.rs` - CLI lookup logic with LPM (Longest Prefix Match)
- `src/server.rs` - HTTP server and API handlers
- `data/raw/` - Cloud provider IP range files (JSON/TXT)

### Adding New Providers

1. Add provider IP range file to `data/raw/`
2. Ensure format is either:
   - JSON with `prefixes`, `values`, or service-specific arrays
   - Plain text with one IP/CIDR per line
3. Rebuild database: `ip-ranger update-local --path ./data/raw`
