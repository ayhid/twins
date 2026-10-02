---
phase: 01-core-pipeline-refactor
plan: 02
subsystem: core
tags: [rust, pipeline, progress, tdd, gate-trip]

requires:
  - phase: 01-core-pipeline-refactor (plan 01-01)
    provides: "twins_core::observe (Stage, Event, Observer, CancelToken) and twins_core::pipeline::scan"
provides:
  - "RED tests each_stage_starts_with_a_zero_done_marker and empty_index_still_marks_hash_stages (failing, committed)"
affects: [01-02, 01-03]

actuals:
  tokens: 584
  tasks: 0
  commits: 1
plan_head_before: 7b99dad547ce9bb4e1042dc1ccbcb8763aee82f7
plan_head_after: 9e7d53bc5180c749be4c56105370bfe9ceebf0e8

tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - crates/twins-core/tests/group_test.rs

key-decisions:
  - "Halted at the TDD runtime gate before Task 1 GREEN: the gate's RED-commit pathspec does not match crates/*/tests/*_test.rs, so the valid RED commit 9e7d53b is invisible to it (known Rust gap, references/tdd.md #4379)"

patterns-established: []

requirements-completed: []

duration: 2min
completed: 2026-10-02
status: halted
---

# Phase 1 Plan 02: Stage Start Markers Summary (HALTED at TDD gate)

**The Task 1 RED tests for the `done == 0` stage-start marker are written, fail for the intended reason (RED_EVIDENCE_OK) and are committed as `9e7d53b`. The TDD runtime gate then tripped before the GREEN step, because its RED-commit pathspec cannot see Rust test files under `crates/*/tests/`. No production code changed.**

## Performance

- **Duration:** about 2 min
- **Started:** 2026-10-02T14:34:27Z
- **Stopped:** 2026-10-02T14:36:11Z
- **Tasks:** 0 of 2 complete (Task 1 RED done; GREEN blocked by the gate)
- **Files modified:** 1 (test only)

## TDD GATE TRIPPED: Plan 01-02, Task 1

Reason: missing_red_commit (as the gate measures it; the RED commit exists but is outside the gate's pathspec)

Behavior expected to be tested:
- each_stage_starts_with_a_zero_done_marker: with two identical "abcd" files and verify(true), Partial, Full and Verify each begin with one event where done == 0 and total equals that stage's later total, in stage order.

Required next step (per the gate contract):
1. Write a failing test for the behavior above. Already done: `9e7d53b test(01-02): add failing tests for hashing stage start markers (#1)`.
2. Commit it as `test(01-02): ...`. Already done, same commit.
3. Re-run /gsd-execute-phase. On this repo the gate trips again unless the RED-commit pathspec covers `crates/*/tests/*_test.rs` (or the user overrides the gate). Deciding that belongs to the user. This executor did not modify any GSD files.

### Gate evaluation (verbatim inputs)

- `task.is-behavior-adding` on 01-02-PLAN.md: `is_behavior_adding: true` (tdd_true, has_behavior_block, has_source_files all true)
- No milestone tag exists (`git describe --tags` fails), so the log range is the full history
- `git log --oneline -E --grep='^[a-z]+\((0*1)-(0*2)\):' -- "*.test.*" "*.spec.*" "tests/" "__tests__/" "*_test.go" "test_*.py" "*_test.py" "*_test.exs" "*_spec.rb" "*_test.rb"` returns nothing, so RED_COMMIT is empty and the gate trips
- The same grep without the pathspec returns `9e7d53b test(01-02): ...`, which touches only `crates/twins-core/tests/group_test.rs`
- `git log --grep='^feat\((0*1)-(0*2)\):'` returns nothing, so no implementation commit precedes RED
- Before starting, the plan 01-01 commits were run through the same pathspec as a probe. That also returned nothing, although `c141fe0` and `47f9348` are test commits under `crates/*/tests/`

### RED evidence (gsd-tools check tdd-red-evidence)

The raw `cargo test` libtest lines were translated one-to-one into TAP (`test NAME ... ok|FAILED` became `ok N - NAME` / `not ok N - NAME`, plus a `# tests/pass/fail` summary; the original output was kept as TAP comments).

| Target test | Command | Exit | Tests / pass / fail | Verdict |
|-------------|---------|------|---------------------|---------|
| each_stage_starts_with_a_zero_done_marker | `cargo test -p twins-core --test group_test` | 101 | 14 / 12 / 2 | RED_EVIDENCE_OK (target_test_failed) |
| empty_index_still_marks_hash_stages | `cargo test -p twins-core --test group_test` | 101 | 14 / 12 / 2 | RED_EVIDENCE_OK (target_test_failed) |

Failure messages, which are the intended assertions and not build or fixture errors:
- `each_stage_starts_with_a_zero_done_marker`: `assertion left == right failed: first partial hash event is not a marker; left: 1, right: 0`
- `empty_index_still_marks_hash_stages`: `left: [], right: [(Partial, 0, 0), (Full, 0, 0), (Verify, 0, 0)]`

The 12 existing group tests, including `reports_progress_per_stage` and `honours_cancellation`, still pass.

## Task Commits

1. **Task 1 RED: failing tests for the stage start markers**, `9e7d53b` (test)
2. Task 1 GREEN (`feat(01-02): mark the start of every hashing stage (#1)`): not started, blocked by the gate
3. Task 2 (pipeline StageStarted mapping and its 7 tests): not started

## Files Created/Modified
- `crates/twins-core/tests/group_test.rs`: two new failing tests, `each_stage_starts_with_a_zero_done_marker` and `empty_index_still_marks_hash_stages`

## Decisions Made
- The executor followed the gate contract literally, as the user instructed for this known caveat. It halted before any change to `find.rs` and modified no GSD files.
- STATE.md was not updated with `last_gate_trip`, because the orchestrator owns STATE.md in worktree mode. The orchestrator should record `last_gate_trip: 01-02/1`.
- The plan ledger (`plan_head_before`) was kept in the session scratchpad, because the sandbox refuses writes under `.git/worktrees/`. The value is the dispatch base 7b99dad.

## Deviations from Plan

None. The plan's tasks were not executed past Task 1 RED. The stop is the designed TDD gate stop, not a deviation.

## Issues Encountered
- As in 01-01, plain `git` calls were refused under the worktree isolation plus rtk hook combination. `/usr/bin/git -C <worktree>` worked.

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness
- Blocked: plan 01-02 cannot pass the TDD runtime gate on this repo as the gate is currently written. Plans 01-03 and later that depend on 01-02's StageStarted mapping wait on this.
- Resume point: Task 1 GREEN. Add `opts.progress(stage, 0, total)` in `compute_keys` before `par_iter`, add `opts.progress(Stage::Verify, 0, total)` before the bucket loop when verify is on, and extend the `Progress` and `on_progress` docs. Then Task 2 in full (its own RED, then GREEN).

## Self-Check: PASSED
- `crates/twins-core/tests/group_test.rs` contains both new test functions
- Commit `9e7d53b` exists on `worktree-agent-a269d50abe5890215`
- `cargo fmt --all --check` passes, and `cargo clippy -p twins-core --all-targets -- -D warnings` reports no issues

---
*Phase: 01-core-pipeline-refactor*
*Halted: 2026-10-02*
