# Release Process

This document explains how releases are built and how to verify them.

## Automated Releases

Releases are automatically built using GitHub Actions in two ways:

### 1. Manual Release (On-Demand)

Trigger a release manually from the GitHub Actions tab:

1. Go to Actions → "Build and Release"
2. Click "Run workflow"
3. Click "Run workflow" to confirm

**Version format:** `YYYY.MM.DD.HHMM` (e.g., `2025.03.11.1430`)

- Includes timestamp to allow multiple releases per day
- Example: `2025.03.11.1430` = March 11, 2025 at 2:30 PM UTC

### 2. Weekly Automated Release

Every Monday at 00:00 UTC, a new release is automatically created:

- **Version format:** `YYYY.MM.DD` (e.g., `2025.03.10`)
- Marked as pre-release
- Latest binaries from the main branch
- No manual intervention needed

## Supported Platforms

Each release includes binaries for:

- **Linux x86_64** (`ip-ranger-linux-x86_64`)
- **Linux ARM64** (`ip-ranger-linux-aarch64`)
- **macOS x86_64** (`ip-ranger-macos-x86_64`)
- **macOS ARM64** (`ip-ranger-macos-aarch64`) - Apple Silicon
- **Windows x86_64** (`ip-ranger-windows-x86_64.exe`)

## Binary Verification

### 1. SHA256 Checksum Verification

Each binary includes a `.sha256` file:

**Linux/macOS:**

```bash
# Download binary and checksum
curl -LO https://github.com/fartbagxp/ip-ranger/releases/download/v1.0.0/ip-ranger-linux-x86_64
curl -LO https://github.com/fartbagxp/ip-ranger/releases/download/v1.0.0/ip-ranger-linux-x86_64.sha256

# Verify checksum
echo "$(cat ip-ranger-linux-x86_64.sha256)  ip-ranger-linux-x86_64" | shasum -a 256 -c
```

**Windows (PowerShell):**

```powershell
# Download binary and checksum
Invoke-WebRequest -Uri "https://github.com/fartbagxp/ip-ranger/releases/download/v1.0.0/ip-ranger-windows-x86_64.exe" -OutFile "ip-ranger.exe"
Invoke-WebRequest -Uri "https://github.com/fartbagxp/ip-ranger/releases/download/v1.0.0/ip-ranger-windows-x86_64.exe.sha256" -OutFile "ip-ranger.exe.sha256"

# Verify checksum
$expectedHash = Get-Content ip-ranger.exe.sha256
$actualHash = (Get-FileHash ip-ranger.exe -Algorithm SHA256).Hash
if ($expectedHash -eq $actualHash) { "✓ Checksum verified" } else { "✗ Checksum mismatch!" }
```

### 2. Binary Attestation Verification

All binaries include build provenance attestations using GitHub's artifact attestation feature.

**Install GitHub CLI:**

```bash
# macOS
brew install gh

# Linux
# See https://github.com/cli/cli/blob/trunk/docs/install_linux.md

# Windows
# See https://github.com/cli/cli/releases
```

**Verify attestation:**

```bash
# Authenticate with GitHub
gh auth login

# Verify the binary
gh attestation verify ip-ranger-linux-x86_64 --owner fartbagxp
```

This verifies:

- ✓ The binary was built by GitHub Actions
- ✓ The binary came from your repository
- ✓ The binary matches the claimed commit SHA
- ✓ The build environment and dependencies used

### 3. What Attestations Provide

Binary attestations are cryptographically signed statements about how a binary was built:

- **Build provenance**: Shows the exact GitHub Actions workflow that built the binary
- **Source code**: Links the binary to a specific commit SHA
- **Build environment**: Documents the OS, toolchain, and dependencies used
- **Supply chain security**: Prevents tampering between build and distribution
- **Reproducibility**: Allows verification that the binary matches the source

## Example: Download and Verify

Complete example for Linux:

```bash
# Set variables (use latest release or specific date version)
VERSION="2025.03.10"  # Weekly release format
# OR
VERSION="2025.03.11.1430"  # Manual release format
OWNER="fartbagxp"
BINARY="ip-ranger-linux-x86_64"

# Download binary and checksum
curl -LO "https://github.com/${OWNER}/ip-ranger/releases/download/${VERSION}/${BINARY}"
curl -LO "https://github.com/${OWNER}/ip-ranger/releases/download/${VERSION}/${BINARY}.sha256"

# Verify SHA256
echo "$(cat ${BINARY}.sha256)  ${BINARY}" | shasum -a 256 -c

# Verify attestation (requires gh CLI)
gh attestation verify ${BINARY} --owner ${OWNER}

# Make executable
chmod +x ${BINARY}

# Run
./${BINARY} --help
```

## Security Best Practices

1. **Always verify checksums** before using downloaded binaries
2. **Verify attestations** when possible for additional security
3. **Use HTTPS** when downloading binaries
4. **Check release notes** for known issues or security updates
5. **Report vulnerabilities** via GitHub Security Advisories

## Troubleshooting

**Checksum verification fails:**

- Re-download the binary (may have been corrupted)
- Ensure you're comparing against the correct `.sha256` file
- Check that you downloaded from the official repository

**Attestation verification fails:**

- Ensure GitHub CLI is authenticated: `gh auth status`
- Check that you're using the correct repository owner
- Verify you have the latest version of `gh`: `gh version`

**Binary won't run:**

- Linux/macOS: Make sure it's executable: `chmod +x ip-ranger-*`
- Linux: Check architecture: `uname -m` (should match x86_64 or aarch64)
- macOS: If blocked by Gatekeeper: Right-click → Open, or remove quarantine: `xattr -d com.apple.quarantine ip-ranger-*`

## Release Workflow Details

The release workflow:

1. Determines version (from tag, manual input, or weekly schedule)
2. Creates a GitHub release with release notes
3. Builds binaries for all platforms in parallel
4. Strips debug symbols for smaller binaries
5. Generates SHA256 checksums
6. Creates build provenance attestations
7. Uploads binaries and checksums to the release
8. Stores artifacts for 30 days for verification

All builds run in isolated GitHub-hosted runners with:

- Clean build environments
- Cached dependencies for faster builds
- Cross-compilation support for ARM platforms
- Reproducible build configurations
