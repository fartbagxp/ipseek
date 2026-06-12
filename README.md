# IP Seek

A fast, memory-efficient Rust application for identifying cloud providers, regions, and services from IP addresses. Designed for deployment on fly.io with minimal resource usage.

## Features

- 🚀 **Fast lookups** over a flat, in-memory longest-prefix table (no database)
- 🌍 **390,000+ IP ranges** from major cloud providers and services (AWS, Azure, GCP, GitHub, Cloudflare, Fastly, Datadog, Stripe, Okta, Oracle, Zscaler, and more)
- 🧅 **Layered results** - when a service runs on top of a cloud (e.g. Stripe on AWS), the answer shows _both_: the service overlay **and** the underlying cloud infrastructure
- 📊 **Rich metadata** including region, service, and provider-specific details
- 💾 **Minimal memory footprint** - single gzip-compressed flat-file database (`db.bin`), embedded in the binary; no external store
- 🔧 **Dual interface** - HTTP API and CLI tool

## Supported Providers

| Category | Providers |
|---|---|
| **Cloud** | AWS, Azure (Public/China/Germany/Gov), GCP, Oracle, DigitalOcean, Linode, Vultr, Alibaba Cloud, Tencent Cloud, Baidu Cloud, Huawei Cloud, OVH, Scaleway, IONOS |
| **CDN** | Cloudflare, Fastly, BunnyCDN, G-Core, CacheFly, MaxCDN |
| **Security** | Zscaler, Imperva |
| **Observability** | Datadog, Grafana, New Relic, Pingdom |
| **Identity** | Okta |
| **Collaboration** | GitHub, GitLab, Atlassian (Jira/Confluence), Zoom, Webex, Microsoft 365 (Worldwide/China/USGov-DoD/GCCHigh) |
| **Payments** | Stripe |
| **CRM / Docs** | Salesforce, DocuSign |
| **Ops / Alerting** | PagerDuty, Twilio (SIP) |
| **Database** | MongoDB Atlas |
| **Privacy** | iCloud Private Relay |
| **ISP** | Starlink |
| **Other** | Seqera |

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

This downloads 85 provider files from [aas-cidr-ranges](https://github.com/fartbagxp/aas-cidr-ranges) and creates a compressed `db.bin` database (~390k ranges).

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

The CLI prints a one-line, layered summary. Services are shown first, then the
cloud infrastructure they run on top of (`service on cloud`).

```bash
# A plain cloud IP
./target/release/ipseek check 8.8.8.0
# Output: 8.8.8.0 -> gcp (cloud)

# A service running on a cloud - shows both layers
./target/release/ipseek check 3.233.144.1
# Output: 3.233.144.1 -> datadog (service · agents) on aws (cloud · us-east-1 · AMAZON)

./target/release/ipseek check 13.112.224.240
# Output: 13.112.224.240 -> stripe (service) on aws (cloud · ap-northeast-1 · AMAZON)

# An unknown IP
./target/release/ipseek check 1.1.1.1
# Output: 1.1.1.1 -> unknown
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

The `data` object carries a backward-compatible **primary** match at the top
level (`provider`, `prefix`, `region`, `service`, `metadata`, `kind`) plus two
fields describing the full picture:

- `summary` - a one-line human-readable description.
- `layers` - every provider that claims the IP, ordered service-first
  (overlay) then cloud-last (infrastructure). Each layer has a `kind` of
  either `"service"` or `"cloud"`.

Example requests:

```bash
# A service on a cloud - Datadog hosted on AWS
curl http://localhost:8080/3.233.144.1 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "ip": "3.233.144.1",
    "provider": "datadog",
    "prefix": "3.233.144.0/20",
    "region": null,
    "service": "agents",
    "kind": "service",
    "metadata": {},
    "summary": "datadog (service · agents) on aws (cloud · us-east-1 · AMAZON)",
    "layers": [
      {
        "provider": "datadog",
        "kind": "service",
        "prefix": "3.233.144.0/20",
        "service": "agents"
      },
      {
        "provider": "aws",
        "kind": "cloud",
        "prefix": "3.224.0.0/12",
        "region": "us-east-1",
        "service": "AMAZON",
        "metadata": { "network_border_group": "us-east-1" }
      }
    ]
  }
}
```

```bash
# A plain cloud IP - single layer
curl http://localhost:8080/8.8.8.8 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "ip": "8.8.8.8",
    "provider": "gcp",
    "prefix": "8.8.8.0/24",
    "region": null,
    "service": null,
    "kind": "cloud",
    "metadata": {},
    "summary": "gcp (cloud)",
    "layers": [{ "provider": "gcp", "kind": "cloud", "prefix": "8.8.8.0/24" }]
  }
}
```

```bash
# Unknown IP
curl http://localhost:8080/1.1.1.1 | jq '.'
```

Response:

```json
{
  "error": "",
  "data": {
    "ip": "1.1.1.1",
    "provider": "unknown",
    "prefix": null,
    "region": null,
    "service": null,
    "kind": null,
    "summary": "unknown",
    "layers": []
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
3. Gzip compression keeps database size small (~3 MB)
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
