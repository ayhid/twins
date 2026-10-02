# twins

## What This Is

twins finds and removes duplicate files on macOS, safely. It is being rebuilt in Rust as a Cargo
workspace (`twins-core`, `twins-cli`, `twins-app`). A Tauri v2 + Svelte desktop app is the main
product and a companion CLI covers scripting. On top of the rewrite comes a workflow builder,
similar in spirit to n8n, for defining what to scan and when to run it. It is a personal tool
first that also happens to be distributed (Homebrew cask, signed and notarized).

## Core Value

**Never lose data.** Deletion must be safe above everything else: at least one copy of every
group always survives, the Trash is the default, every operation is journaled and `--dry-run`
changes nothing. Speed, automation and polish all come second to this.

## Requirements

### Validated

- ✓ Rust workspace with `twins-core` (engine) and `twins-cli` (`twins` binary), with the Go source removed from the tree — step 1 of #1
- ✓ Filesystem walk with safety exclusions (`.git`, `node_modules`, caches, Time Machine, `~/Library` opt-in, bundles never opened, symlinks never followed, iCloud placeholders never downloaded, network volumes skipped) — existing
- ✓ Protected system paths (`/System`, `/Library`, `/usr`, `/Applications`, …) are never scanned — existing
- ✓ Duplicate pipeline: group by size → hardlink dedup on (dev, inode) → partial xxHash64 (first and last 16 KiB) → full BLAKE3, with optional byte-by-byte `--verify` — existing
- ✓ `oldest` keep strategy (earliest mtime, then shallowest path, then lexical order); hardlinks of the kept file are never listed for removal — existing
- ✓ Read-only `twins scan`, `twins report` (versioned JSON schema v1) and `twins version` — existing
- ✓ Flags: `--json`, `--min-size`, `--exclude`, `--jobs`, `--include-empty`, `--include-library`, `--include-node-modules`, `--include-remote`, `--verify`, `--verbose` — existing

### Active

**Step 2: Safe deletion**
- [ ] Keep-one invariant enforced and validated before any operation runs
- [ ] Keep strategies `oldest`, `newest`, `shortest-path`, `in-dir` (+ keep dir, falling back to `oldest`)
- [ ] Delete modes: trash via NSFileManager (Finder Put Back works), permanent (typed confirmation, `--force` for scripts), APFS clonefile link
- [ ] `--dry-run` that journals and changes nothing
- [ ] Operations journal (JSON lines in `~/Library/Logs/twins/operations.log`)
- [ ] `twins clean` command (interactive confirm, `--yes` for scripts)

**Step 3: Cache and config**
- [ ] Hash cache keyed by (dev, inode, size, mtime) in `~/Library/Application Support/twins/`
- [ ] TOML config (`config.toml`); flags override the file
- [ ] `twins config show | path | init | clear-cache`

**Step 4: Desktop app (Tauri v2 + Svelte)**
- [ ] Menu / home, scan with live progress
- [ ] Group browser (expand groups, filter, reveal in Finder, Quick Look)
- [ ] Selection with keep-strategy auto-select; the last copy can never be marked
- [ ] Confirmation and execution through the same core deletion path as the CLI

**Workflows: custom rules and automation**
- [ ] A workflow defines scan scope (paths, excludes, size/type filters), a trigger and an outcome
- [ ] Triggers: manual, schedule, folder change (FSEvents)
- [ ] Per-workflow outcome: report-only (notify) or auto-clean (Trash + journal, using a keep strategy); report-only is the default
- [ ] Workflows are saved as files and run from both the app and the CLI (`twins run <workflow>`)
- [ ] Builder UI: node-graph canvas (n8n-style) as the model, with a simpler ordered rule-list view over it
- [ ] Scheduled and folder-change workflows run without the app open (background agent)

**Step 5: Release**
- [ ] semantic-release: `main` → beta prerelease, `stable` → release
- [ ] Tauri bundler, Developer ID signing and notarization
- [ ] Homebrew cask

**Parity**
- [ ] Feature parity with Go v0 (commit `9024929`) verified against its recorded behaviour

### Out of Scope

- Terminal UI (the Go v0 bubbletea browser) — replaced by the desktop app; the CLI stays scriptable, not interactive-browsing
- Non-macOS platforms — trash, clonefile, FSEvents and Full Disk Access are macOS-specific
- Workflow rules that override the keep decision or the action per file — not requested; workflows control scope, triggers and report-only vs auto-clean, and reuse the standard keep strategies
- Unattended permanent deletion — automation only ever trashes (Core Value)
- Volume-mounted triggers — not needed for v1
- Network/remote volume dedup by default — still opt-in via `--include-remote`

## Context

- Brownfield: codebase mapped in `.planning/codebase/` (commit `c962eec`). Engine is `crates/twins-core` (scan, group/find, keep, hash, report, safety, fsutil); CLI is `crates/twins-cli`.
- The Go v0 at commit `9024929` is the reference for behaviour parity (README there documents clean, keep strategies, delete modes, journal, cache, config).
- Tracked in GitHub issue #1 (ayhid/twins); step 1 is done (commits up to `c962eec`).
- Concerns already noted in the map: deletion unimplemented, `meta.dry_run` hardcoded false in `run.rs`, no cache, no config file.
- Go v0 benchmark to beat or match: 16 000-file Downloads folder, 1.5 s cold, 0.15 s warm cache.

## Constraints

- **Platform**: macOS only. Needs Full Disk Access for restricted folders.
- **Tech stack**: Rust 1.85+, edition 2024, workspace lints (`unsafe_code` warn, clippy pedantic); Tauri v2 + Svelte for the app.
- **Safety**: every deletion path (CLI, app, workflow) goes through one core routine that enforces keep-one, validates the plan and writes the journal.
- **Architecture**: `twins-core` holds all logic; the CLI, the app and the workflow runner are thin shells over it.
- **Distribution**: signed and notarized builds through a Homebrew cask.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rewrite in Rust, Go kept only in history | Tauri needs a Rust core | ✓ Good (step 1 shipped) |
| Tauri v2 + Svelte for the desktop app | Small bundle, Rust backend shares `twins-core` | — Pending |
| Workflows control scope + trigger + report/auto-clean, not keep/action logic | User's stated need; keeps the safety surface small | — Pending |
| Workflow builder: graph model with a list view on top | User wants n8n-style power plus a simpler view | — Pending |
| Workflows runnable from both app and CLI | Automation must work without the UI | — Pending |
| Auto-clean is per workflow, report-only by default, Trash only | Core Value: never lose data | — Pending |
| Order: deletion → cache/config → app → workflows → release | Each layer depends on the one before it | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-10-02 after initialization*
