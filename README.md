# IP Seek

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

### Download Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/fartbagxp/ipseek/releases):

- **Linux x86_64**: `ipseek-linux-x86_64`
- **Linux ARM64**: `ipseek-linux-aarch64`
- **macOS x86_64**: `ipseek-macos-x86_64`
- **macOS ARM64**: `ipseek-macos-aarch64` (Apple Silicon)
- **Windows x86_64**: `ipseek-windows-x86_64.exe`

**Verify the download:**

```bash
# Download binary and checksum (use latest weekly release: YYYY.MM.DD)
VERSION="2025.03.10"  # Check releases page for latest version
curl -LO https://github.com/fartbagxp/ipseek/releases/download/${VERSION}/ipseek-linux-x86_64
curl -LO https://github.com/fartbagxp/ipseek/releases/download/${VERSION}/ipseek-linux-x86_64.sha256

# Verify SHA256 checksum
echo "$(cat ipseek-linux-x86_64.sha256)  ipseek-linux-x86_64" | shasum -a 256 -c

# Verify build attestation (requires gh CLI)
gh attestation verify ipseek-linux-x86_64 --owner fartbagxp

# Make executable
chmod +x ipseek-linux-x86_64
```

See [`.github/RELEASE.md`](.github/RELEASE.md) for detailed verification instructions.

### Build from Source

```bash
cargo build --release
```

The binary will be available at `./target/release/ipseek`

## Usage

### 1. Download and Build the Database

The easiest way to get started is to download IP range files and build the database in one step:

```bash
# Download IP ranges from GitHub and build database automatically
./target/release/ipseek update --auto-build
```

This downloads 56 provider files from [aas-cidr-ranges](https://github.com/fartbagxp/aas-cidr-ranges) and creates a compressed `db.bin` database (~2.3 MB).

**Alternative: Manual Workflow**

You can also download and build separately:

```bash
# Download IP range files from GitHub
./target/release/ipseek update --path ./data/raw

# Build database from downloaded files
./target/release/ipseek update-local --path ./data/raw
```

**Options for the update command:**
- `--path <PATH>` - Output directory for downloaded files (default: `./data/raw`)
- `--parallel <N>` - Number of concurrent downloads (default: 5)
- `--auto-build` - Automatically build database after downloading

### 2. CLI Usage

Check individual IP addresses from the command line:

```bash
# Check an AWS IP
./target/release/ipseek check 13.32.1.1
# Output: 13.32.1.1 -> aws region=GLOBAL service=AMAZON

# Check an Azure IP
./target/release/ipseek check 40.72.175.176
# Output: 40.72.175.176 -> azure-china service=ActionGroup

# Check a GitHub IP
./target/release/ipseek check 192.30.252.1
# Output: 192.30.252.1 -> github service=hooks

# Check an unknown IP
./target/release/ipseek check 8.8.8.8
# Output: 8.8.8.8 -> unknown
```

### 3. HTTP Server

Run as an HTTP API server:

```bash
./target/release/ipseek serve
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
# Download IP ranges and build database in one step
ipseek update --auto-build

# Download IP ranges only
ipseek update --path ./data/raw

# Build database from local files
ipseek update-local --path ./data/raw

# Check a single IP
ipseek check <IP_ADDRESS>

# Run HTTP server
ipseek serve

# Specify custom database location
ipseek --db /path/to/db.bin serve
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
- `src/downloader.rs` - Download IP range files from GitHub
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
3. Rebuild database: `ipseek update-local --path ./data/raw`
