# Requirements: twins — desktop app and automation (workstream `app`)

**Defined:** 2026-10-02
**Core Value:** Never lose data. At least one copy of every group always survives, the Trash is the default, every operation is journaled and dry-run changes nothing.

## v1 Requirements

The v1 requirements owned by this workstream. The full set of 63 is split across `default`, `app` and `release`; IDs are unchanged.

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

- [ ] **WFL-03**: User can create and edit workflows in the app with an ordered list editor (steps: trigger → scope → filters → keep → outcome)
- [ ] **WFL-04**: User can preview (test-run) a workflow as a dry run before enabling it
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
| WFL-03 | Phase 7 | Pending |
| WFL-04 | Phase 7 | Pending |
| WFL-07 | Phase 8 | Pending |
| WFL-08 | Phase 7 | Pending |
| WFL-09 | Phase 7 | Pending |
| AGENT-01 | Phase 8 | Pending |
| AGENT-02 | Phase 8 | Pending |
| AGENT-03 | Phase 8 | Pending |
| AGENT-04 | Phase 8 | Pending |
| AGENT-05 | Phase 8 | Pending |
| AGENT-06 | Phase 8 | Pending |

**Coverage:**
- v1 requirements in this stream: 25
- Mapped to phases: 25
- Unmapped: 0 ✓

---
*Requirements defined: 2026-10-02*
*Last updated: 2026-10-02 after split into workstreams*
