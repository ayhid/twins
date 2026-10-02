# Requirements: twins — release pipeline (workstream `release`)

**Defined:** 2026-10-02
**Core Value:** Never lose data. At least one copy of every group always survives, the Trash is the default, every operation is journaled and dry-run changes nothing.

## v1 Requirements

The v1 requirements owned by this workstream. The full set of 63 is split across `default`, `app` and `release`; IDs are unchanged.

### Release

- [ ] **REL-01**: Every push to `main` publishes a beta prerelease, and fast-forwarding `stable` publishes a release (semantic-release)
- [ ] **REL-02**: The app, CLI and agent binaries are signed (Developer ID, hardened runtime) and notarized
- [ ] **REL-03**: User can install with `brew install --cask ayhid/tap/twins` (or `twins@beta`), which puts the `twins` CLI on the PATH, and uninstalling unregisters the agent
- [ ] **REL-04**: CI runs tests, lints and a smoke test of the built bundle on every push

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
| REL-01 | Phase 9 | Pending |
| REL-02 | Phase 9 | Pending |
| REL-03 | Phase 9 | Pending |
| REL-04 | Phase 9 | Pending |

**Coverage:**
- v1 requirements in this stream: 4
- Mapped to phases: 4
- Unmapped: 0 ✓

---
*Requirements defined: 2026-10-02*
*Last updated: 2026-10-02 after split into workstreams*
