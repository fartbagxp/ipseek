# Release Checklist

## Before First Release

- [ ] Replace `YOUR_USERNAME` in README.md with your GitHub username
- [ ] Update repository URL in `.github/RELEASE.md`
- [ ] Review `.github/workflows/release.yml` settings
- [ ] Ensure `data/raw/` directory has IP range files
- [ ] Build and test locally: `cargo build --release`
- [ ] Generate initial db.bin: `./target/release/ip-ranger update-local --path ./data/raw`
- [ ] Commit db.bin to repository (for fly.io deployment)

## Triggering Your First Release

### Option 1: Manual Release (Recommended)

1. Go to GitHub → Actions → "Build and Release"
2. Click "Run workflow"
3. Click "Run workflow" to confirm
4. Wait ~10 minutes for all builds to complete
5. Check Releases page for binaries
6. Version will be: `YYYY.MM.DD.HHMM` (e.g., `2025.03.11.1430`)

### Option 2: Wait for Weekly Release

- Automatically runs every Monday at 00:00 UTC
- Creates version: `YYYY.MM.DD` (e.g., `2025.03.10`)
- Marked as pre-release

## After First Release

- [ ] Download and test binaries for your platform
- [ ] Verify SHA256 checksum
- [ ] Test attestation verification: `gh attestation verify <binary> --owner <username>`
- [ ] Note the version format (YYYY.MM.DD or YYYY.MM.DD.HHMM)
- [ ] Update README examples with actual release version
- [ ] Announce release (optional)

## Ongoing Releases

Weekly releases run automatically, but trigger manual releases for:

- Security patches
- Feature releases
- Bug fixes
- Before important announcements

## Troubleshooting

**Workflow fails:**

- Check Actions logs for specific error
- Verify Rust compilation works locally
- Check GitHub Actions permissions (Settings → Actions → General)

**Attestation missing:**

- Ensure repository has "Read and write permissions" for GITHUB_TOKEN
- Enable "Allow GitHub Actions to create and approve pull requests" if needed

**Binary doesn't work:**

- Verify correct platform (x86_64 vs aarch64)
- Check executable permissions: `chmod +x <binary>`
- macOS: Remove quarantine: `xattr -d com.apple.quarantine <binary>`

## Platform-Specific Notes

**Linux:**

- x86_64 for Intel/AMD processors
- aarch64 for ARM64 (Raspberry Pi 4+, AWS Graviton, etc.)

**macOS:**

- x86_64 for Intel Macs
- aarch64 for Apple Silicon (M1/M2/M3)

**Windows:**

- x86_64 for modern 64-bit Windows
- Run in PowerShell or Command Prompt

## Version Format Reference

**Weekly automated releases:** `YYYY.MM.DD`

- Example: `2025.03.10`
- Released every Monday at 00:00 UTC
- Marked as pre-release

**Manual on-demand releases:** `YYYY.MM.DD.HHMM`

- Example: `2025.03.11.1430` (March 11, 2025 at 2:30 PM UTC)
- Allows multiple releases per day
- Standard release (not pre-release)
