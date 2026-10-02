---
phase: 01-core-pipeline-refactor
plan: 02
subsystem: core
tags: [rust, rayon, progress, observer, cancellation, pipeline]

# Dependency graph
requires:
  - phase: 01-core-pipeline-refactor (plan 01-01)
    provides: observe::{Stage, Event, Observer, CancelToken}, pipeline::scan, ScanSpec::steps, Recorder/CancelOn test observers
provides:
  - group::find opens partial hash, full hash and verify with exactly one done == 0 marker, sent from the calling thread, even for empty stages
  - group::find reports done in order within a stage (1, 2, ..., total), so the stage's last event is (total, total)
  - pipeline::scan emits Event::StageStarted { stage, step, steps } for every hashing stage, in the order walk, size grouping, partial hash, full hash (, verify)
  - cancel at any hashing stage start returns PipelineError::Cancelled with Finished(Cancelled) last
affects: [01-03 Throttle (relies on StageStarted reopening and an in-order final Progress), CLI progress rendering, twins-app progress UI]

# Actuals (#2632)
actuals:
  tokens: 3263
  tasks: 2
  commits: 5

plan_head_before: 71bdc8ed9a25f049020649d23264eaa0ea50a0cf
plan_head_after: 4fe140e98a785b5bde99b64f5f603440f139f15e

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Stage-start marker: a progress callback with done == 0 means the stage started; real progress has done >= 1"
    - "Ordered progress from rayon workers: bump the counter and call the callback under one Mutex"

key-files:
  created: []
  modified:
    - crates/twins-core/src/group/find.rs
    - crates/twins-core/src/pipeline.rs
    - crates/twins-core/tests/group_test.rs
    - crates/twins-core/tests/pipeline_test.rs

key-decisions:
  - "Hashing progress is counted and reported under one Mutex (was an AtomicU64 plus an unordered callback), so observers see done rise by one per event. This fixes the flaky reports_progress_per_stage deferred from 01-04"
  - "The done == 0 marker is mapped to StageStarted inside the pipeline's on_progress closure. No new public type or flag; group::Progress keeps its shape"

patterns-established:
  - "Stage-start marker contract: documented on group::Progress and Options::on_progress"
  - "Stage-order contract: documented on pipeline::scan"

requirements-completed: [CORE-01, CORE-02]

coverage:
  - id: D1
    description: "group::find sends one done == 0 marker at the start of partial hash, full hash and verify, before any worker event of that stage, also on an empty index"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/group_test.rs#each_stage_starts_with_a_zero_done_marker"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/group_test.rs#empty_index_still_marks_hash_stages"
        status: pass
    human_judgment: false
  - id: D2
    description: "Hashing progress arrives in order within a stage (done 0, 1, ..., total), so the last event is (total, total)"
    requirement: CORE-01
    verification:
      - kind: unit
        ref: "crates/twins-core/tests/group_test.rs#progress_done_rises_by_one_within_a_stage"
        status: pass
      - kind: unit
        ref: "crates/twins-core/tests/group_test.rs#reports_progress_per_stage (25/25 runs)"
        status: pass
    human_judgment: false
  - id: D3
    description: "pipeline::scan emits StageStarted in order with an honest step counter: (Walk,1,4)..(FullHash,4,4), or (Walk,1,5)..(Verify,5,5) with verify, also on an empty tree; every Progress sits inside its stage"
    requirement: CORE-01
    verification:
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#stage_events_in_order_without_verify"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#stage_events_in_order_with_verify"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#stage_events_on_empty_tree"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#progress_events_follow_their_stage_start"
        status: pass
    human_judgment: false
  - id: D4
    description: "Raising the CancelToken when partial hash, full hash or verify starts returns PipelineError::Cancelled, emits no later StageStarted, and ends with Finished(Cancelled)"
    requirement: CORE-02
    verification:
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#cancel_at_partial_hash_stops_before_full_hash"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#cancel_at_full_hash_returns_cancelled"
        status: pass
      - kind: integration
        ref: "crates/twins-core/tests/pipeline_test.rs#cancel_at_verify_returns_cancelled"
        status: pass
    human_judgment: false

# Metrics
duration: ~15min (this session; RED for Task 1 was committed in an earlier session)
completed: 2026-10-02
status: complete
---

# Phase 1 Plan 02: Observable hashing stages Summary

**`group::find` opens each hashing stage with a `done == 0` marker and reports `done` in order. `pipeline::scan` turns each marker into `Event::StageStarted { stage, step, steps }`, so observers see walk, size grouping, partial hash, full hash and, with verify, verify (5/5) in order, and can cancel at any stage start.**

## Performance

- **Duration:** about 15 min in this session. Task 1 RED (`9e7d53b`) was committed in an earlier session that halted at the TDD gate.
- **Started:** 2026-10-02 (resumed at Task 1 GREEN; the timestamp 20:18:49Z was recorded after setup)
- **Completed:** 2026-10-02T20:22:50Z
- **Tasks:** 2 of 2
- **Files modified:** 4

## Accomplishments

- `compute_keys` calls `opts.progress(stage, 0, total)` on the calling thread before `par_iter`. `find_in_pool` calls `opts.progress(Stage::Verify, 0, total)` before the bucket loop when verify is on. Both fire even when the stage is empty.
- Hashing progress is now counted and reported under one mutex, so `done` rises by exactly one per event. This removes the flaky `reports_progress_per_stage`.
- The pipeline's `on_progress` closure maps `done == 0` to `Event::StageStarted` with `stage.step()` and `spec.steps()`. All other events stay `Event::Progress`.
- Contracts are documented on `group::Progress`, `Options::on_progress` and `pipeline::scan`.

## Task Commits

1. **Task 1: group::find announces each stage with a done == 0 marker**
   - RED `9e7d53b` test(01-02): add failing tests for hashing stage start markers (earlier session)
   - GREEN `b00ec94` feat(01-02): mark the start of every hashing stage
2. **Deviation (Rule 1): in-order hashing progress**
   - RED `f0c616e` test(01-02): add failing test for in-order progress within a stage
   - FIX `3c048b7` fix(01-02): report hashing progress in order
3. **Task 2: pipeline turns markers into StageStarted; cancel at every hashing stage**
   - RED `d825275` test(01-02): add failing tests for stage order and cancel at hashing stages
   - GREEN `4fe140e` feat(01-02): report every pipeline stage in order with its step

No REFACTOR commits were needed.

## TDD Gate Compliance

| Gate | Task 1 | Ordering fix | Task 2 |
|------|--------|--------------|--------|
| RED commit | `9e7d53b` | `f0c616e` | `d825275` |
| RED evidence (`check tdd-red-evidence`) | RED_EVIDENCE_OK, target `each_stage_starts_with_a_zero_done_marker` | RED_EVIDENCE_OK, target `progress_done_rises_by_one_within_a_stage` | RED_EVIDENCE_OK, target `stage_events_in_order_with_verify` |
| GREEN commit | `b00ec94` | `3c048b7` (fix) | `4fe140e` |

The checker reads TAP or Surefire output only. Each record was built by a script that ran the real `cargo test` command and converted every `test NAME ... ok|FAILED` line to one TAP line, keeping the raw cargo output in the record. The records are in the session scratchpad, not the repo. RED failures were on the intended assertions:
- Task 1: the first Partial event had `done == 1`, and an empty index emitted no events.
- Ordering: `done` arrived as `..., 20, 22, ..., 30, 21, ...`.
- Task 2: the hashing StageStarted events were missing, cancel never fired (`unwrap_err` on `Ok`), and hashing Progress arrived outside its stage.

## Files Created/Modified

- `crates/twins-core/src/group/find.rs`: stage-start markers, in-order progress under a mutex, doc contract
- `crates/twins-core/src/pipeline.rs`: marker → `Event::StageStarted` mapping, stage-order doc on `scan`
- `crates/twins-core/tests/group_test.rs`: `progress_done_rises_by_one_within_a_stage` added (Task 1 tests were already in `9e7d53b`)
- `crates/twins-core/tests/pipeline_test.rs`: 7 tests plus the `pair_tree`, `stages` and `assert_cancelled_at` helpers

## Decisions Made

- Fixed the progress race at its source rather than relaxing `reports_progress_per_stage` to assert on the maximum `done`. Out-of-order `done` is a real defect for any progress bar, and for plan 01-03's Throttle, which must forward the stage's final Progress. The mutex costs one uncontended-in-practice lock per hashed file, which is negligible next to the hashing.
- No change to `group::Progress`'s shape and no new public API. The marker is the `done == 0` convention the plan specified.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Hashing progress reached observers out of order**
- **Found during:** Task 1 (logged in `deferred-items.md` by 01-04 as the flaky `reports_progress_per_stage`)
- **Issue:** `compute_keys` did `done.fetch_add(1) + 1` and then called `on_progress` with no ordering between workers. Observers could see `done` go backwards, and the stage's last event could carry a stale `done`. The repro test failed 8 of 8 runs, e.g. `21` arriving after `30`.
- **Fix:** replaced the `AtomicU64` with a `Mutex<u64>`. The counter bump and the callback happen under the lock, and poisoning is tolerated via `PoisonError::into_inner`. The `Progress` doc now states the in-order guarantee.
- **Files modified:** `crates/twins-core/src/group/find.rs`, `crates/twins-core/tests/group_test.rs`
- **Verification:** `progress_done_rises_by_one_within_a_stage` passes. `group_test` and `pipeline_test` passed 25 of 25 back-to-back runs.
- **Committed in:** `f0c616e` (RED), `3c048b7` (fix)

---

**Total deviations:** 1 auto-fixed (Rule 1 bug).
**Impact on plan:** Needed so the stage-final Progress is deterministic, which plan 01-03's Throttle relies on. No scope creep: same file, same code path the plan reworks.

## Issues Encountered

- `cargo test --workspace` does not exit 0 yet. The only failures are plan 01-03's committed RED tests (`cd48f49`): `human_test::group_digits_inserts_spaces_every_three_digits` and `observe_test::throttle_limits_progress` / `throttle_forwards_final_progress` / `throttle_reopens_on_stage_start`. They fail identically at the base commit `71bdc8e` and will pass once 01-03 GREEN lands. All other test binaries pass, and `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
- `deferred-items.md` still lists the flaky `reports_progress_per_stage` as open. I left it untouched to avoid a merge conflict with sibling worktrees. The orchestrator can mark it resolved by `3c048b7`.

## User Setup Required

None. No external service configuration required.

## Next Phase Readiness

- Plan 01-03 (Throttle, CLI progress) can rely on: StageStarted for every stage, in order and never sent from a worker before its stage's marker; Progress with `done` rising by one; and a final `(total, total)` for each completed hashing stage.
- No blockers.

---
*Phase: 01-core-pipeline-refactor*
*Completed: 2026-10-02*

## Self-Check: PASSED

- Files exist: find.rs, pipeline.rs, group_test.rs, pipeline_test.rs
- Commits exist: 9e7d53b, b00ec94, f0c616e, 3c048b7, d825275, 4fe140e
- Acceptance criteria re-run: all pass except `cargo test --workspace` exit 0, which is blocked only by 01-03's RED tests (see Issues Encountered)
