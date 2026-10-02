# Deferred Items: Phase 01

Out-of-scope issues found during execution. Logged here, not fixed by the plan that found them.

## From 01-04

- **Flaky `reports_progress_per_stage` (crates/twins-core/tests/group_test.rs:216).** Found while running `cargo test --workspace` on stable after the MSRV bump. It fails intermittently: 3 of 10 isolated runs on the 01-01 base code, with `left: Some((1, 2))`, `right: Some((2, 2))`. Likely cause: worker threads in `group::find` advance the per-stage `done` counter and then call `on_progress` without holding a lock. The callbacks can therefore reach the observer out of order, so the last `Partial` event pushed can carry a stale `done`. The test asserts that the last event has `done == total`. This is unrelated to the rust-version change. The files belong to plan 01-02 (`find.rs`, `group_test.rs`), which reworks this path. Make the stage-final event deterministic, for example by emitting one final `(total, total)` after the parallel loop, or assert on the maximum `done` instead of the last one.

  **Resolved in 01-02 (`3c048b7`):** the counter bump and the `on_progress` call now happen under one mutex, so `done` rises by one per event within a stage and the last event is `(total, total)`. Covered by `progress_done_rises_by_one_within_a_stage`.
