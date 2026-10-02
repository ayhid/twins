---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# Technology Stack

**Analysis Date:** 2026-10-02

## Languages

**Primary:**
- Rust 1.85 - Core engine and CLI implementation

## Runtime

**Environment:**
- macOS (required platform for full functionality - `Full Disk Access` needed for restricted folders)
- POSIX-compliant shell for cargo builds

**Package Manager:**
- Cargo (bundled with Rust 1.85+)
- Lockfile: `Cargo.lock` (present)

## Frameworks & Build Tools

**CLI Framework:**
- clap 4.6.6 - Command-line argument parsing with derive macros (`#[command]`, `#[arg]` attributes)
  - Provides `Parser`, `Args`, `Subcommand` traits for declarative CLI definition

**Build & Development:**
- Cargo - Workspace-based build system with workspace dependencies (resolver "3", edition 2024)
- LTO: Thin LTO enabled for release builds
- Strip enabled for release binaries (reduced size)
- Codegen units: 1 (maximum optimization for release)

## Workspace Structure

**Configuration:**
- Resolver version: 3 (latest)
- Edition: 2024 (Rust edition)
- License: MIT
- Repository: https://github.com/ayhid/twins

**Members:**
- `crates/twins-core` - Core duplicate detection engine
- `crates/twins-cli` - CLI binary (`twins` command)

**Workspace Lints:**

```toml
[workspace.lints.rust]
unsafe_code = "warn"
missing_docs = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
module_name_repetitions = "allow"
```

## Core Dependencies

**Hashing & Cryptography:**
- blake3 1.8.7 - Cryptographic hash for full file content comparison (parallel hashing capable)
- xxhash-rust 0.8.18 (xxh64 feature) - Fast partial hashing for first/last 16 KiB blocks

**Filesystem & Path Handling:**
- walkdir 2.5.0 - Recursive directory traversal with customizable behavior
- globset 0.4.20 - Glob pattern matching for file exclusion rules
- libc 0.2 - System-level file operations (device, inode detection)

**Concurrency & Parallelization:**
- rayon 1.12.0 - Data parallelism for stat and hashing operations (par_iter, thread pool management)
- Default parallelism: number of CPUs, capped at 8

**Serialization:**
- serde 1.0.229 (derive feature) - Serialization framework
- serde_json 1.0.151 - JSON serialization for report output (versioned schema v1)

**Error Handling:**
- thiserror 2.0.20 - Derive macro for error types with custom exit codes

**CLI Argument Parsing:**
- clap 4.6.6 (derive feature) - Declarative command-line interface using attributes

## Testing & Development Dependencies

**Testing Framework:**
- assert_cmd 2.2.2 - CLI integration testing (subprocess execution, output assertions)
- predicates 3 - Output predicate matching for assertions
- tempfile 3.27.0 - Temporary file/directory creation for integration tests

**Development Tools:**
- These are dev-only dependencies (not included in release builds)

## Configuration

**Build Configuration:**
- Release profile optimizations:
  ```toml
  [profile.release]
  lto = "thin"           # Link-time optimization enabled
  codegen-units = 1      # Maximum optimization passes
  strip = true           # Binary stripping for size reduction
  ```

**Runtime Configuration:**
- Environment variables read:
  - `HOME` - User home directory (fallback path for expansion)
  - `CARGO_PKG_VERSION` - Injected at compile time for `twins version` output

**Command-line Configuration:**
- All configuration via CLI flags (no config file parsing)
- Supported flags:
  - `--json` - Output format toggle
  - `--min-size` - File size filtering (e.g., 512, 10K, 1.5MiB, 2 GB)
  - `--exclude GLOB` - File exclusion patterns (repeatable)
  - `--jobs N` - Parallelism control
  - `--include-empty` - Include empty files
  - `--include-library` - Descend into ~/Library
  - `--include-node-modules` - Descend into node_modules
  - `--include-remote` - Accept network volumes
  - `--verify` - Byte-by-byte verification
  - `--verbose` - Detailed logging

## Platform Requirements

**Development:**
- Rust 1.85 or later (as specified in `rust-version`)
- macOS (for target platform matching and filesystem-specific behavior)
- Cargo (comes with Rust)

**Runtime:**
- macOS only
- Full Disk Access permission (required for scanning Mail, Messages, and other restricted folders)
- Command-line terminal with proper execution permissions

**Performance Specifications:**
- Parallelism: Number of CPU cores, capped at 8
- Memory: Reasonably low memory footprint (no large data structures held in memory)
- Threading model: Rayon-based thread pool for work distribution

---

*Stack analysis: 2026-10-02*
