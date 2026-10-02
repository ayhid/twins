# Requirements: twins

**Defined:** 2026-10-02
**Core Value:** Never lose data. At least one copy of every group always survives, the Trash is the default, every operation is journaled and dry-run changes nothing.

## v1 Requirements

Requirements for this milestone (the rest of #1, plus workflows). Each maps to a roadmap phase.

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

### Desktop App

- [ ] **APP-01**: User can open twins.app, pick folders to scan and start a scan
- [ ] **APP-02**: User is guided to grant Full Disk Access, and sees how many items were skipped for lack of permission
- [ ] **APP-03**: User sees live scan progress by stage and can cancel
- [ ] **APP-04**: User sees a summary (groups, duplicate files, reclaimable bytes)
- [ ] **APP-05**: User can browse groups in a list that stays smooth with tens of thousands of groups, with sort and path filter
- [ ] **APP-06**: User selects which copy to keep in each group, and the UI cannot express a group with zero kept copies
- [ ] **APP-07**: User can auto-select every group with a keep strategy, then adjust by hand
- [ ] **APP-08**: User can apply folder actions ("keep everything in this folder", "remove copies from this folder") across all groups
- [ ] **APP-09**: User reviews a confirmation (files, bytes, mode) before anything is removed, and execution goes through the same core routine as the CLI
- [ ] **APP-10**: User sees per-file results and errors after a clean
- [ ] **APP-11**: User can restore a past run from the app ("Restore this run")
- [ ] **APP-12**: User can see agent status and the last run from a menu bar item
- [ ] **APP-13**: User can right-click a folder in Finder and choose a Quick Action to find duplicates in it with twins
- [ ] **APP-14**: Running bare `twins` opens the app if it is installed, otherwise prints help

### Workflows

- [ ] **WFL-01**: A workflow is a TOML file that defines scan scope (paths, excludes, min size, file-type filters, protected folders), trigger, keep strategy and outcome
- [ ] **WFL-02**: User can run a workflow from the CLI with `twins run <workflow>`, and list and validate workflows with `twins workflow list` and `twins workflow validate`
- [ ] **WFL-03**: User can create and edit workflows in the app with an ordered list editor (steps: trigger → scope → filters → keep → outcome)
- [ ] **WFL-04**: User can preview (test-run) a workflow as a dry run before enabling it
- [ ] **WFL-05**: Each workflow's outcome is either report-only (default) or auto-clean, and auto-clean only ever moves files to the Trash
- [ ] **WFL-06**: Auto-clean falls back to report-only when a run exceeds its caps (max files, max bytes, max share of scope), or when the workflow's scope or strategy changed since its last run
- [ ] **WFL-07**: User can enable or disable each workflow, and pause all automation globally
- [ ] **WFL-08**: User can see run history (when, trigger, outcome, files, bytes, errors) in the app and the CLI
- [ ] **WFL-09**: User can run any workflow manually from the app

### Background Agent & Triggers

- [ ] **AGENT-01**: Scheduled and folder-change workflows run while the app is closed, through a background agent the user can turn on or off from the app
- [ ] **AGENT-02**: A workflow can run on a schedule (daily or weekly at a given time), and a run missed while the Mac was asleep catches up once
- [ ] **AGENT-03**: A workflow can run when a watched folder changes, after activity settles, ignoring partial downloads and twins' own Trash moves
- [ ] **AGENT-04**: Workflow runs are queued one at a time, so two triggers never act on the same files concurrently
- [ ] **AGENT-05**: After a run that found duplicates, the user gets a notification with a "Review" action that opens the run in the app
- [ ] **AGENT-06**: The agent can read the folders it watches (Full Disk Access), and the app tells the user when it cannot

### Release

- [ ] **REL-01**: Every push to `main` publishes a beta prerelease, and fast-forwarding `stable` publishes a release (semantic-release)
- [ ] **REL-02**: The app, CLI and agent binaries are signed (Developer ID, hardened runtime) and notarized
- [ ] **REL-03**: User can install with `brew install --cask ayhid/tap/twins` (or `twins@beta`), which puts the `twins` CLI on the PATH, and uninstalling unregisters the agent
- [ ] **REL-04**: CI runs tests, lints and a smoke test of the built bundle on every push

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

Which phases cover which requirements. Updated during roadmap creation.

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
| APP-01 | Phase 5 | Pending |
| APP-02 | Phase 5 | Pending |
| APP-03 | Phase 5 | Pending |
| APP-04 | Phase 5 | Pending |
| APP-05 | Phase 5 | Pending |
| APP-06 | Phase 5 | Pending |
| APP-07 | Phase 5 | Pending |
| APP-08 | Phase 5 | Pending |
| APP-09 | Phase 5 | Pending |
| APP-10 | Phase 5 | Pending |
| APP-11 | Phase 5 | Pending |
| APP-12 | Phase 8 | Pending |
| APP-13 | Phase 5 | Pending |
| APP-14 | Phase 5 | Pending |
| WFL-01 | Phase 6 | Pending |
| WFL-02 | Phase 6 | Pending |
| WFL-03 | Phase 7 | Pending |
| WFL-04 | Phase 7 | Pending |
| WFL-05 | Phase 6 | Pending |
| WFL-06 | Phase 6 | Pending |
| WFL-07 | Phase 8 | Pending |
| WFL-08 | Phase 7 | Pending |
| WFL-09 | Phase 7 | Pending |
| AGENT-01 | Phase 8 | Pending |
| AGENT-02 | Phase 8 | Pending |
| AGENT-03 | Phase 8 | Pending |
| AGENT-04 | Phase 8 | Pending |
| AGENT-05 | Phase 8 | Pending |
| AGENT-06 | Phase 8 | Pending |
| REL-01 | Phase 9 | Pending |
| REL-02 | Phase 9 | Pending |
| REL-03 | Phase 9 | Pending |
| REL-04 | Phase 9 | Pending |
| PAR-01 | Phase 10 | Pending |

**Coverage:**
- v1 requirements: 63 total
- Mapped to phases: 63
- Unmapped: 0 ✓

**Cross-phase surfaces:** SAFE-03 (protected folders) is enforced in core and exposed as `--protect` in Phase 2. Its config, app and workflow surfaces are checked in Phase 4 (CONF-01), Phase 5 (success criterion 3) and Phase 6 (WFL-01).

---
*Requirements defined: 2026-10-02*
*Last updated: 2026-10-02 after roadmap creation*
