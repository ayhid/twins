# Roadmap: twins — engine and CLI (workstream `default`)

## Overview

This workstream owns the engine and the CLI. twins already scans and reports duplicates from a Rust
workspace (step 1 of issue #1 is done and is not planned again). The scan pipeline moves into
`twins-core` first so every shell can watch and cancel it. Next comes the single deletion routine
every later surface relies on: keep-one by identity, Trash by default, a write-ahead journal and
undo. APFS clones, then the hash cache and config, finish the CLI. Workflows follow as TOML files
run with `twins run`, and the stream closes with a parity check against Go v0 (`9024929`) on the
released build. Through every phase the Core Value holds: never lose data.

Phase numbers are shared across workstreams so requirement IDs and phase directories stay stable.

## Cross-workstream dependencies

| This stream needs | From | For |
|---|---|---|
| release Phase 9 | `release` | Phase 10 checks the released build |

## Phases

- [ ] **Phase 1: Core Pipeline Refactor** - Scan orchestration lives in `twins-core`, any caller can watch progress and cancel, MSRV 1.90
- [ ] **Phase 2: Safe Deletion** - `twins clean` removes duplicates to the Trash with keep-one by identity, protected folders, write-ahead journal, dry-run and undo
- [ ] **Phase 3: APFS Clone Mode** - `twins clean --link` frees space by swapping duplicates for APFS clones, keeping every path
- [ ] **Phase 4: Hash Cache and Config** - Unchanged folders rescan near-instantly and defaults live in `config.toml`
- [ ] **Phase 6: Workflow Model and CLI Runner** - Workflows are TOML files run with `twins run`, report-only by default, auto-clean Trash-only within caps
- [ ] **Phase 10: Go v0 Parity** - Every documented Go v0 behaviour is checked against the Rust version and every gap is closed or recorded

## Phase Details

### Phase 1: Core Pipeline Refactor

**Goal**: The scan pipeline runs inside `twins-core`, and any caller (CLI now, app and agent later) can follow its progress by stage and cancel it, on the Rust 1.90 toolchain Tauri needs
**Mode:** mvp
**Depends on**: Nothing (first phase; builds on the completed step 1 of #1)
**Requirements**: CORE-01, CORE-02, CORE-03, CORE-04
**Success Criteria** (what must be TRUE):
  1. `twins scan` and `twins report` produce the same groups and the same JSON (schema v1) as before, with orchestration moved out of `twins-cli/src/run.rs` into `twins-core`, and the existing test suite passes
  2. A long `twins scan` in a terminal still shows single-line progress by stage (walk, size grouping, partial hash, full hash), now driven by the core observer, and a test observer receives the same stage events in order
  3. Pressing Ctrl+C during a scan stops it promptly (within about a second on a large tree), prints that it was cancelled and writes no partial report
  4. The report's `meta.dry_run` is taken from the run's real mode instead of a hardcoded `false`, and a test fails if a dry-run report says otherwise
  5. The workspace builds and passes tests and clippy (pedantic) on Rust 1.90, with `rust-version` and CI raised to match

**Plans:** 8/9 plans executed

Plans:
**Wave 1**
- [x] 01-01-PLAN.md — Tracer: `twins scan` runs through `twins_core::pipeline::scan` (characterization tests first), `meta.dry_run` derived from `RunMode` (wave 1)

**Wave 2** *(blocked on Wave 1 completion)*
- [x] 01-02-PLAN.md — Hashing stages announce themselves; ordered `StageStarted` with step/steps; cancel at every stage (wave 2)
- [x] 01-03-PLAN.md — Core `Throttle` + `group_digits`; CLI `TerminalObserver` with `[k/N]` progress, also with `--json` on a TTY (wave 2)
- [x] 01-04-PLAN.md — MSRV 1.90: `rust-version`, lint fixes, CI `msrv` job (wave 2)

**Wave 3** *(blocked on Wave 2 completion)*
- [x] 01-05-PLAN.md — Walk progress hook and prompt cancel in the stat phase (wave 3)
- [x] 01-06-PLAN.md — Mid-file cancel for full hash and verify; interrupted reads never reported or digested (wave 3)

**Wave 4** *(blocked on Wave 3 completion)*
- [x] 01-07-PLAN.md — Ctrl+C: `scan cancelled`, exit 130, double Ctrl+C escape hatch, real-signal tests, phase gate on stable and 1.90 (wave 4)

**Gap closure** *(UAT gaps G-01-4, G-01-1)*
- [x] 01-08-PLAN.md — G-01-4: stable clippy 1.99 `assert_is_empty` fix plus a `--keep-going` workspace sweep; CI test and msrv job commands pass locally, user confirms CI after the next push (wave 1)
- [ ] 01-09-PLAN.md — G-01-1: readable text report: `keep`/`remove` labels, each group's folder printed once, blank line between groups; JSON schema v1 unchanged (wave 2, after 01-08)

### Phase 2: Safe Deletion

**Goal**: User can remove duplicates from the CLI knowing that at least one physical copy of every group survives, every operation is journaled before and after it happens, and the default is the Trash with undo
**Mode:** mvp
**Depends on**: Phase 1
**Requirements**: SAFE-01, SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, SAFE-07, DEL-01, DEL-02, DEL-03, DEL-04, DEL-05, DEL-06, DEL-07, DEL-08, DEL-09
**Success Criteria** (what must be TRUE):
  1. User runs `twins clean <path>`, reviews the plan, confirms (or passes `--yes`), and duplicates move to the Trash using the chosen keep strategy (`oldest`, `newest`, `shortest-path`, `in-dir DIR` falling back to `oldest`), with each kept copy showing the rule that chose it
  2. `twins clean --dry-run` goes through the same code path as a real run, journals exactly what would happen and leaves the filesystem unchanged; every real operation (run id, path, Trash location, size, hash, mode, result) is appended to `~/Library/Logs/twins/operations.log` before and after it runs
  3. No plan can remove the last physical copy of a group: keep-one is checked by (volume, inode), hardlinks of the keeper are never removed, nothing under a `--protect DIR` folder is removed, and protected system paths cannot be reached through case variants (`/library`), `/Volumes/<boot>/…` aliases or symlinked roots. Property tests cover these aliases
  4. If a keeper or target changed between scan and clean (identity, size, mtime, ctime or content, re-hashed from disk), the whole group is skipped and reported; if a Trash move fails the file is skipped and never permanently deleted; a second clean started while one is running (from another process) cannot run at the same time
  5. User can delete permanently only after typing "permanent" interactively, or with `--force` in scripts, and can restore a past Trash run with `twins undo <run-id>` from the journal's recorded Trash locations

**Plans**: TBD

### Phase 3: APFS Clone Mode

**Goal**: User can reclaim the space taken by duplicates without losing any path, by replacing copies with APFS clones of the keeper
**Mode:** mvp
**Depends on**: Phase 2
**Requirements**: CLONE-01, CLONE-02, CLONE-03, CLONE-04
**Success Criteria** (what must be TRUE):
  1. `twins clean --link` replaces each duplicate with an APFS clone of the keeper: every original path still opens with identical content, the freed space shows up as available disk space, and the run is journaled and supports `--dry-run` like any other mode
  2. A cloned file keeps its own permissions, dates, Finder tags and extended attributes, and the swap is atomic: if anything fails, the original file is left untouched
  3. Clone mode refuses files on different volumes or on non-APFS volumes, reports why for each affected group, and leaves those files in place
  4. Rescanning a folder after a `--link` run does not count files that already share clone storage as reclaimable space

**Plans**: TBD

### Phase 4: Hash Cache and Config

**Goal**: A second scan of an unchanged folder is near-instant, and the user sets their defaults once in `config.toml` instead of repeating flags
**Mode:** mvp
**Depends on**: Phase 1 for the cache, which can run in parallel with Phases 2-3; config keys for keep, delete mode and protected folders need Phase 2 and Phase 3
**Requirements**: CACHE-01, CACHE-02, CACHE-03, CONF-01, CONF-02
**Success Criteria** (what must be TRUE):
  1. Rescanning an unchanged 16 000-file folder matches or approaches the Go v0 warm time (0.15 s); a file that was edited, replaced or touched is rehashed, and deletion's pre-action re-verification still reads content from disk, never from the cache
  2. Several twins processes scanning at the same time share the cache in `~/Library/Application Support/twins/` without corruption or lock errors
  3. Cancelling a scan halfway keeps the hashes already computed, so the next scan of the same folder is faster
  4. User sets defaults in `config.toml` (min size, excludes, keep strategy, keep dir, delete mode, protected folders, include flags, jobs), and any flag given on the command line overrides the file
  5. `twins config show` prints the effective settings, `config path` prints where the file lives, `config init` writes a starter file and `config clear-cache` empties the hash cache

**Plans**: TBD

### Phase 6: Workflow Model and CLI Runner

**Goal**: User can define a workflow as a TOML file and run it from the CLI, where it only reports by default and an auto-clean outcome can only move files to the Trash, within caps
**Mode:** mvp
**Depends on**: Phase 2, Phase 4 (independent of the app)
**Requirements**: WFL-01, WFL-02, WFL-05, WFL-06
**Success Criteria** (what must be TRUE):
  1. User writes a workflow file (paths, excludes, min size, file-type filters, protected folders, trigger, keep strategy, outcome); `twins workflow validate` accepts it or names each problem, and `twins workflow list` shows every workflow
  2. `twins run <workflow>` scans the workflow's scope and, with the default report-only outcome, reports what it found and changes nothing
  3. A workflow set to auto-clean moves duplicates to the Trash through the core deletion routine, with a journal entry, and no workflow setting can select permanent deletion
  4. Auto-clean falls back to report-only, and says why, when a run would exceed its caps (max files, max bytes, max share of scope) or when the workflow's scope or keep strategy changed since its last run

**Plans**: TBD

### Phase 10: Go v0 Parity

**Goal**: The Rust twins is confirmed to do everything the Go v0 did, and every difference is a recorded, intended choice
**Mode:** mvp
**Depends on**: release Phase 9 (checks the released build; the CLI behaviours under test are in place after Phase 4)
**Requirements**: PAR-01
**Success Criteria** (what must be TRUE):
  1. Every behaviour in the Go v0 README at `9024929` (scan, clean, report, keep strategies, delete modes, dry-run, journal, cache, config) has a recorded Rust status: matches, gap closed, or intentionally different
  2. The same fixture tree run through Go v0 and the Rust version gives the same duplicate groups and the same keep decisions for every keep strategy both versions share
  3. The 16 000-file Downloads benchmark matches or beats Go v0 (1.5 s cold, 0.15 s warm)
  4. Every remaining difference (for example the dropped terminal UI or journal-based undo replacing Put Back) is recorded as intended in PROJECT.md

**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 6 → 10

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Pipeline Refactor | 8/9 | In Progress|  |
| 2. Safe Deletion | 0/TBD | Not started | - |
| 3. APFS Clone Mode | 0/TBD | Not started | - |
| 4. Hash Cache and Config | 0/TBD | Not started | - |
| 6. Workflow Model and CLI Runner | 0/TBD | Not started | - |
| 10. Go v0 Parity | 0/TBD | Not started | - |
