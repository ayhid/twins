# Requirements: twins — engine and CLI (workstream `default`)

**Defined:** 2026-10-02
**Core Value:** Never lose data. At least one copy of every group always survives, the Trash is the default, every operation is journaled and dry-run changes nothing.

## v1 Requirements

The v1 requirements owned by this workstream. The full set of 63 is split across `default`, `app` and `release`; IDs are unchanged.

### Core Foundation

- [ ] **CORE-01**: The scan pipeline runs inside `twins-core` and reports staged progress to any caller (CLI, app, agent) through an observer
- [ ] **CORE-02**: User can cancel a running scan or clean, and the operation stops promptly without leaving partial state
- [ ] **CORE-03**: The JSON report's `dry_run` field reflects the real mode of the run instead of being hardcoded
- [ ] **CORE-04**: The workspace builds on Rust 1.90 (the MSRV Tauri needs) with existing tests passing

### Safety

- [ ] **SAFE-01**: Protected system paths are matched by file identity (device, inode), not by text, so case variants (`/library`), `/Volumes/<boot>/…` aliases and symlinked roots cannot bypass them
- [ ] **SAFE-02**: Scan roots are canonicalised and symlinked roots are never followed into protected locations
- [ ] **SAFE-03**: User can mark protected (reference) folders, and no file under them is ever removed, from the CLI (`--protect DIR`), config, the app and workflows
- [ ] **SAFE-04**: A deletion plan is rejected unless every group keeps at least one physical copy, checked by (volume, inode) identity so hardlinks and aliases cannot fake a survivor
- [ ] **SAFE-05**: Immediately before acting on a group, twins re-checks the keeper and every target against the live filesystem (identity, size, mtime, ctime) and re-verifies content without trusting the cache. If anything drifted, it skips the whole group and reports it
- [ ] **SAFE-06**: Only one deletion runs at a time across CLI, app and agent (global execute lock)
- [ ] **SAFE-07**: If moving a file to the Trash fails (e.g. a volume without a Trash), the file is skipped. twins never falls back to permanent deletion

### Deletion

- [ ] **DEL-01**: User can choose the keep strategy `oldest`, `newest`, `shortest-path` or `in-dir` (with a keep directory, falling back to `oldest`)
- [ ] **DEL-02**: User can see why each kept copy was chosen (the strategy rule that decided it)
- [ ] **DEL-03**: User can move duplicates to the Trash (default mode)
- [ ] **DEL-04**: User can delete duplicates permanently only after typing "permanent" interactively, or with `--force` in scripts
- [ ] **DEL-05**: User can run any clean with `--dry-run`, which journals what would happen and changes nothing, through the same code path as a real run
- [ ] **DEL-06**: Every operation (run id, original path, Trash location, size, hash, mode, result) is appended to `~/Library/Logs/twins/operations.log` as JSON lines before and after it happens
- [ ] **DEL-07**: User can run `twins clean [path...]` to scan, review and confirm interactively, or `--yes` to skip confirmation in scripts
- [ ] **DEL-08**: User can restore a past run from the CLI with `twins undo <run-id>`, using the journal's recorded Trash locations (Finder Put Back is best-effort only)
- [ ] **DEL-09**: Hardlinks of the kept file are never removed

### APFS Clones

- [ ] **CLONE-01**: User can replace duplicates with APFS clones (`--link`), so the paths remain and the space is freed
- [ ] **CLONE-02**: A cloned file keeps its own permissions, dates, tags and extended attributes, and is swapped in atomically, so a failure leaves the original untouched
- [ ] **CLONE-03**: Clone mode refuses files on different volumes or non-APFS volumes and reports why
- [ ] **CLONE-04**: Files that already share APFS clone storage are recognised and are not counted as reclaimable space on rescans

### Cache & Config

- [ ] **CACHE-01**: Hashes are cached in `~/Library/Application Support/twins/`, keyed by volume UUID, inode, size, mtime and ctime, so a second scan of an unchanged folder is near-instant (Go v0 baseline: 16 000 files, 0.15 s warm)
- [ ] **CACHE-02**: The cache is safe to use from the CLI, app and agent at the same time
- [ ] **CACHE-03**: A cancelled scan keeps the hashes it already computed
- [ ] **CONF-01**: User can set defaults in `config.toml` (min size, excludes, keep strategy, keep dir, delete mode, protected folders, include flags, jobs), and flags override the file
- [ ] **CONF-02**: User can run `twins config show`, `config path`, `config init` and `config clear-cache`

### Workflows

- [ ] **WFL-01**: A workflow is a TOML file that defines scan scope (paths, excludes, min size, file-type filters, protected folders), trigger, keep strategy and outcome
- [ ] **WFL-02**: User can run a workflow from the CLI with `twins run <workflow>`, and list and validate workflows with `twins workflow list` and `twins workflow validate`
- [ ] **WFL-05**: Each workflow's outcome is either report-only (default) or auto-clean, and auto-clean only ever moves files to the Trash
- [ ] **WFL-06**: Auto-clean falls back to report-only when a run exceeds its caps (max files, max bytes, max share of scope), or when the workflow's scope or strategy changed since its last run

### Parity

- [ ] **PAR-01**: Every behaviour documented in the Go v0 README at commit `9024929` (scan, clean, report, keep strategies, delete modes, dry-run, journal, cache, config) is checked against the Rust version, and every gap is either closed or recorded as intended

## v2 Requirements

Deferred. Tracked, but not in the current roadmap.

### Workflow Builder

- **WFL-V2-01**: User can edit workflows on a node-graph canvas (n8n-style) kept in sync with the list view

### App

- **APP-V2-01**: User can Quick Look a file from the group browser
- **APP-V2-02**: User can reveal a file in Finder from the group browser

### Triggers & Detection

- **AGENT-V2-01**: A workflow can run when a volume is mounted
- **DET-V2-01**: Duplicate folder detection and merge

## Out of Scope

| Feature | Reason |
|---------|--------|
| Terminal UI (Go v0 bubbletea browser) | Replaced by the desktop app; the CLI stays scriptable |
| Non-macOS platforms | Trash, clonefile, FSEvents, SMAppService and Full Disk Access are macOS-specific |
| Workflow rules that override keep or action per file | Not requested; workflows control scope, trigger and outcome only, which keeps the safety surface small |
| Unattended permanent deletion | Core Value: automation only ever moves files to the Trash |
| Deleting the last copy of a group | Core Value: enforced in core, with no override |
| Fuzzy or similar-file matching | twins is exact-duplicate only |
| Replacing duplicates with symlinks or hardlinks | Breaks apps and backups silently; APFS clones cover space saving |
| Deduplication inside Photos or Music libraries | Corrupts the library database; bundles are never opened |
| Script, HTTP or arbitrary-action workflow nodes | Not a general automation tool; keeps workflows auditable |
| Workflow chaining | Unnecessary complexity for v1 |
| In-app updater | The Homebrew cask owns updates |
| App Sandbox | Incompatible with scanning arbitrary folders and Full Disk Access |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CORE-01 | Phase 1 | Pending |
| CORE-02 | Phase 1 | Pending |
| CORE-03 | Phase 1 | Pending |
| CORE-04 | Phase 1 | Pending |
| SAFE-01 | Phase 2 | Pending |
| SAFE-02 | Phase 2 | Pending |
| SAFE-03 | Phase 2 | Pending |
| SAFE-04 | Phase 2 | Pending |
| SAFE-05 | Phase 2 | Pending |
| SAFE-06 | Phase 2 | Pending |
| SAFE-07 | Phase 2 | Pending |
| DEL-01 | Phase 2 | Pending |
| DEL-02 | Phase 2 | Pending |
| DEL-03 | Phase 2 | Pending |
| DEL-04 | Phase 2 | Pending |
| DEL-05 | Phase 2 | Pending |
| DEL-06 | Phase 2 | Pending |
| DEL-07 | Phase 2 | Pending |
| DEL-08 | Phase 2 | Pending |
| DEL-09 | Phase 2 | Pending |
| CLONE-01 | Phase 3 | Pending |
| CLONE-02 | Phase 3 | Pending |
| CLONE-03 | Phase 3 | Pending |
| CLONE-04 | Phase 3 | Pending |
| CACHE-01 | Phase 4 | Pending |
| CACHE-02 | Phase 4 | Pending |
| CACHE-03 | Phase 4 | Pending |
| CONF-01 | Phase 4 | Pending |
| CONF-02 | Phase 4 | Pending |
| WFL-01 | Phase 6 | Pending |
| WFL-02 | Phase 6 | Pending |
| WFL-05 | Phase 6 | Pending |
| WFL-06 | Phase 6 | Pending |
| PAR-01 | Phase 10 | Pending |

**Coverage:**
- v1 requirements in this stream: 34
- Mapped to phases: 34
- Unmapped: 0 ✓

**Cross-phase surfaces:** SAFE-03 (protected folders) is enforced in core and exposed as `--protect` in Phase 2. Its config and workflow surfaces are checked in Phase 4 (CONF-01) and Phase 6 (WFL-01); its app surface in app Phase 5 (success criterion 3).

---
*Requirements defined: 2026-10-02*
*Last updated: 2026-10-02 after split into workstreams*
