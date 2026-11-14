# IPSeek

[![Latest release](https://img.shields.io/github/v/release/fartbagxp/ipseek?sort=date)](https://github.com/fartbagxp/ipseek/releases/latest)
[![License: MIT](https://img.shields.io/github/license/fartbagxp/ipseek)](LICENSE)
[![Build and Release](https://github.com/fartbagxp/ipseek/actions/workflows/release.yml/badge.svg?branch=main)](https://github.com/fartbagxp/ipseek/actions/workflows/release.yml)
[![Fly Deploy](https://github.com/fartbagxp/ipseek/actions/workflows/fly-deploy.yml/badge.svg?branch=main)](https://ipseek.fly.dev/3.233.144.1)

[IPSeek](https://ipseek.fly.dev) gives you information about whether a IP address belongs to: which cloud, which region, and which service is running on it.

I use it by going to [https://ipseek.fly.dev/3.233.144.1](https://ipseek.fly.dev/3.233.144.1) and adding an IP address in the end of the URL and get an answer back. I needed a service like this because there were incidents in the past when I needed to figure out quickly where traffic were streaming in from.

Most of these companies publish their IP ranges, but each one uses its own format at its own URL, and the lists change constantly. [aas-cidr-ranges](https://github.com/fartbagxp/aas-cidr-ranges) collects them. I load them into a single table and answers lookups against it using IPSeek. When a service runs on someone else's cloud, which is common for Software-as-a-Service providers, it reports both instead of picking one:

```bash
3.233.144.1 -> datadog (service · agents) on aws (cloud · us-east-1 · AMAZON)
```

## What IPSeek Covers

- Covers 400,000+ ranges from cloud providers, CDNs, SaaS products, crawlers, and US government and health networks. The full list is below.
- Returns region, service, and provider-specific metadata where the source publishes it.
- Runs as a CLI (`ipseek check`) or an HTTP API (`ipseek serve`).
- Keeps everything in memory. The ranges are stored in one gzip-compressed file (`db.bin`) and searched with longest-prefix match. There's no database server, so it runs comfortably on a small fly.io machine.

## Supported Providers

| Category                | Providers                                                                                                                                                               |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Cloud**               | AWS, Azure (Public/China/Germany/Gov), GCP, Oracle, DigitalOcean, Linode, Vultr, Alibaba Cloud, Tencent Cloud, Baidu Cloud, Huawei Cloud, OVH, Scaleway, IONOS, Hetzner |
| **CDN**                 | Cloudflare, Fastly, BunnyCDN, G-Core, CacheFly, MaxCDN                                                                                                                  |
| **Security**            | Zscaler, Imperva                                                                                                                                                        |
| **Observability**       | Datadog, Grafana, New Relic, Pingdom, StatusCake                                                                                                                        |
| **Identity**            | Okta                                                                                                                                                                    |
| **Collaboration**       | GitHub, GitLab, Atlassian (Jira/Confluence), Zoom, Webex, Microsoft 365 (Worldwide/China/USGov-DoD/GCCHigh), Telegram                                                   |
| **Payments**            | Stripe                                                                                                                                                                  |
| **CRM / Docs**          | Salesforce, DocuSign                                                                                                                                                    |
| **Ops / Alerting**      | PagerDuty, Twilio (SIP)                                                                                                                                                 |
| **Database**            | MongoDB Atlas                                                                                                                                                           |
| **Privacy**             | iCloud Private Relay                                                                                                                                                    |
| **ISP**                 | Starlink                                                                                                                                                                |
| **DNS**                 | Public DNS resolvers (Cloudflare, Google, Quad9, and more)                                                                                                              |
| **AI Crawlers / Bots**  | OpenAI (GPTBot, ChatGPT-User, SearchBot), Bingbot, DuckDuckGo (DuckDuckBot, DuckAssistBot), Perplexity                                                                  |
| **Other**               | Seqera                                                                                                                                                                  |
| **Government & Health** | US Federal, State & City Government; Health IT Vendors; Hospital Systems; Health Insurance; Pharmacy Benefit Managers; Academics                                        |

## Installation

### Download Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/fartbagxp/ipseek/releases):

- **Linux x86_64**: `ipseek-linux-x86_64`
- **Linux ARM64**: `ipseek-linux-aarch64`
- **macOS x86_64**: `ipseek-macos-x86_64`
- **macOS ARM64**: `ipseek-macos-aarch64` (Apple Silicon)
- **Windows x86_64**: `ipseek-windows-x86_64.exe`

**Verify the Download:**

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

This downloads 98 provider files from [aas-cidr-ranges](https://github.com/fartbagxp/aas-cidr-ranges) and creates a compressed `db.bin` database (~406k ranges). Additional IP ranges for US government and health organizations are sourced from [gov-domains](https://github.com/fartbagxp/gov-domains) ASN data via `--gov-asn-path`.

**Alternative: Manual Workflow**

You can also download and build separately:

```bash
# Download IP range files from GitHub
./target/release/ipseek update --path ./data/raw

# Build database from downloaded files
./target/release/ipseek update-local --path ./data/raw
```

**Options for `update`:**

- `--path <PATH>` - Output directory for downloaded files (default: `./data/raw`)
- `--parallel <N>` - Number of concurrent downloads (default: 5)
- `--auto-build` - Automatically build database after downloading

**Options for `update-local`:**

- `--path <PATH>` - Directory of local provider files (default: `./data/raw`)
- `--gov-asn-path <PATH>` - Optional path to a [gov-domains](https://github.com/fartbagxp/gov-domains) `data/asn/` directory to include US government and health organization IP ranges

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

**GET /favicon.ico**, **GET /favicon.svg** - Site icon

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

# Build database and include gov-domains ASN data
ipseek update-local --path ./data/raw --gov-asn-path /path/to/gov-domains/data/asn

# Check a single IP
ipseek check <IP_ADDRESS>

# Run HTTP server
ipseek serve

# Specify custom database location
ipseek --db /path/to/db.bin serve
```

## Deployment on fly.io

The public instance at [ipseek.fly.dev](https://ipseek.fly.dev) runs on a single shared-CPU, 512 MB machine. The Docker build downloads the latest ranges, builds `db.bin`, and copies it into the runtime image. The server loads it once at startup, and every lookup after that runs in memory. Machines auto-stop when idle and start again on the next request, so the first request after a quiet period is slower.

See `fly.toml` and `Dockerfile` for the configuration.

## Development

### Project Structure

- `src/main.rs` - CLI argument parsing and command routing
- `src/lib.rs` - Library crate root, used by the integration tests
- `src/downloader.rs` - Download IP range files from GitHub
- `src/db.rs` - Database serialization/deserialization
- `src/updater.rs` - Parse provider files and build database
- `src/lookup.rs` - CLI lookup logic with LPM (Longest Prefix Match)
- `src/resolve.rs` - Turns matching ranges into the layered service-on-cloud answer
- `src/server.rs` - HTTP server and API handlers
- `assets/` - Favicon (`favicon.svg` is the source; `favicon.ico` is generated from it)
- `tests/check_ip.rs` - Integration tests
- `data/raw/` - Downloaded provider IP range files (JSON/TXT), created by `ipseek update`

### Pre-commit Hooks

Install [pre-commit](https://pre-commit.com/#install), then install the hooks:

```bash
pip install pre-commit
pre-commit install
```

The hooks run automatically on `git commit`. To run them manually against all files:

```bash
pre-commit run --all-files
```

To update pre-commit dependencies

```bash
pre-commit autoupdate --freeze
```

Hooks configured:

| Hook                   | What it does                                             |
| ---------------------- | -------------------------------------------------------- |
| `trailing-whitespace`  | Strips trailing whitespace                               |
| `end-of-file-fixer`    | Ensures files end with a newline                         |
| `check-yaml`           | Validates YAML syntax                                    |
| `check-toml`           | Validates TOML syntax (`Cargo.toml`, etc.)               |
| `check-merge-conflict` | Rejects leftover merge conflict markers                  |
| `fmt`                  | Runs `cargo fmt` to enforce Rust formatting              |
| `cargo-check`          | Runs `cargo check` to catch compile errors before commit |
| `gitleaks`             | Scans for secrets and credentials in staged changes      |

### Cargo Dependencies

Install [cargo outdated](https://github.com/kbknapp/cargo-outdated), then:

```bash
cargo outdated -R   # list outdated direct dependencies
cargo update        # apply semver-compatible updates (rewrites Cargo.lock only)
cargo test
```

Anything with `---` in the Compat column is a breaking upgrade. Bump its version in `Cargo.toml` by hand, check the crate's changelog for renamed features or APIs, and test again.

### Running Tests

```bash
cargo test
```

To run only the gov-domains ASN category tests:

```bash
cargo test test_gov_asn
```

### Adding New Providers

1. Add provider IP range file to `data/raw/`
2. Ensure format is either:
   - JSON with `prefixes`, `values`, or service-specific arrays
   - Plain text with one IP/CIDR per line
3. Rebuild database: `ipseek update-local --path ./data/raw`
