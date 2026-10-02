---
phase: 01-core-pipeline-refactor
plan: 05
subsystem: core
tags: [rust, walkdir, rayon, cancellation, progress]

requires:
  - phase: 01-core-pipeline-refactor
    provides: "01-02 ordered StageStarted events; 01-03 Walk Progress contract (total: None) rendered by the CLI"
provides:
  - "pub(crate) scan::walk_observed with a progress hook fed a Stats snapshot per regular file counted during directory listing"
  - "Walk cancel polled per root, per listed entry, per file in the parallel stat phase and after the stat phase"
  - "Pipeline Walk Progress events carry the walk's file count (stats.files), all before SizeGrouping starts"
affects: [01-07, twins-cli progress, twins-app progress]

actuals:
  tokens: 2232
  tasks: 2
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Crate-private observed variant of a public function: the public fn delegates with a no-op hook, so its signature never changes"
    - "Cancel poll inside rayon par_iter closures: early return per item, then one Cancelled check after pool.install"

key-files:
  created: []
  modified:
    - crates/twins-core/src/scan/walk.rs
    - crates/twins-core/src/scan/mod.rs
    - crates/twins-core/src/pipeline.rs
    - crates/twins-core/tests/scan_test.rs
    - crates/twins-core/tests/pipeline_test.rs

key-decisions:
  - "Walk Progress counts files seen while listing (stats.files), not candidates; the count stays frozen during the stat phase (flagged assumption CORE-01 accepted as planned)"
  - "on_progress is called from the single listing thread, so done values are non-decreasing without extra synchronisation"
  - "The 01-03 interim per-candidate counter in walk_stage is removed; walk_observed is the only walk-progress source"

patterns-established:
  - "Progress hooks take &Stats snapshots; the pipeline maps them to Event::Progress and leaves rate limiting to Throttle"

requirements-completed: [CORE-01, CORE-02]

coverage:
  - id: D1
    description: "A walk whose cancel flag is already raised returns ScanError::Cancelled"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/scan_test.rs#walk_cancelled_before_start"
        status: pass
    human_judgment: false
  - id: D2
    description: "Raising the cancel flag during the parallel stat phase makes scan::walk return Cancelled and stops visiting the remaining files"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/scan_test.rs#walk_cancel_during_stat_phase_stops"
        status: pass
    human_judgment: false
  - id: D3
    description: "The pipeline emits Walk Progress { done: files seen, total: None } during listing, all before StageStarted(SizeGrouping), non-decreasing, ending at stats.files"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#walk_progress_counts_files_before_size_grouping"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/observe_test.rs#walk_reports_a_running_file_count"
        status: pass
    human_judgment: false
  - id: D4
    description: "Public scan::walk signature unchanged; existing scan, pipeline and CLI tests still pass"
    requirement: CORE-01
    verification:
      - kind: other
        ref: "cargo test --workspace"
        status: pass
      - kind: other
        ref: "cargo clippy --workspace --all-targets -- -D warnings"
        status: pass
    human_judgment: false

duration: 4min
completed: 2026-10-02
status: complete
plan_head_before: 09f59c99a38d14858273a9aaa8b166edeff1ece8
plan_head_after: da802ae
---

# Phase 1 Plan 05: Observable, Promptly Cancellable Walk Summary

**`scan::walk_observed` reports the file count while directories are listed, and the walk now polls cancel per root, per listed entry and per file in the rayon stat phase, so a cancel no longer waits for every file to be statted.**

## Performance

- **Duration:** about 4 min
- **Started:** 2026-10-02T20:26:47Z
- **Completed:** 2026-10-02T20:30:45Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- The walk stops promptly when cancelled. It checks the flag before each root, for each entry it lists, for each file in the parallel stat phase, and once more after that phase returns. A cancel raised mid-stat now returns `ScanError::Cancelled`. Before, the walk returned `Ok` after statting all 200 files (D-05, T-01-11 mitigated).
- New crate-private `walk_observed(opts, visit, on_error, on_progress)`. It passes a `Stats` snapshot to `on_progress` each time a regular file is counted during listing. The public `scan::walk` keeps its exact signature and delegates with a no-op hook.
- `pipeline::walk_stage` maps each snapshot to `Event::Progress { stage: Walk, done: files, total: None }`, so the CLI's `[1/4] walk  48 210 files` line now rises during directory listing (D-06). The interim per-candidate counter that 01-03 added has been replaced. There is one walk-progress source again.

## Task Commits

1. **Task 1 RED: failing tests for walk cancellation**: `04ef633` (test)
2. **Task 1 GREEN: stop the walk promptly when cancelled**: `9f6344e` (fix)
3. **Task 2 RED: failing test for walk progress during listing**: `b2e46d4` (test)
4. **Task 2 GREEN: report walk progress from core**: `da802ae` (feat)

Base: `09f59c99a38d14858273a9aaa8b166edeff1ece8`. The plan ledger was kept outside `.git/worktrees` because the hook refuses writes there. `commits: 4` was measured with `git rev-list --count 09f59c9..HEAD` before this SUMMARY commit.

## Files Created/Modified

- `crates/twins-core/src/scan/walk.rs`: `walk_observed` with a `P: Fn(&Stats) + Sync` hook, a private `cancelled(opts)` helper, and cancel polls per root, per entry, per statted file and after the stat phase.
- `crates/twins-core/src/scan/mod.rs`: `pub(crate) use walk::walk_observed;`. `pub use walk::{Stats, walk};` is unchanged.
- `crates/twins-core/src/pipeline.rs`: `walk_stage` uses `walk_observed` and emits Walk Progress from the snapshots. The interim `walked` counter is removed.
- `crates/twins-core/tests/scan_test.rs`: `walk_cancelled_before_start` and `walk_cancel_during_stat_phase_stops`.
- `crates/twins-core/tests/pipeline_test.rs`: `walk_progress_counts_files_before_size_grouping`.

## Decisions Made

- Walk progress counts files seen (`stats.files`), not candidates. A tree of small files below the minimum size still shows a rising count. The count stays frozen during the stat phase, which is the planned assumption CORE-01.
- `on_progress` runs only on the listing thread. The `done` values are therefore non-decreasing by construction. The old candidate counter fired from rayon workers and had no such guarantee.
- There is no throttling in core. The CLI wraps its observer in `Throttle` (01-03).

## TDD Gate Compliance

- Task 1: RED `04ef633` came before GREEN `9f6344e`. I converted the `cargo test` output to TAP and ran `check tdd-red-evidence` on it. The verdict was `RED_EVIDENCE_OK`: the target `walk_cancel_during_stat_phase_stops` failed with exit 101 on its assertion (`Ok(Stats { files: 200, candidates: 200, .. })` instead of `Err(Cancelled)`).
  - `walk_cancelled_before_start` already passed at RED, because the listing loop already polled cancel at the root entry. The plan expected only the stat-phase test to fail. It said so in the task: "Today this returns Ok, because the parallel stat phase never polls". The new test pins behaviour that no test covered before.
- Task 2: RED `b2e46d4` came before GREEN `da802ae`. The verdict was `RED_EVIDENCE_OK`: the target `walk_progress_counts_files_before_size_grouping` failed with exit 101 and "no walk progress", because the interim counter only fired for candidates and none of the five small files was one.
- No REFACTOR commits were needed.

## Deviations from Plan

None. The plan was executed as written.

Note: commit `9f6344e` builds with one `unused import: walk::walk_observed` warning. The plan puts the crate re-export in Task 1, but its only consumer, `pipeline.rs`, arrives in Task 2. Commit `da802ae` resolves it, and clippy with `-D warnings` is clean at HEAD.

## Issues Encountered

- The worktree isolation hook refused a compound shell command that used a heredoc. I made the test edits with the Edit tool and ran each git command on its own with `/usr/bin/git -C <worktree>`.

## Verification

- `cargo fmt --all --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: no issues
- `cargo test --workspace`: all suites pass (scan_test 10, pipeline_test 17, observe_test, CLI tests included)
- `cargo test -p twins-core --test scan_test cancel`: 2 passed, stable over 10 repeated runs

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness

- 01-07 (second Ctrl+C escape hatch) can rely on the walk honouring cancel in every phase. A directory read blocked on a stuck volume is still out of reach of a cooperative poll (flagged assumption CORE-02).
- The app and agent shells get walk progress through the same `Observer` events with no extra wiring.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED

- FOUND: crates/twins-core/src/scan/walk.rs (contains `pub(crate) fn walk_observed`)
- FOUND: crates/twins-core/tests/scan_test.rs (contains `fn walk_cancel_during_stat_phase_stops`)
- FOUND: crates/twins-core/tests/pipeline_test.rs (contains `fn walk_progress_counts_files_before_size_grouping`)
- FOUND commits: 04ef633, 9f6344e, b2e46d4, da802ae
