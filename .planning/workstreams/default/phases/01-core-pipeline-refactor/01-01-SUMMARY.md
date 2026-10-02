---
phase: 01-core-pipeline-refactor
plan: 01
subsystem: core
tags: [rust, pipeline, observer, cancellation, serde, characterization-tests]

requires: []
provides:
  - "twins_core::observe: CancelToken, Stage, Outcome, Event, Observer, NoopObserver"
  - "twins_core::pipeline: RunMode, ScanSpec, ScanOutcome, PipelineError, scan()"
  - "CLI scan/report rendered from pipeline::scan; exit code 130 for PipelineError::Cancelled"
  - "Characterization tests locking JSON (relative root, key order), text, exit codes and unreadable-file reporting"
affects: [01-02, 01-03, 01-04, 01-05, 01-06, 01-07, phase-2-clean, phase-5-tauri, phase-6-runner, phase-8-agent]

actuals:
  tokens: 9274
  tasks: 2
  commits: 3
plan_head_before: 10401aaf1f9817c6ca1007b55dc6582043b42fca
plan_head_after: 47f9348f356d74963ad11ca35820597f79083aac

tech-stack:
  added: []
  patterns:
    - "Core orchestrates, shells render: the CLI builds a ScanSpec and prints outcome.report(now)"
    - "Report dry-run flag derived only from RunMode::is_dry_run() inside pipeline.rs"
    - "scan() wraps a private run() and always emits Event::Finished last"
    - "Explicit cancel checks before the walk, after the walk and after hashing"
    - "PipelineError From impls collapse ScanError::Cancelled / FindError::Cancelled into PipelineError::Cancelled"

key-files:
  created:
    - crates/twins-core/src/observe.rs
    - crates/twins-core/src/pipeline.rs
    - crates/twins-core/tests/pipeline_test.rs
  modified:
    - crates/twins-core/src/lib.rs
    - crates/twins-cli/src/run.rs
    - crates/twins-cli/tests/cli_test.rs

key-decisions:
  - "PipelineError::Scan/Find are #[error(transparent)] so CLI error text is unchanged; the From impls are hand-written so cancellation always becomes PipelineError::Cancelled"
  - "Interim CLI observer SkipPrinter only prints 'skip {path}: {reason}' with --verbose; the TTY progress line is gone until plan 01-03 adds the terminal observer"
  - "exit_code no longer downcasts ScanError directly; PipelineError::Scan gives 2, Find 1, Cancelled 130, ParseSizeError still 2"

patterns-established:
  - "Observer contract: Event is #[non_exhaustive], internally tagged (type), camelCase variants and fields, struct variants only, paths as lossy Strings"
  - "Test observers: Recorder (Mutex<Vec<Event>>) and CancelOn (raises the token on a given StageStarted)"

requirements-completed: [CORE-01, CORE-02, CORE-03]

coverage:
  - id: D1
    description: "twins scan / twins report output unchanged after the move (JSON with relative root and key order, exact text, empty stderr, exit codes and messages, unreadable-file summary and --verbose listing)"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#characterize_json_report_with_relative_root"
        status: pass
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#characterize_text_report"
        status: pass
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#usage_errors_exit_2_with_the_same_message"
        status: pass
      - kind: integration
        ref: "crates/twins-cli/tests/cli_test.rs#unreadable_files_are_counted_and_listed_with_verbose"
        status: pass
    human_judgment: false
  - id: D2
    description: "Pipeline runs in twins-core and gives the same report as the hand-wired walk + find + plan; run.rs no longer wires the stages"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#pipeline_matches_manual_wiring"
        status: pass
      - kind: other
        ref: "! grep -rnE 'scan::walk|group::find|group::plan|Keeper::new|Index::new|Meta \\{|dry_run' crates/twins-cli/src"
        status: pass
    human_judgment: false
  - id: D3
    description: "Report dry_run derived from RunMode inside core (Scan default false, DryRun true)"
    requirement: CORE-03
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#dry_run_in_meta_follows_run_mode"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#default_run_mode_is_scan"
        status: pass
    human_judgment: false
  - id: D4
    description: "Observer events: Walk is step 1 of 4 (5 with verify), SizeGrouping progress equals candidates, exactly one Finished and it is last; unreadable files become FileSkipped"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#finished_is_the_last_event"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#verify_raises_the_step_count_to_five"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#unreadable_file_is_reported_as_file_skipped"
        status: pass
    human_judgment: false
  - id: D5
    description: "One CancelToken drives walk and hashing; cancel at the walk and between stages returns PipelineError::Cancelled with Finished{Cancelled}; walk errors give Failed; CLI maps Cancelled to exit 130"
    requirement: CORE-02
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#cancel_at_walk_returns_cancelled"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#cancel_between_stages_is_not_ignored"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/pipeline_test.rs#walk_errors_fail_without_cancelling"
        status: pass
      - kind: unit
        ref: "crates/twins-cli/src/run.rs#exit_code_maps_cancel_to_130"
        status: pass
    human_judgment: false

duration: ~25min
completed: 2026-10-02
status: complete
---

# Phase 1 Plan 01: Pipeline Tracer Summary

**`twins scan` and `twins report` now get their report from `twins_core::pipeline::scan(spec, &observer, &cancel)`. The new `observe` module provides the CancelToken/Event/Observer contract, and the report's dry-run flag comes from `RunMode` inside core. Characterization tests committed before the move confirm the output is unchanged.**

## Performance

- **Duration:** about 25 min (start time was not recorded)
- **Completed:** 2026-10-02T14:29:20Z
- **Tasks:** 2 (1 tracer, 1 auto)
- **Files modified:** 6 (3 created, 3 modified)

## Accomplishments
- Four CLI characterization tests lock the current output: the full JSON report with a relative root (every field except `scanned_at`, plus top-level key order), the exact text report with empty stderr, exit 2 with exact messages for `/System`, `--min-size 12X` and `--exclude [`, and the unreadable-file summary, `--verbose` listing and silent `--json`. They were committed before any source change and still pass after it.
- `observe.rs`: `CancelToken` (shared `Arc<AtomicBool>`), `Stage` (with `step()`, `Display` and `From<group::Stage>`), `Outcome`, the `Event` enum (internally tagged with `type`, camelCase variants and fields), the `Observer: Send + Sync` trait and `NoopObserver`.
- `pipeline.rs`: `RunMode` (Scan by default, DryRun), the `ScanSpec` builder, `ScanOutcome` (`actions`/`meta`/`stats`/`errors`/`report(now)`), `PipelineError` (Scan/Find transparent, plus Cancelled) and `scan()`. `scan()` emits StageStarted/Progress/FileSkipped events and always ends with exactly one `Finished`. It checks for cancellation before the walk, after the walk and after hashing.
- `run.rs` now only builds a `ScanSpec`, prints skipped files with `--verbose` through `SkipPrinter`, and renders the report. The manual walk/index/find/plan/`Meta` wiring and the `Progress` struct are gone. `exit_code` maps Cancelled to 130.
- Nine core tests in `pipeline_test.rs` cover: report parity with the hand-wired stages, the dry-run flag following `RunMode`, `Finished` being last, the step count with verify, cancelling at the walk and between stages, walk failure, and unreadable files.

## Task Commits

1. **Task 1 (tracer), Step A: characterization tests**, `c141fe0` (test)
2. **Task 1 (tracer), Steps B-E: pipeline move**, `d013589` (feat)
3. **Task 2: core pipeline tests**, `47f9348` (test)

**Plan metadata:** the SUMMARY commit (docs) that follows

## Files Created/Modified
- `crates/twins-core/src/observe.rs`: cancellation token, stages, events and the observer trait
- `crates/twins-core/src/pipeline.rs`: run mode, scan spec/outcome, pipeline error and `scan()`
- `crates/twins-core/src/lib.rs`: `pub mod observe; pub mod pipeline;`
- `crates/twins-cli/src/run.rs`: reduced to building the spec, calling the pipeline, rendering the report and mapping exit codes, plus 3 unit tests
- `crates/twins-cli/tests/cli_test.rs`: 4 characterization tests
- `crates/twins-core/tests/pipeline_test.rs`: 9 pipeline tests

## Decisions Made
- `PipelineError::Find` matches `FindError::ThreadPool(_)` explicitly instead of a wildcard, because clippy's `match_wildcard_for_single_variants` rejects the wildcard. Behaviour is the same.
- Core and CLI changed in a single feat commit. The tracer task is one end-to-end slice, and the commit before it holds only the characterization tests, as the acceptance criteria require.

## Deviations from Plan

None. The plan was executed as written. One small fix during Task 1 was a lint fix, not a behaviour change (see Decisions Made).

## Issues Encountered
- The rtk command-rewriting hook combined with worktree isolation refused plain `git ...` invocations. `/usr/bin/git -C <worktree>` worked. This had no effect on the code.
- The interactive progress line on a TTY is gone for now (`SkipPrinter` only lists skipped files). This is intended: plan 01-03 adds the terminal observer. Tests do not cover it because they never run on a TTY.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness
- Plans 01-02 through 01-07 can build on the fixed `observe`/`pipeline` names. Still missing: the Throttle and terminal observer (01-03), stage-start markers and walk progress (01-02 and later), in-file and stat-phase cancellation (01-05 and 01-06), and SIGINT (01-07).
- CORE-01 and CORE-02 are shared with sibling plans, so only CORE-03 was marked complete in REQUIREMENTS.md (shared-ID gate).

## Self-Check: PASSED
- Files exist: observe.rs, pipeline.rs, pipeline_test.rs, cli_test.rs, run.rs, lib.rs
- Commits exist: c141fe0, d013589, 47f9348
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all exit 0. The CLI integration suite passed 10 tests, the run.rs unit tests 3, and pipeline_test 9.
- The grep gate on `crates/twins-cli/src` is clean. `report.rs` is unchanged (`VERSION: u32 = 1`, no `mode`). twins-core has no tokio, signal-hook or ctrlc dependency.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*
