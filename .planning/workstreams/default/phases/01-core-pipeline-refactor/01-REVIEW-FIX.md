---
phase: 01-core-pipeline-refactor
fixed_at: 2026-10-02T21:30:00Z
review_path: .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-10-02T21:30:00Z
**Source review:** .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4 (WR-01..WR-04; fix scope critical_warning, so the 8 Info findings are out of scope)
- Fixed: 4
- Skipped: 0

**Verification:** every gate ran in the main checkout (`/Users/ayoub/projects/twins`, branch `main`, no worktree), after each fix:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all passed.
Each new regression test was also run against the pre-fix source and failed there.
The ignored SIGINT tests (`cargo test -p twins-cli --test cli_test -- --ignored sigint`) pass on the final tree.

## Fixed Issues

### WR-01: `ScanSpec::keep_dir` is never absolutized, so `Strategy::InDir` silently keeps the wrong copy

**Files modified:** `crates/twins-core/src/pipeline.rs`, `crates/twins-cli/src/run.rs`, `crates/twins-core/tests/pipeline_test.rs`
**Commit:** fc41ba7
**Status:** fixed: requires human verification (plan logic)
**Applied fix:**
- `run()` builds the `Keeper` before the walk, in a new `keeper(spec)` function. For `Strategy::InDir`, the keep dir goes through `safety::absolutize`, the same lexical resolution as the roots.
- `InDir` with no keep dir now fails with the new `PipelineError::MissingKeepDir` before any walk event. The CLI maps it to exit 2 (usage).
- If a relative keep dir cannot be resolved because the current directory is unavailable, the run fails with the new `PipelineError::KeepDir(path)`, which maps to exit 1.
- Other strategies ignore the keep dir.
- `plan()` now takes the prebuilt keeper. The docs of `ScanSpec::keep_dir` and `scan`'s `# Errors` are updated.
- New tests:
  - `in_dir_keeps_the_copy_under_an_unnormalised_keep_dir` covers a relative keep dir (built from the cwd, with no `set_current_dir`) and an absolute one with `..`/`.`. Both keep `keep/b.bin` over the older `a.bin`.
  - `in_dir_without_keep_dir_fails_before_the_walk`.
  - The CLI unit test `exit_code_maps_missing_keep_dir_to_2`.

### WR-02: The observer runs while every hashing worker waits on the same mutex

**Files modified:** `crates/twins-core/src/group/find.rs`, `crates/twins-core/src/observe.rs`, `crates/twins-core/tests/group_test.rs`
**Commit:** 3ae882f
**Status:** fixed: requires human verification (lock-free concurrency protocol)
**Applied fix:**
- The `Mutex<u64>` held across `opts.progress` is gone. In its place is a private `Ticker` that hands one worker a reporting slot:
  - Each worker does a `SeqCst` `fetch_add` on `done`, then tries a CAS on a `busy` flag.
  - The worker that holds the slot reports every pending count in order. It then releases the slot and checks `done` again, so a count bumped during the release is not lost.
  - A worker that finds the slot taken goes straight back to hashing.
- Only the reporting worker waits for the observer. Progress keeps its exact guarantees: one call per count, `done` rising by one, no concurrent calls, and a final `(total, total)` call before the stage returns.
- The docs of `Observer`, `Options::on_progress` and `Progress` now describe this contract:
  - only `FileSkipped` can arrive concurrently;
  - a slow observer delays what it displays, not the hashing;
  - a stage still waits for its last event, and an observer that never returns stalls the run.
- New test `a_blocked_progress_callback_does_not_stall_the_other_workers`:
  - A callback blocks at the first partial-hash progress until 32 of the 64 files are fingerprinted, measured with a counting `Hasher`. It times out after 10 s.
  - The test then asserts that the held-back counts are still delivered as `0..=64`.
  - On the old code only 4 files (one per worker) were hashed while it blocked.
- `progress_done_rises_by_one_within_a_stage` is unchanged in what it asserts; only its fixture moved into a shared `same_size_pairs()` helper. Both tests were run 40 times with no flakes.

### WR-03: `Throttle`'s concurrency guarantees are only true for serialized callers

**Files modified:** `crates/twins-core/src/observe.rs`, `crates/twins-core/tests/observe_test.rs`
**Commit:** 94462b9
**Status:** fixed: requires human verification (concurrency semantics)
**Applied fix:**
- The `AtomicU64` CAS gate is replaced by a `Mutex<Gate>` that holds the last forwarded instant and the `(stage, done)` of the newest forwarded `Progress`. `Progress` and `StageStarted` are admitted and forwarded under that lock. Concurrent callers are therefore serialized and can no longer lose an admitted event to a failed CAS.
- `Throttle` also drops any `Progress` whose `done` is not above the last one forwarded for the same stage. A stale `done == 5` can therefore never follow the final event, even when concurrent callers arrive out of order. `StageStarted` resets the gate.
- `FileSkipped` and `Finished` bypass the lock and are never dropped.
- The docs no longer claim it is lock-free. They state the lock, that a slow inner observer makes other threads' `Progress` wait (uncontended in the pipeline, which delivers serially), and that a zero interval "forwards every `Progress` that moves its stage forward".
- The public API is unchanged: `new`, `inner`, `into_inner`.
- New tests: `throttle_drops_progress_that_would_go_back` (deterministic) and `throttle_keeps_concurrent_progress_in_order`. The second uses 8 threads and 2000 events, with both a zero and a one-hour interval, and asserts strictly increasing `done` with the final event last. Both failed on the old code and passed 20 stress runs on the new one.

### WR-04: Fatal glob errors print the parser message twice

**Files modified:** `crates/twins-cli/src/run.rs`, `crates/twins-cli/tests/cli_test.rs`
**Commit:** d336d52
**Status:** fixed
**Applied fix:**
- I used the review's alternative fix: `describe()` now skips any cause whose text the accumulated message already ends with.
- I did this instead of removing `{source}` from `ScanError::Glob`'s Display because:
  - `FsError::Io` (reachable as the fatal `ScanError::Io` on a root) and `FindError::ThreadPool` have the same "Display includes source plus `#[source]`" shape. They would also print twice.
  - The codebase convention, also followed by `HashError`, keeps the source in Display for shells that print only `to_string()`, such as `FileSkipped` reasons.
- `usage_errors_exit_2_with_the_same_message` now asserts the exact stderr: `twins: invalid exclude pattern "[": error parsing glob '[': unclosed character class; missing ']'`.
- New unit tests: `describe_prints_a_source_shown_by_its_parent_once` (the `FsError::Io` chain) and `describe_joins_context_and_cause` (normal anyhow context chains are unchanged).

---

_Fixed: 2026-10-02T21:30:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
