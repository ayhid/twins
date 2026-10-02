---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# External Integrations

**Analysis Date:** 2026-10-02

## APIs & External Services

**Not used.** Twins is a standalone CLI tool with no external API dependencies or network calls.

## Data Storage

**Databases:**
- Not used. No persistent data storage or database.

**File Storage:**
- Local filesystem only. Reads from the user's filesystem, produces no artifacts or cached data.
- Default scan root: User's home directory (`~`)
- Scan scope controlled via command-line arguments (specific paths or default)

**Caching:**
- Not implemented in current version. See README status: "Deletion, caching, configuration and the desktop app follow in later steps."

## Authentication & Identity

**Auth Provider:**
- Not used. No authentication layer.

**System Access:**
- macOS Full Disk Access permission - Required via System Settings → Privacy & Security for scanning restricted folders (Mail, Messages, etc.)
- Terminal execution permissions - Standard shell execution privileges

## Monitoring & Observability

**Error Tracking:**
- Not used. No error tracking service integration.

**Logs:**
- Standard error output (stderr) via `eprintln!` macro
- Optional verbose mode (`--verbose` flag) - Lists every file that could not be read
- Structured output:
  - Text format: Human-readable report of duplicate groups (default)
  - JSON format: Versioned JSON schema (version 1) for machine parsing and scripting

## CI/CD & Deployment

**Hosting:**
- Not applicable. Twins is a CLI tool distributed via cargo.

**Distribution:**

```bash
cargo install --path crates/twins-cli
```

**CI Pipeline:**
- GitHub Actions: `.github/workflows/ci.yml` (job `test`, `macos-latest`)
- Triggers: push to `main`, every pull request
- Toolchain: `dtolnay/rust-toolchain@stable` with rustfmt + clippy; cache via `Swatinem/rust-cache@v2`
- Env: `RUSTFLAGS=-D warnings`
- Steps: `cargo fmt --all --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace` → `cargo build --workspace --release`

**Build Output:**
- Binary: `target/debug/twins` (development) or `target/release/twins` (optimized)
- Installation: Via `cargo install`, places binary in user's Cargo bin directory (`~/.cargo/bin/twins`)

## Environment Configuration

**Required environment variables:**
- None strictly required at runtime
- `HOME` - Read optionally for home directory resolution (used as fallback path expansion)

**Optional environment variables:**
- None defined

**Secrets location:**
- No secrets used or stored
- No `.env` files present

## Webhooks & Callbacks

**Incoming:**
- Not used. Twins is a CLI tool, not a server.

**Outgoing:**
- Not used. No external event callbacks or webhooks.

## Output & Reporting

**Report Formats:**
- Text (default): Human-readable duplicate group listings
- JSON (via `--json` flag or `twins report` command): Machine-parseable JSON with versioned schema

**JSON Schema:**
- Version: 1 (versioned for compatibility)
- Contains:
  - Summary metadata
  - Duplicate groups (lists of byte-identical files)
  - Removal recommendations (based on "oldest" strategy)

**Example Scripting:**

```bash
twins report ~/Downloads | jq '.summary'           # Extract summary stats
twins report ~/Downloads | jq -r '.groups[].remove[]'  # List files to remove
```

## Platform-Specific Integrations

**macOS Filesystem:**
- Hardlink detection: Uses `(device, inode)` pairs for deduplication awareness
- Symbolic link handling: Never follows symlinks (skips them entirely)
- Bundle handling: Skips `.app`, `.photoslibrary` and similar bundles (never opened)
- Network volume support: Refused by default, accepted with `--include-remote` flag
- iCloud placeholder handling: Never descends into iCloud placeholders (not downloaded)
- Time Machine: Excluded from scans

**macOS System Paths:**
- Never scanned: `/System`, `/Library`, `/usr`, `/Applications` (hardcoded exclusions)
- Opt-in scanning: `~/Library` (disabled by default, enabled with `--include-library`)

## Development Dependencies (External)

**GitHub Repository:**
- Repository: https://github.com/ayhid/twins
- License: MIT
- CI via GitHub Actions (see CI Pipeline above); no release/deploy automation

**Crate Registry:**
- All dependencies sourced from crates.io (default registry)
- No private registries or custom sources

---

*Integration audit: 2026-10-02*
