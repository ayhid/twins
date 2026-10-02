---
phase: 01-core-pipeline-refactor
plan: 03
subsystem: core
tags: [rust, observer, throttle, progress, tdd, gate-trip]

requires:
  - phase: 01-01
    provides: "twins_core::observe (CancelToken, Stage, Event, Observer) and pipeline::scan"
provides:
  - "RED tests only: crates/twins-core/tests/observe_test.rs (8 tests) and group_digits_inserts_spaces_every_three_digits in human_test.rs"
  - "Compile-only stubs: pass-through observe::Throttle<O> (new/inner/into_inner) and ungrouped human::group_digits"
affects: [01-03-continuation, 01-07]

actuals:
  tokens: 2199
  tasks: 0
  commits: 1
plan_head_before: 7b99dad547ce9bb4e1042dc1ccbcb8763aee82f7
plan_head_after: cd48f4905d3d686fa31b7249bebb1f71f7b63796

tech-stack:
  added: []
  patterns:
    - "Rust RED with compile-only stubs, so the RED run fails on assertions (RED_EVIDENCE_OK) instead of on unresolved imports (INVALID_RED)"
    - "libtest 'test <name> ... ok|FAILED' lines translated one for one into TAP for gsd_run check tdd-red-evidence"

key-files:
  created:
    - crates/twins-core/tests/observe_test.rs
  modified:
    - crates/twins-core/tests/human_test.rs
    - crates/twins-core/src/observe.rs
    - crates/twins-core/src/human.rs

key-decisions:
  - "Halted at the TDD runtime gate before Task 1's implementation step: check 1 (RED commit touching a test file) finds nothing because the documented pathspec does not match crates/*/tests/*_test.rs. This is the Rust gap the user accepted in advance; GSD files were not modified"
  - "The RED commit carries compile-only stubs (Throttle forwards everything, group_digits = to_string) so that RED fails on assertions; GREEN replaces both bodies"

patterns-established:
  - "Throttle tests use a 1-hour interval so they never depend on wall-clock timing"

requirements-completed: []

coverage:
  - id: D1
    description: "RED tests for Throttle, CancelToken sharing, Event JSON shape, the D-10 stage sequence and group_digits, committed and failing on assertions"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "cargo test --no-fail-fast -p twins-core --test observe_test --test human_test (exit 101: 8 passed, 4 failed as intended)"
        status: fail
    human_judgment: true
    rationale: "Plan halted at the TDD gate; the deliverables of 01-03 (Throttle, group_digits, TerminalObserver, run.rs wiring) are not implemented yet"

duration: 3min
completed: 2026-10-02
status: halted
---

# Phase 1 Plan 03: Staged Terminal Progress Summary (HALTED at TDD gate)

**Only the RED step of Task 1 landed. Eight observer tests and one digit-grouping test are committed, with compile-only stubs, and they fail on assertions (`RED_EVIDENCE_OK`). Execution then halted at the TDD runtime gate: the gate's test-file pathspec cannot see Rust integration tests under `crates/*/tests/`.**

## Performance

- **Duration:** about 3 min
- **Started:** 2026-10-02T14:35:47Z
- **Completed:** 2026-10-02T14:38:03Z
- **Tasks:** 0 of 2 complete (Task 1 RED only)
- **Files modified:** 4 (1 created, 3 modified)

## TDD GATE TRIPPED: Plan 01-03, Task 1

Reason: `missing_red_commit` (check 1). The RED commit exists, but the gate's pathspec does not see it.

Behavior expected to be tested:
- group_digits_inserts_spaces_every_three_digits: 0 → "0", 999 → "999", 1000 → "1 000", 48210 → "48 210", 1234567 → "1 234 567", u64::MAX → "18 446 744 073 709 551 615" (plain ASCII space).

Gate checks as run:

| Check | Result | Evidence |
|-------|--------|----------|
| 1. RED commit touches a test file | **TRIPPED** | `git log -E --grep='^[a-z]+\((0*1)-(0*3)\):' -- "*.test.*" "*.spec.*" "tests/" "__tests__/" "*_test.go" "test_*.py" "*_test.py" "*_test.exs" "*_spec.rb" "*_test.rb"` prints nothing. Without the pathspec the same grep prints `cd48f49 test(01-03): ...`, which touches `crates/twins-core/tests/observe_test.rs` and `crates/twins-core/tests/human_test.rs`. The pathspec `tests/` only matches a root-level `tests/`, and `*_test.rs` is not in the list. This is the Rust gap described in `references/tdd.md` (#4379). The same pathspec also finds no RED commit for plan 01-01. |
| 2. RED was intentional (#3770) | PASS | `gsd_run check tdd-red-evidence` → `RED_EVIDENCE_OK` / `target_test_failed`, target `throttle_limits_progress`, exit 101, 12 tests, 8 pass, 4 fail (`throttle_limits_progress`, `throttle_forwards_final_progress`, `throttle_reopens_on_stage_start`, `group_digits_inserts_spaces_every_three_digits`) |
| 3. No feat before test | PASS | The only `(01-03)` commit is `cd48f49 test(01-03)` |

`last_gate_trip: 01-03/1`. Not written to STATE.md, because in worktree mode the orchestrator owns that file. The orchestrator should record it.

Required next step (gate contract):
1. A failing test for the behavior above already exists and is committed: `cd48f49 test(01-03): add failing tests for Throttle and digit grouping (#1)`.
2. The gate cannot see it until someone either (a) widens the gate's pathspec to cover Rust (`*_test.rs`, or `tests/` at any depth), which is a GSD change the executor may not make, or (b) explicitly overrides the gate for this phase (the documented `--force-mvp-gate` escape hatch, or setting `workflow.tdd_mode` to false).
3. Then re-run `/gsd-execute-phase 01`. The continuation resumes at Task 1 GREEN.

## Accomplishments
- `crates/twins-core/tests/observe_test.rs` (new, 8 tests): `cancel_token_clones_share_the_flag`, `event_json_uses_camel_case_tags`, `throttle_forwards_non_progress_events`, `throttle_limits_progress`, `throttle_forwards_final_progress`, `throttle_reopens_on_stage_start`, `throttle_zero_interval_forwards_everything`, `throttled_observer_sees_the_same_stage_sequence` (D-10). It uses a `Recorder` observer and a 1-hour interval, so the tests do not depend on timing.
- `group_digits_inserts_spaces_every_three_digits` added to `crates/twins-core/tests/human_test.rs`.
- Compile-only stubs, so the tests build: `observe::Throttle<O>` with `new(inner, interval)`, `inner()`, `into_inner()`, and an `Observer` impl that forwards everything; `human::group_digits(n)` returns `n.to_string()`.

## RED Run (for the continuation)

`cargo test --no-fail-fast -p twins-core --test observe_test --test human_test` exits 101:
- Fail as intended (assertions): `throttle_limits_progress` (all 100 Progress forwarded instead of 1), `throttle_forwards_final_progress` (1..=10 instead of [1, 10]), `throttle_reopens_on_stage_start` (PartialHash 2 forwarded), `group_digits_inserts_spaces_every_three_digits` ("1000" vs "1 000").
- Pass already, which is expected: `cancel_token_clones_share_the_flag` and `event_json_uses_camel_case_tags` pin plan 01-01 behaviour. `throttle_forwards_non_progress_events`, `throttle_zero_interval_forwards_everything` and `throttled_observer_sees_the_same_stage_sequence` are preservation properties that a pass-through already has; they guard GREEN against dropping or reordering structural events.

## Task Commits

1. **Task 1 RED: failing tests + compile-only stubs**: `cd48f49` (test)
2. Task 1 GREEN: not started (halted at the gate)
3. Task 2 (TerminalObserver, run.rs wiring, CLI test): not started

**Plan metadata:** the SUMMARY commit (docs) that follows

## Files Created/Modified
- `crates/twins-core/tests/observe_test.rs`: observer contract tests (new)
- `crates/twins-core/tests/human_test.rs`: group_digits test; the module doc now mentions counts
- `crates/twins-core/src/observe.rs`: pass-through `Throttle<O>` stub (to be replaced in GREEN)
- `crates/twins-core/src/human.rs`: ungrouped `group_digits` stub (to be replaced in GREEN)

## Decisions Made
- Halted instead of implementing. The user said in advance that, if the gate trips on the Rust pathspec gap, the executor follows the gate contract and does not modify GSD files.
- Compile-only stubs went into the RED commit. A test-only commit would fail on `unresolved import`, which #3770 classifies as INVALID_RED. With the stubs, RED fails on the target assertion, and the commit reproduces the evidence exactly.

## Deviations from Plan

**1. [Rule 3 - Blocking] Compile-only stubs in the RED commit**
- **Found during:** Task 1 RED
- **Issue:** The plan says the tests may "fail to compile or fail". A compile failure is INVALID_RED under the TDD gate (#3770), so it would also have tripped check 2.
- **Fix:** Added a minimal pass-through `Throttle` and an ungrouped `group_digits` so the tests compile and fail on assertions.
- **Files modified:** crates/twins-core/src/observe.rs, crates/twins-core/src/human.rs
- **Verification:** `gsd_run check tdd-red-evidence` → `RED_EVIDENCE_OK`. Clippy (`-D warnings`) and `cargo fmt --check` are clean on the RED tree.
- **Committed in:** cd48f49

**Total deviations:** 1 (Rule 3). **Impact:** none on scope. GREEN replaces both stub bodies, as the plan's action specifies.

## Issues Encountered
- The TDD gate tripped (see above). This plan's work is blocked until the gate is widened or overridden.
- The worktree isolation hook refuses plain `git` (rtk rewrites it) and any compound command that names git. `/usr/bin/git -C <worktree>`, one command per call, works. The plan's start-of-execution ledger was kept in the session scratchpad, because writing under `.git/worktrees/...` was refused; the base `7b99dad` matches the dispatch base.
- `cargo test --workspace` is intentionally red on this tree (the 4 RED tests). `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

## Known Stubs

| File | Item | Reason | Resolved by |
|------|------|--------|-------------|
| crates/twins-core/src/observe.rs | `Throttle<O>`: forwards every event, ignores the interval | RED scaffolding | Plan 01-03 Task 1 GREEN (AtomicU64 + compare_exchange, as specified in the plan) |
| crates/twins-core/src/human.rs | `group_digits`: no grouping | RED scaffolding | Plan 01-03 Task 1 GREEN |

## User Setup Required

None. No external service configuration is required.

## Next Phase Readiness
- Blocked: plan 01-03 needs the TDD gate decision (widen the pathspec for Rust, or override) before GREEN. When resumed, Task 1 GREEN starts from `cd48f49`, then Task 2 (`progress.rs` TerminalObserver, `run.rs` wiring with `Throttle::new(TerminalObserver::stderr(..), Duration::from_millis(80))`, `json_stderr_is_silent_when_not_a_terminal`).
- The CLI still has no TTY progress line (interim state left by plan 01-01).
- The same gate will trip for every `tdd="true"` task in this phase (01-02 and later), because all Rust tests live in `crates/*/tests/` or in `#[cfg(test)]` modules.

## Self-Check: PASSED
- Files exist: crates/twins-core/tests/observe_test.rs, crates/twins-core/tests/human_test.rs, crates/twins-core/src/observe.rs, crates/twins-core/src/human.rs
- Commit exists: cd48f49
- RED evidence: `RED_EVIDENCE_OK` (target `throttle_limits_progress`)
- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` exit 0

---
*Phase: 01-core-pipeline-refactor*
*Halted: 2026-10-02*
