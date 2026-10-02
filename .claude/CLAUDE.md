<!-- GSD:project-start source:PROJECT.md -->

## Project

**twins**

twins finds and removes duplicate files on macOS, safely. It is being rebuilt in Rust as a Cargo
workspace (`twins-core`, `twins-cli`, `twins-app`). A Tauri v2 + Svelte desktop app is the main
product and a companion CLI covers scripting. On top of the rewrite comes a workflow builder,
similar in spirit to n8n, for defining what to scan and when to run it. It is a personal tool
first that also happens to be distributed (Homebrew cask, signed and notarized).

**Core Value:** **Never lose data.** Deletion must be safe above everything else: at least one copy of every
group always survives, the Trash is the default, every operation is journaled and `--dry-run`
changes nothing. Speed, automation and polish all come second to this.

### Constraints

- **Platform**: macOS only. Needs Full Disk Access for restricted folders.
- **Tech stack**: Rust 1.85+, edition 2024, workspace lints (`unsafe_code` warn, clippy pedantic); Tauri v2 + Svelte for the app.
- **Safety**: every deletion path (CLI, app, workflow) goes through one core routine that enforces keep-one, validates the plan and writes the journal.
- **Architecture**: `twins-core` holds all logic; the CLI, the app and the workflow runner are thin shells over it.
- **Distribution**: signed and notarized builds through a Homebrew cask.

<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->

## Technology Stack

## Languages

- Rust 1.85 - Core engine and CLI implementation

## Runtime

- macOS (required platform for full functionality - `Full Disk Access` needed for restricted folders)
- POSIX-compliant shell for cargo builds
- Cargo (bundled with Rust 1.85+)
- Lockfile: `Cargo.lock` (present)

## Frameworks & Build Tools

- clap 4.6.6 - Command-line argument parsing with derive macros (`#[command]`, `#[arg]` attributes)
- Cargo - Workspace-based build system with workspace dependencies (resolver "3", edition 2024)
- LTO: Thin LTO enabled for release builds
- Strip enabled for release binaries (reduced size)
- Codegen units: 1 (maximum optimization for release)

## Workspace Structure

- Resolver version: 3 (latest)
- Edition: 2024 (Rust edition)
- License: MIT
- Repository: https://github.com/ayhid/twins
- `crates/twins-core` - Core duplicate detection engine
- `crates/twins-cli` - CLI binary (`twins` command)

## Core Dependencies

- blake3 1.8.7 - Cryptographic hash for full file content comparison (parallel hashing capable)
- xxhash-rust 0.8.18 (xxh64 feature) - Fast partial hashing for first/last 16 KiB blocks
- walkdir 2.5.0 - Recursive directory traversal with customizable behavior
- globset 0.4.20 - Glob pattern matching for file exclusion rules
- libc 0.2 - System-level file operations (device, inode detection)
- rayon 1.12.0 - Data parallelism for stat and hashing operations (par_iter, thread pool management)
- Default parallelism: number of CPUs, capped at 8
- serde 1.0.229 (derive feature) - Serialization framework
- serde_json 1.0.151 - JSON serialization for report output (versioned schema v1)
- thiserror 2.0.20 - Derive macro for error types with custom exit codes
- clap 4.6.6 (derive feature) - Declarative command-line interface using attributes

## Testing & Development Dependencies

- assert_cmd 2.2.2 - CLI integration testing (subprocess execution, output assertions)
- predicates 3 - Output predicate matching for assertions
- tempfile 3.27.0 - Temporary file/directory creation for integration tests
- These are dev-only dependencies (not included in release builds)

## Configuration

- Release profile optimizations:
- Environment variables read:
- All configuration via CLI flags (no config file parsing)
- Supported flags:

## Platform Requirements

- Rust 1.85 or later (as specified in `rust-version`)
- macOS (for target platform matching and filesystem-specific behavior)
- Cargo (comes with Rust)
- macOS only
- Full Disk Access permission (required for scanning Mail, Messages, and other restricted folders)
- Command-line terminal with proper execution permissions
- Parallelism: Number of CPU cores, capped at 8
- Memory: Reasonably low memory footprint (no large data structures held in memory)
- Threading model: Rayon-based thread pool for work distribution

<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

## Naming Patterns

- Snake case: `hash.rs`, `fsutil.rs`, `cli_test.rs`
- Module entry points: `mod.rs` in directories like `crates/twins-core/src/group/mod.rs`
- Test files: `{module}_test.rs` (e.g., `hash_test.rs`, `keep_test.rs`)
- Snake case: `partial()`, `equal()`, `visited()`, `colliding_pair()`
- Descriptive names with verb-noun pattern: `parse_size()`, `write_json()`, `is_local_volume()`
- Snake case: `mut filled`, `dir`, `opts`, `min_size`
- Abbreviations acceptable when clear from context: `f` for file, `n` for count
- Pascal case: `Digest`, `FileMeta`, `Group`, `ScanArgs`, `HashError`
- Error types: `{Action}Error` pattern (e.g., `HashError`, `FsError`, `GroupError`)
- Strategy enums: `Strategy::Oldest`, `Strategy::Newest`, `Strategy::ShortestPath`
- Upper case with underscores: `SAMPLE_SIZE`, `BUFFER_SIZE`, `SAMPLE_LEN`, `SF_DATALESS`, `MIB`
- Module-level visibility: exported from module files (e.g., `pub const SAMPLE_SIZE: u64 = 16 * 1024;`)

## Code Style

- Tool: `rustfmt` (built-in Rust formatter, checked in CI with `cargo fmt --all --check`)
- Standard Rust formatting rules apply (4-space indents, trailing commas in multi-line structures)
- No custom `.rustfmt.toml` configuration—uses defaults
- Tool: `clippy`
- CI enforcement: `cargo clippy --workspace --all-targets -- -D warnings` (warnings treated as errors)
- Workspace lints in `Cargo.toml`:
- Suppress with `#[allow(...)]` only when justified by comment (e.g., `#[allow(clippy::struct_excessive_bools)]`)

## Import Organization

- Relative paths: `use crate::...` for internal modules
- No path remapping configured; imports use direct module paths

## Documentation

- All public functions, types, and modules must have doc comments
- Use `///` for items and `//!` for module-level documentation
- Include error documentation with `# Errors` section
- Public struct fields documented with `///`
- No doc comments required for private items (though helpful comments are acceptable)

## Error Handling

- Custom error types derived with `thiserror::Error`:
- Use `Result<T, E>` return types for fallible functions
- Propagate errors with `?` operator
- For CLI: use `anyhow::Result<()>` and `anyhow!()` macro for simple context
- `{Domain}Error` pattern: `HashError`, `FsError`, `GroupError`
- Use enum variants for multiple error conditions

#[derive(Debug, thiserror::Error)]

## Unsafe Code

- Mark unsafe blocks with `#[allow(unsafe_code)]` at function level
- Add explanatory comment above unsafe block:

## Visibility & Encapsulation

- Use `pub use` in `mod.rs` for public re-exports
- Avoid `pub mod` for leaf modules—prefer `mod` and selective `pub use`
- Private fields with public accessor methods
- Use `#[must_use]` on accessors to encourage consumption

## Attributes & Annotations

- Applied to all builder methods and functions whose return value should not be discarded
- Applied to functions that compute important values
- Use standard trait derives: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
- Add `thiserror::Error` for error types
- Add `serde::{Deserialize, Serialize}` for types that need JSON serialization

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
- Use `#[serde(default, skip_serializing_if = "String::is_empty")]` for optional fields
- Use `#[serde(...)]` sparingly—only for schema control

## Builder Pattern

- Use method chaining with `self` returns for optional configuration
- Builders consume `self` and return `Self`

<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

## System Overview

```text

```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| CLI Parser | Parse arguments, invoke scan command | `crates/twins-cli/src/cli.rs` |
| Run Handler | Orchestrate pipeline, manage progress | `crates/twins-cli/src/run.rs` |
| Scan Walk | Traverse filesystem, filter candidates | `crates/twins-core/src/scan/walk.rs` |
| Rules Engine | Apply exclusion globs, skip patterns | `crates/twins-core/src/scan/rules.rs` |
| Safety Guards | Protect system directories | `crates/twins-core/src/safety.rs` |
| Candidate Index | Collect files by size | `crates/twins-core/src/group/index.rs` |
| Pipeline Refiner | Hash candidates, eliminate non-duplicates | `crates/twins-core/src/group/find.rs` |
| Keep Strategy | Decide which copy survives | `crates/twins-core/src/group/keep.rs` |
| Hasher Interface | Abstract hashing (current: direct reads) | `crates/twins-core/src/group/hasher.rs` |
| Hash Algorithms | Partial (xxHash64) and full (BLAKE3) | `crates/twins-core/src/hash.rs` |
| Report Builder | Format results as JSON or text | `crates/twins-core/src/report.rs` |
| Filesystem Utils | Read metadata, detect hardlinks/iCloud | `crates/twins-core/src/fsutil.rs` |
| Human Formatting | Parse sizes, format numbers/times | `crates/twins-core/src/human.rs` |

## Pattern Overview

- **Layered pipeline**: Each stage only processes what the previous stage couldn't rule out
- **Concurrency**: Rayon thread pools for filesystem walk and hashing (separate pools, different parallelism)
- **Immutable snapshots**: FileMeta records point-in-time metadata; no shared mutable state except Index
- **Safety-first**: Protected paths checked before scan; hardlinks never deleted
- **Progressive filtering**: ~90% of files eliminated by size alone; most remaining by partial hash

## Layers

- Purpose: Parse command-line arguments and dispatch to scan or report
- Location: `crates/twins-cli/src/`
- Contains: Clap-derived argument structs, version, subcommand routing
- Depends on: `twins-core`, `anyhow`, `clap`
- Used by: User directly via the `twins` binary
- Purpose: Traverse filesystem safely, collect file metadata, validate roots
- Location: `crates/twins-core/src/scan/` and `fsutil.rs`
- Contains: Walk logic with WalkDir, exclusion rules, volume locality checks, iCloud detection
- Depends on: `walkdir`, `globset`, `libc` (for volume checks)
- Used by: Run handler to populate Index with candidates
- Purpose: Hold candidates in memory, organize by size, refine through hashing
- Location: `crates/twins-core/src/group/`
- Contains: Index (concurrent, size-keyed), Group (immutable set of duplicates), Strategy (keep decisions)
- Depends on: `rayon` for parallel hashing
- Used by: Find pipeline to build Groups from candidates
- Purpose: Compute fingerprints (xxHash64 partial, BLAKE3 full) and compare
- Location: `crates/twins-core/src/hash.rs` and `group/hasher.rs`
- Contains: Partial hash (head/tail samples), full hash (streaming), byte-by-byte compare, Digest type
- Depends on: `blake3`, `xxhash-rust`
- Used by: Find pipeline to eliminate non-duplicates
- Purpose: Serialize decisions and statistics as JSON or text
- Location: `crates/twins-core/src/report.rs`
- Contains: Report struct (versioned), Summary and GroupEntry types, JSON/text writers
- Depends on: `serde_json`
- Used by: Run handler to output results
- Purpose: Protect system directories and user homes from accidental scans
- Location: `crates/twins-core/src/safety.rs`
- Contains: Protected path list (/System, /Library, /usr, etc.), User::Library detection
- Depends on: None (filesystem introspection only)
- Used by: Scan walk to validate roots and directories
- Purpose: Parse human sizes (1K, 1MiB) and format output (numbers, times)
- Location: `crates/twins-core/src/human.rs`
- Contains: ParseSizeError, parse_size, human_size functions
- Depends on: None
- Used by: CLI run handler and report builder

## Data Flow

### Primary Request Path (Scan Command)

### Secondary Flow: Hardlink Detection

- During scan walk, `FileMeta` captures `(device, inode)` as `Identity`
- `Group::physical()` counts distinct identities → hardlinks count once
- Hardlinks are never removed: deletion would waste no space and risk data loss
- `Action::reclaimable()` only counts truly distinct physical copies
- **Immutable**: FileMeta, Group, Identity (thread-safe by value)
- **Mutable**: Index (Mutex-wrapped HashMap for concurrent `add`)
- **Concurrent writes**: Index::add from multiple walk threads
- **Concurrent reads**: Index::candidates accessed by main thread after walk completes

## Key Abstractions

- Purpose: Immutable snapshot of a regular file's identity and size
- Examples: `crates/twins-core/src/fsutil.rs`
- Pattern: Value type (Clone, Copy where possible); read via `stat()` once per file
- Contains: path, size, mtime, identity (device, inode), nlink, dataless flag
- Purpose: Uniquely identify physical data on disk via `(device, inode)` pair
- Examples: Compared to detect hardlinks in `Group::physical()`
- Pattern: Hash + Eq + Ord for set operations and grouping
- Purpose: Immutable set of byte-identical files (members sorted by path)
- Examples: Returned by `group::find()`, consumed by `group::plan()`
- Pattern: Built via `Group::new()` with sorted members; provides `files()`, `size()`, `digest()`, `reclaimable()`
- Purpose: Concurrent, size-keyed collection of FileMeta
- Examples: Populated during scan walk, queried for candidates
- Pattern: Mutex-wrapped HashMap with `add()` for writes, `candidates()` for post-walk query
- Thread-safe: Multiple walk threads call `add()` concurrently
- Purpose: Policy for which copy to keep in a duplicate group
- Examples: Oldest (default), Newest, ShortestPath, InDir
- Pattern: Enum with `Keeper` applier; tiebreakers (depth, path length, lex order)
- Purpose: Trait for fingerprinting (partial and full); extensible for caching
- Examples: DirectHasher (current); future cache could wrap it
- Pattern: `partial(&FileMeta) -> u64`, `full(&FileMeta) -> Digest`
- Purpose: 256-bit BLAKE3 hash as byte array + Display
- Examples: Group members with identical Digest are byte-identical
- Pattern: Newtype `[u8; 32]` with hex string Display, FromStr parsing

## Entry Points

- Location: `crates/twins-cli/src/main.rs:12-28`
- Triggers: User runs binary with `scan` subcommand
- Responsibilities: Parse args, call `run::scan(..., false)`, format text or JSON output
- Location: `crates/twins-cli/src/main.rs:14-20`
- Triggers: User runs binary with `report` subcommand
- Responsibilities: Same as scan but always outputs JSON (force `--json`)
- Location: `crates/twins-core/src/scan/walk.rs:77-111`
- Triggers: `run::scan()` invokes it to populate Index
- Responsibilities: Traverse filesystem, filter by rules, call visitor for each candidate
- Location: `crates/twins-core/src/group/find.rs:191-196`
- Triggers: After scan completes, to refine candidates into Groups
- Responsibilities: Run parallel hashing pipeline, report progress and errors
- Location: `crates/twins-core/src/group/keep.rs:180-192`
- Triggers: After groups found, to decide which files to remove
- Responsibilities: Apply Keeper strategy to each group, return Actions

## Architectural Constraints

- **Threading:** Separate rayon pools: walk uses one thread per CPU (stat-bound), hashing caps at 8 (I/O bound). No global thread pools; each pool created on demand.
- **Global state:** None. Index is the only shared mutable state (Mutex-wrapped). Options and Strategy are immutable.
- **Circular imports:** None (three-layer DAG: CLI → Core → std/external)
- **Hardlink safety:** Files with identical `(device, inode)` are hardlinks; only keep's copy survives. Never listed for deletion.
- **macOS-specific:** Uses `libc` for volume checks (SEEK_DATA bit), `st_flags` for iCloud detection, BSD `nlink` field.
- **File metadata consistency:** Snapshots taken at walk time; changes after walk won't be detected. Inode reuse not possible in a scan session.

## Anti-Patterns

### File Content Modified After Walk

- HashError is reported via `on_error` callback, file is excluded from results
- `scan::walk()` uses snapshot at walk time; follow-up walks are independent
- Future: caching layer would need refresh-on-mtime-change logic

### Hardlinks Treated as Separate Duplicates

### Scanning Protected Paths Without Permission

## Error Handling

- **Fatal**: ScanError (no roots, protected root, glob parse, thread pool)
- **Non-fatal**: Per-file walk/stat/hash errors reported to `on_error` callback, counted in Stats, file excluded from results
- **User-facing**: Exit code 2 for usage errors (ScanError, ParseSizeError), 1 for I/O errors

## Cross-Cutting Concerns

<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

| Skill | Description | Path |
|-------|-------------|------|
| dev-adr | Record an architecture decision as an ADR — draw out the options that were rejected and why, write the record, and freeze it. Use when a decision gets made mid-session, when the user types /dev-adr, or when asked why something is the way it is. | `.claude/skills/dev-adr/SKILL.md` |
| dev-bug | Capture a bug as a tracker issue — investigate the likely code path, check for duplicates, draft the issue, and file it on approval. Use when the user types /dev-bug or describes something broken mid-session. | `.claude/skills/dev-bug/SKILL.md` |
| dev-docs-init | Scaffold a greenfield project's documentation — create the context, architecture, domain, api, ux, operations, testing and security documents, then fill them a claim at a time, each line carrying the evidence that would show it false. Use when a new project has no documentation yet, when /dev-init reports greenfield, or when the user types /dev-docs-init. | `.claude/skills/dev-docs-init/SKILL.md` |
| dev-done | Finish work on the current branch's tracker issue — verify acceptance criteria, run the project's checks, and on confirmation move the ticket to the configured done state with a summary comment. Use when the user says they are done or types /dev-done. | `.claude/skills/dev-done/SKILL.md` |
| dev-ingest-docs | Absorb an existing codebase's documentation into a verified map — inventory the docs, extract what they claim with evidence, find contradictions, ask for arbitration where evidence cannot settle it, and emit a map. Runs in steps across sessions. Use when joining a brownfield project, when /dev-init reports one, or when asked to understand, onboard onto, or digest a codebase and its docs. | `.claude/skills/dev-ingest-docs/SKILL.md` |
| dev-init | Set up the dev workflow in this project — pick the issue tracker, probe the repo, confirm the project, language, state ladder and check commands with the user, and write .dev-workflow.json. Use when /dev-task, /dev-bug or /dev-done reports missing config, or when the user types /dev-init. | `.claude/skills/dev-init/SKILL.md` |
| dev-lint-rules | Turn a project's stated conventions into rules its linter can decide, each with the count of what it would flag today — and name the ones no linter can settle as a hook, a claim, or noise. Use before writing a conventions document, when a review keeps restating the same rule, or when the user types /dev-lint-rules. | `.claude/skills/dev-lint-rules/SKILL.md` |
| dev-review | Review the current branch through three adversarial lenses — a blind pass that never sees the intent, an edge-case pass, and an acceptance audit — and report findings sorted into fix-the-code, fix-the-spec and out-of-scope. Use before opening a PR, when asked to review a branch, or when the user types /dev-review. | `.claude/skills/dev-review/SKILL.md` |
| dev-standup | Report everything in flight across the project's repos — what merged recently, what is checked out, what has stopped moving, what is still open on the tracker, and the one thing waiting on you. Use when the user asks for a standup, what they were working on, what is in progress, what landed yesterday, or what to pick up next. | `.claude/skills/dev-standup/SKILL.md` |
| dev-task | Start work on a tracker issue, or on a plain sentence describing what you want — file the issue if there is none, agree acceptance criteria, plan, move it to the in-progress state, create the branch or worktree, and implement with ticket-referencing commits. Use when the user starts work on a ticket, describes something they want built, or types /dev-task. | `.claude/skills/dev-task/SKILL.md` |
| dev-tdd | Drive an agreed acceptance criterion through red/green/refactor — a test confirmed to fail for the intended reason before any production code, then the least code that passes it. Use when /dev-task hands off at implementation, or when the user types /dev-tdd. | `.claude/skills/dev-tdd/SKILL.md` |
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
