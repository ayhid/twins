---
last_mapped_commit: c962eec129ca808d6d8cde2e6ee924fc6b1fccc7
last_mapped_at: 2026-10-02
---
<!-- refreshed: 2026-10-02 -->

# Architecture

**Analysis Date:** 2026-10-02

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                      CLI Entry Point                         │
│                   `crates/twins-cli/src/`                    │
│                 (main.rs, cli.rs, run.rs)                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Scan & Collection Layer                   │
│  `crates/twins-core/src/scan/` and `fsutil.rs`               │
│                                                               │
│  - Walk filesystem with safety checks                        │
│  - Collect candidate files (FileMeta snapshots)              │
│  - Index candidates by size                                  │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│               Duplicate Detection Pipeline                   │
│         `crates/twins-core/src/group/find.rs`                │
│                                                               │
│  Stage 1: Group by size (drop unique sizes)                 │
│  Stage 2: Partial hash (first/last 16 KiB)                  │
│  Stage 3: Full hash (BLAKE3 on survivors)                   │
│  Stage 4: Verify (optional byte-by-byte)                    │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Keep Strategy & Action Planning                 │
│         `crates/twins-core/src/group/keep.rs`                │
│                                                               │
│  - Choose which copy to keep (oldest, newest, etc.)         │
│  - Mark duplicates for removal (hardlinks exempt)            │
│  - Calculate reclaimable space                              │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Report Generation & Output                      │
│         `crates/twins-core/src/report.rs`                    │
│                                                               │
│  - JSON report (machine-readable, versioned)                │
│  - Text report (human-readable with progress)               │
└─────────────────────────────────────────────────────────────┘
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

**Overall:** Two-crate workspace with a scan-hash-group-report pipeline.

**Key Characteristics:**
- **Layered pipeline**: Each stage only processes what the previous stage couldn't rule out
- **Concurrency**: Rayon thread pools for filesystem walk and hashing (separate pools, different parallelism)
- **Immutable snapshots**: FileMeta records point-in-time metadata; no shared mutable state except Index
- **Safety-first**: Protected paths checked before scan; hardlinks never deleted
- **Progressive filtering**: ~90% of files eliminated by size alone; most remaining by partial hash

## Layers

**CLI Layer:**
- Purpose: Parse command-line arguments and dispatch to scan or report
- Location: `crates/twins-cli/src/`
- Contains: Clap-derived argument structs, version, subcommand routing
- Depends on: `twins-core`, `anyhow`, `clap`
- Used by: User directly via the `twins` binary

**Scan & Collection Layer:**
- Purpose: Traverse filesystem safely, collect file metadata, validate roots
- Location: `crates/twins-core/src/scan/` and `fsutil.rs`
- Contains: Walk logic with WalkDir, exclusion rules, volume locality checks, iCloud detection
- Depends on: `walkdir`, `globset`, `libc` (for volume checks)
- Used by: Run handler to populate Index with candidates

**Index & Grouping Layer:**
- Purpose: Hold candidates in memory, organize by size, refine through hashing
- Location: `crates/twins-core/src/group/`
- Contains: Index (concurrent, size-keyed), Group (immutable set of duplicates), Strategy (keep decisions)
- Depends on: `rayon` for parallel hashing
- Used by: Find pipeline to build Groups from candidates

**Hashing & Identification Layer:**
- Purpose: Compute fingerprints (xxHash64 partial, BLAKE3 full) and compare
- Location: `crates/twins-core/src/hash.rs` and `group/hasher.rs`
- Contains: Partial hash (head/tail samples), full hash (streaming), byte-by-byte compare, Digest type
- Depends on: `blake3`, `xxhash-rust`
- Used by: Find pipeline to eliminate non-duplicates

**Reporting Layer:**
- Purpose: Serialize decisions and statistics as JSON or text
- Location: `crates/twins-core/src/report.rs`
- Contains: Report struct (versioned), Summary and GroupEntry types, JSON/text writers
- Depends on: `serde_json`
- Used by: Run handler to output results

**Safety Layer:**
- Purpose: Protect system directories and user homes from accidental scans
- Location: `crates/twins-core/src/safety.rs`
- Contains: Protected path list (/System, /Library, /usr, etc.), User::Library detection
- Depends on: None (filesystem introspection only)
- Used by: Scan walk to validate roots and directories

**Human-Readable Utilities:**
- Purpose: Parse human sizes (1K, 1MiB) and format output (numbers, times)
- Location: `crates/twins-core/src/human.rs`
- Contains: ParseSizeError, parse_size, human_size functions
- Depends on: None
- Used by: CLI run handler and report builder

## Data Flow

### Primary Request Path (Scan Command)

1. **Parse arguments** (`crates/twins-cli/src/main.rs:12-20`)
   - Clap parses `scan`, `report`, or `version` subcommand
   - Routes to `run::scan(&args, false)` or `run::scan(&args, true)`

2. **Validate roots & build options** (`crates/twins-cli/src/run.rs:18-29`)
   - Normalize root paths (default to `$HOME`)
   - Create `scan::Options` with size, exclusions, workers, include flags
   - Instantiate `group::Index` (thread-safe size-keyed map)

3. **Walk filesystem** (`crates/twins-cli/src/run.rs:34-41`)
   - `scan::walk()` traverses roots with `walkdir::WalkDir`
   - Filters by safety, exclusions, rules (skip `.git`, `node_modules`, etc.)
   - Detects iCloud placeholders (`SF_DATALESS` flag), symlinks, bundles
   - Collects `FileMeta` snapshots (path, size, mtime, inode, device)
   - Each file's metadata added to `Index` by size

4. **Find duplicates** (`crates/twins-cli/src/run.rs:52`)
   - `group::find(&idx, &find_opts)` runs the refinement pipeline:
     - **Size grouping**: Index returns size buckets with 2+ physical identities
     - **Partial hash**: xxHash64 on first/last 16 KiB (two reads max)
     - **Full hash**: BLAKE3 on survivors only (parallel, 8 workers max)
     - **Verify** (optional): byte-by-byte comparison after BLAKE3
   - Returns `Vec<Group>` of confirmed duplicates, largest first

5. **Plan deletions** (`crates/twins-cli/src/run.rs:56-57`)
   - `group::plan(&groups, &keeper)` applies keep strategy to each group
   - Keeper picks which copy survives (oldest by default)
   - Hardlinks of kept file never marked for removal
   - Returns `Vec<Action>` with keep/remove decisions

6. **Build & render report** (`crates/twins-cli/src/run.rs:65-70`)
   - `report::build(&actions, &meta, now)` assembles Report with:
     - Summary (files scanned, candidates, groups found, reclaimable bytes)
     - GroupEntry per duplicate (members, keep decision, reclaimable space)
   - Writes JSON (if `--json`) or text (default) to stdout
   - Text output includes progress on stderr (if not piped)

### Secondary Flow: Hardlink Detection

- During scan walk, `FileMeta` captures `(device, inode)` as `Identity`
- `Group::physical()` counts distinct identities → hardlinks count once
- Hardlinks are never removed: deletion would waste no space and risk data loss
- `Action::reclaimable()` only counts truly distinct physical copies

**State Management:**
- **Immutable**: FileMeta, Group, Identity (thread-safe by value)
- **Mutable**: Index (Mutex-wrapped HashMap for concurrent `add`)
- **Concurrent writes**: Index::add from multiple walk threads
- **Concurrent reads**: Index::candidates accessed by main thread after walk completes

## Key Abstractions

**FileMeta:**
- Purpose: Immutable snapshot of a regular file's identity and size
- Examples: `crates/twins-core/src/fsutil.rs`
- Pattern: Value type (Clone, Copy where possible); read via `stat()` once per file
- Contains: path, size, mtime, identity (device, inode), nlink, dataless flag

**Identity:**
- Purpose: Uniquely identify physical data on disk via `(device, inode)` pair
- Examples: Compared to detect hardlinks in `Group::physical()`
- Pattern: Hash + Eq + Ord for set operations and grouping

**Group:**
- Purpose: Immutable set of byte-identical files (members sorted by path)
- Examples: Returned by `group::find()`, consumed by `group::plan()`
- Pattern: Built via `Group::new()` with sorted members; provides `files()`, `size()`, `digest()`, `reclaimable()`

**Index:**
- Purpose: Concurrent, size-keyed collection of FileMeta
- Examples: Populated during scan walk, queried for candidates
- Pattern: Mutex-wrapped HashMap with `add()` for writes, `candidates()` for post-walk query
- Thread-safe: Multiple walk threads call `add()` concurrently

**Strategy:**
- Purpose: Policy for which copy to keep in a duplicate group
- Examples: Oldest (default), Newest, ShortestPath, InDir
- Pattern: Enum with `Keeper` applier; tiebreakers (depth, path length, lex order)

**Hasher:**
- Purpose: Trait for fingerprinting (partial and full); extensible for caching
- Examples: DirectHasher (current); future cache could wrap it
- Pattern: `partial(&FileMeta) -> u64`, `full(&FileMeta) -> Digest`

**Digest:**
- Purpose: 256-bit BLAKE3 hash as byte array + Display
- Examples: Group members with identical Digest are byte-identical
- Pattern: Newtype `[u8; 32]` with hex string Display, FromStr parsing

## Entry Points

**`twins scan [PATH...]`:**
- Location: `crates/twins-cli/src/main.rs:12-28`
- Triggers: User runs binary with `scan` subcommand
- Responsibilities: Parse args, call `run::scan(..., false)`, format text or JSON output

**`twins report [PATH...]`:**
- Location: `crates/twins-cli/src/main.rs:14-20`
- Triggers: User runs binary with `report` subcommand
- Responsibilities: Same as scan but always outputs JSON (force `--json`)

**`scan::walk()`:**
- Location: `crates/twins-core/src/scan/walk.rs:77-111`
- Triggers: `run::scan()` invokes it to populate Index
- Responsibilities: Traverse filesystem, filter by rules, call visitor for each candidate

**`group::find()`:**
- Location: `crates/twins-core/src/group/find.rs:191-196`
- Triggers: After scan completes, to refine candidates into Groups
- Responsibilities: Run parallel hashing pipeline, report progress and errors

**`group::plan()`:**
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

**What happens:** If a file is modified or deleted between scan walk and grouping, hashing can fail silently (file not found) or produce incorrect results (partial hash of different content).

**Why it's wrong:** Users expect current state of filesystem to be reported. Stale metadata can lead to incorrect duplication decisions.

**Do this instead:** Accept files can change mid-scan and handle gracefully:
- HashError is reported via `on_error` callback, file is excluded from results
- `scan::walk()` uses snapshot at walk time; follow-up walks are independent
- Future: caching layer would need refresh-on-mtime-change logic

### Hardlinks Treated as Separate Duplicates

**What happens:** If Identity is not checked, two paths with identical content but same `(device, inode)` would be marked for separate removal.

**Why it's wrong:** Deleting one hardlink doesn't free space (data survives on disk). Removing all hardlinks wastes the data.

**Do this instead:** In `Group::physical()` and `Action::reclaimable()`, count distinct identities only. Keeper::choose() ensures hardlinks of kept file are never listed in remove.

### Scanning Protected Paths Without Permission

**What happens:** If roots include `/Library` or `/System` without a check, walk fails with permission denied or produces incomplete results.

**Why it's wrong:** On macOS, some paths require Full Disk Access even for stat. Users get a cryptic error or incomplete results.

**Do this instead:** Use `safety::is_protected()` on roots before walk (lines 82-100 in walk.rs). Refuse protected locations unless explicitly flagged. Require Full Disk Access in docs.

## Error Handling

**Strategy:** Fail-fast on fatal errors (bad roots, invalid globs, thread pool creation), report non-fatal errors via callback, drop problematic files from results.

**Patterns:**
- **Fatal**: ScanError (no roots, protected root, glob parse, thread pool)
- **Non-fatal**: Per-file walk/stat/hash errors reported to `on_error` callback, counted in Stats, file excluded from results
- **User-facing**: Exit code 2 for usage errors (ScanError, ParseSizeError), 1 for I/O errors

## Cross-Cutting Concerns

**Logging:** Errors logged to stderr (verbose mode lists skipped files). Progress printed to stderr if a terminal (single-line update). Controlled by `run::Progress` struct and `on_error` callback.

**Validation:** Roots validated for existence, locality, protection status before walk. Exclusion globs compiled once (ScanError if invalid). File metadata snapshot taken at walk time (no re-stat).

**Authentication:** None (runs as current user; filesystem permissions enforced by OS). macOS Full Disk Access required for protected paths (documented in README).

---

*Architecture analysis: 2026-10-02*
