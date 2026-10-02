---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
# Codebase Structure

**Analysis Date:** 2026-10-02

## Directory Layout

```
twins/
├── Cargo.toml                    # Workspace configuration, shared dependencies
├── Cargo.lock                    # Locked dependency versions
├── README.md                     # Project overview, usage, installation
├── LICENSE                       # MIT license
├── .gitignore                    # Git ignore patterns
├── .planning/
│   └── codebase/                 # This directory: analysis documents
├── crates/
│   ├── twins-core/               # Core engine library (scan, hash, group, report)
│   │   ├── Cargo.toml            # Engine dependencies
│   │   ├── src/
│   │   │   ├── lib.rs            # Public API (module re-exports)
│   │   │   ├── fsutil.rs         # Filesystem utilities (FileMeta, Identity, stat)
│   │   │   ├── hash.rs           # Hashing algorithms (partial, full, compare)
│   │   │   ├── human.rs          # Human-readable formatting (size parsing, display)
│   │   │   ├── report.rs         # Report generation (JSON, text)
│   │   │   ├── safety.rs         # Protected paths and security checks
│   │   │   ├── scan/
│   │   │   │   ├── mod.rs        # Module definition, Options, error types
│   │   │   │   ├── walk.rs       # Filesystem traversal with filters
│   │   │   │   ├── options.rs    # Scan options (roots, exclusions, workers, flags)
│   │   │   │   └── rules.rs      # Exclusion rules (.git, node_modules, bundles)
│   │   │   └── group/
│   │   │       ├── mod.rs        # Module definition, Group type
│   │   │       ├── find.rs       # Duplicate finding pipeline (size → partial → full → verify)
│   │   │       ├── hasher.rs     # Hasher trait and DirectHasher
│   │   │       ├── index.rs      # Candidate index (concurrent, size-keyed)
│   │   │       └── keep.rs       # Keep strategy and planning (Action, Keeper)
│   │   └── tests/
│   │       ├── fsutil_test.rs    # Tests for FileMeta, stat, Identity
│   │       ├── group_test.rs     # Tests for group finding and hardlinks
│   │       ├── hash_test.rs      # Tests for partial, full, compare hashing
│   │       ├── human_test.rs     # Tests for size parsing and formatting
│   │       ├── keep_test.rs      # Tests for keep strategies
│   │       ├── report_test.rs    # Tests for Report generation
│   │       ├── safety_test.rs    # Tests for protected path detection
│   │       ├── scan_test.rs      # Tests for filesystem walk
│   │       └── fixtures/
│   │           └── mod.rs        # Test fixture helpers (temp files, directories)
│   │
│   └── twins-cli/                # Command-line binary
│       ├── Cargo.toml            # CLI dependencies
│       ├── src/
│       │   ├── main.rs           # Entry point (parse args, dispatch, exit code)
│       │   ├── cli.rs            # Argument parsing (Clap derive, VERSION)
│       │   └── run.rs            # Execution orchestration (scan, report, progress)
│       └── tests/
│           └── cli_test.rs       # Integration tests (scan, report, JSON)
│
└── .claude/                      # Claude Code tooling (ignored by analyzer)
    └── (agent configuration files)
```

## Directory Purposes

**Workspace Root (`twins/`):**
- Purpose: Rust workspace configuration and shared build settings
- Contains: Cargo.toml with workspace members and shared dependencies (blake3, clap, rayon, serde, etc.)
- Key files: `Cargo.toml` (resolver 3, workspace dependencies), `Cargo.lock` (locked versions)

**`crates/twins-core/`:**
- Purpose: Core duplicate-detection engine library; public API for reuse
- Contains: Filesystem utilities, hashing, scanning, grouping, reporting
- Key files:
  - `lib.rs`: Re-exports public modules (scan, group, fsutil, hash, report, human, safety)
  - `src/scan/`: Walk with exclusions and safety checks
  - `src/group/`: Find duplicates through refinement pipeline
  - `src/hash.rs`: BLAKE3 and xxHash64 fingerprinting
  - Tests co-located with modules (module_test.rs)

**`crates/twins-cli/`:**
- Purpose: Command-line binary; thin wrapper around twins-core
- Contains: Argument parsing, run orchestration, progress reporting
- Key files:
  - `main.rs`: Entry point, subcommand routing, exit code handling
  - `cli.rs`: Clap-derived argument structures and VERSION
  - `run.rs`: Pipeline execution, progress printing, error description

**`crates/twins-core/src/scan/`:**
- Purpose: Filesystem traversal with safety and exclusion rules
- Contains:
  - `walk.rs`: WalkDir traversal, volume checks, candidate filtering
  - `rules.rs`: Glob compilation, skip patterns (.git, node_modules, .app bundles)
  - `options.rs`: Configuration (roots, exclusions, workers, include flags)

**`crates/twins-core/src/group/`:**
- Purpose: Duplicate detection pipeline and keep-strategy planning
- Contains:
  - `find.rs`: Refinement pipeline (size, partial hash, full hash, verify)
  - `index.rs`: Concurrent size-keyed collection
  - `hasher.rs`: Hasher trait and DirectHasher implementation
  - `keep.rs`: Strategy enum, Keeper applier, Action decisions
  - `mod.rs`: Group type (immutable, sorted by path)

**`crates/twins-core/tests/`:**
- Purpose: Unit and integration tests for core modules
- Contains: One test file per major module (hash_test.rs, scan_test.rs, etc.)
- Pattern: Tests are co-located with source, use tempfile for fixtures

## Key File Locations

**Entry Points:**
- `crates/twins-cli/src/main.rs`: Binary entry point (argv parse → subcommand dispatch → exit code)
- `crates/twins-core/src/lib.rs`: Library entry point (module re-exports)

**Configuration:**
- `Cargo.toml`: Workspace and crate manifest (dependencies, edition 2024, rust 1.85)
- `Cargo.lock`: Locked dependency versions

**Core Logic:**
- **Scan walk**: `crates/twins-core/src/scan/walk.rs` (filesystem traversal)
- **Duplicate finding**: `crates/twins-core/src/group/find.rs` (refinement pipeline)
- **Hashing**: `crates/twins-core/src/hash.rs` (partial, full, compare)
- **Keep strategy**: `crates/twins-core/src/group/keep.rs` (which copy survives)
- **Reporting**: `crates/twins-core/src/report.rs` (JSON and text output)

**Safety & Utilities:**
- `crates/twins-core/src/safety.rs`: Protected path checks
- `crates/twins-core/src/fsutil.rs`: FileMeta, Identity, stat
- `crates/twins-core/src/human.rs`: Size parsing and formatting

**Testing:**
- `crates/twins-core/tests/`: Co-located unit tests (hash_test.rs, scan_test.rs, etc.)
- `crates/twins-cli/tests/cli_test.rs`: Integration tests for the binary
- `crates/twins-core/tests/fixtures/mod.rs`: Test fixture helpers

## Naming Conventions

**Files:**
- `{module}_test.rs`: Unit tests for module (e.g., `hash_test.rs` for `hash.rs`)
- `mod.rs`: Module definition (aggregates submodules)
- `{feature}.rs`: Standalone feature (e.g., `fsutil.rs`, `safety.rs`, `human.rs`)

**Directories:**
- `src/`: Rust source files
- `tests/`: Test code (placed at crate root, not nested in src)
- `fixtures/`: Shared test data and helpers
- `crates/`: Workspace member directories

**Modules:**
- Lowercase with underscores (e.g., `twins_core`, `twins_cli`, `fsutil`, `scan`)
- Submodules in directories (e.g., `scan/walk.rs`, `group/find.rs`)

**Types:**
- PascalCase (FileMeta, Identity, Group, Strategy, Keeper, Digest)

**Functions:**
- snake_case (walk, find, partial, full, stat, plan)

**Error types:**
- PascalCase ending in `Error` (ScanError, FsError, HashError, FindError, GroupError)

## Where to Add New Code

**New Feature (e.g., caching, deletion, UI):**
- **Core logic**: `crates/twins-core/src/{feature}.rs` or `crates/twins-core/src/{module}/{feature}.rs`
- **Tests**: `crates/twins-core/tests/{feature}_test.rs`
- **CLI**: `crates/twins-cli/src/{feature}.rs` if user-facing
- **Example**: Deletion feature would be `crates/twins-core/src/delete.rs` with tests in `crates/twins-core/tests/delete_test.rs`

**New Component/Module:**
- **Library module**: `crates/twins-core/src/{name}.rs` or `crates/twins-core/src/{parent}/{name}.rs`
- **Binary helper**: `crates/twins-cli/src/{name}.rs`
- **Test file**: `crates/twins-core/tests/{name}_test.rs` (co-located)
- **Submodule**: Create `crates/twins-core/src/{parent}/mod.rs` if grouping related features

**Utilities & Helpers:**
- Shared helpers (size parsing, formatting, etc.): `crates/twins-core/src/human.rs`
- Filesystem utilities: `crates/twins-core/src/fsutil.rs`
- Safety checks: `crates/twins-core/src/safety.rs`

**Tests:**
- Unit tests: Same directory as src, file named `{module}_test.rs`
- Integration tests: `crates/twins-core/tests/` or `crates/twins-cli/tests/`
- Fixtures: `crates/twins-core/tests/fixtures/mod.rs`

## Special Directories

**`crates/`:**
- Purpose: Workspace members (libraries and binaries)
- Generated: No
- Committed: Yes

**`.planning/`:**
- Purpose: Planning and documentation generated by GSD tools
- Generated: Yes (by `/gsd-map-codebase`, `/gsd-plan-phase`, etc.)
- Committed: Yes (aids future execution)

**`.claude/`, `.codex/`, `_dev-workflow/`:**
- Purpose: Tool configuration (Claude Code, Codex, GSD workflow)
- Generated: Yes (by developer tools)
- Committed: Yes (team coordination)
- **Not part of project code** — ignored by codebase analyzer

**`.git/`:**
- Purpose: Git repository metadata
- Generated: Yes (by git)
- Committed: No (.gitignored)

**`target/` (created during build):**
- Purpose: Build artifacts (debug, release)
- Generated: Yes (by `cargo build`)
- Committed: No (.gitignored)

## Module Organization

**Library Layers (twins-core):**

1. **Low-level utilities** (no inter-module dependencies):
   - `fsutil.rs`: FileMeta snapshots, stat wrapper
   - `safety.rs`: Protected path checks
   - `human.rs`: Size parsing and formatting
   - `hash.rs`: Hash algorithms and Digest

2. **Concurrent collections**:
   - `group/index.rs`: Thread-safe size-keyed map

3. **Algorithms** (use layers 1-2):
   - `scan/walk.rs`: Uses fsutil, safety, rules; populates Index
   - `group/find.rs`: Uses hash, index, hasher; builds Groups
   - `group/keep.rs`: Uses Group; builds Actions

4. **Output** (use layers 1-3):
   - `report.rs`: Serializes Actions to JSON/text

**Binary Layer (twins-cli):**

1. **Argument parsing**:
   - `cli.rs`: Clap-derived structures

2. **Orchestration**:
   - `main.rs`: Routes subcommands, handles exit codes
   - `run.rs`: Calls scan walk, grouping, reporting, manages progress

---

*Structure analysis: 2026-10-02*
