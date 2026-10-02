---
phase: 01-core-pipeline-refactor
reviewed: 2026-10-02T20:47:00Z
depth: standard
files_reviewed: 23
files_reviewed_list:
  - .github/workflows/ci.yml
  - Cargo.toml
  - crates/twins-cli/Cargo.toml
  - crates/twins-cli/src/main.rs
  - crates/twins-cli/src/progress.rs
  - crates/twins-cli/src/run.rs
  - crates/twins-cli/tests/cli_test.rs
  - crates/twins-core/src/group/find.rs
  - crates/twins-core/src/group/hasher.rs
  - crates/twins-core/src/hash.rs
  - crates/twins-core/src/human.rs
  - crates/twins-core/src/lib.rs
  - crates/twins-core/src/observe.rs
  - crates/twins-core/src/pipeline.rs
  - crates/twins-core/src/report.rs
  - crates/twins-core/src/scan/mod.rs
  - crates/twins-core/src/scan/walk.rs
  - crates/twins-core/tests/group_test.rs
  - crates/twins-core/tests/hash_test.rs
  - crates/twins-core/tests/human_test.rs
  - crates/twins-core/tests/observe_test.rs
  - crates/twins-core/tests/pipeline_test.rs
  - crates/twins-core/tests/scan_test.rs
findings:
  critical: 0
  warning: 4
  info: 8
  total: 12
status: issues_found
---

# Phase 01: Code Review Report

**Reviewed:** 2026-10-02T20:47:00Z
**Depth:** standard
**Files Reviewed:** 23
**Status:** issues_found

## Summary

The review covered the move of scan orchestration into `twins_core::pipeline::scan`, the observer/event model (`observe.rs`, including `Throttle`), cancellation in every stage (walk listing and stat, partial hash, cancellable full hash and byte compare), and the CLI shell (`TerminalObserver`, SIGINT through signal-hook, exit code 130).

To check correctness I ran the tests and traced the call paths. `cargo clippy --workspace --all-targets -- -D warnings` is clean. `cargo test --workspace` passes. Both ignored SIGINT tests pass locally (`-- --ignored sigint`). For data safety, the cancel paths hold up: every interruption path ends in `PipelineError::Cancelled`. That covers a cancelled `full_cancellable`, a key dropped by the post-check in `compute_keys`, a partial `verify` output, and a cancel that arrives between stages. None of these can produce an `Ok` outcome built from a prefix digest or a half-verified group. signal-hook-registry installs its handler with `SA_RESTART`, so the un-retried `f.read` in `full_cancellable` does not turn Ctrl+C into spurious read errors.

I found no blockers. The warnings are about API contracts that the next consumers (the app and phase 2 clean) will lean on:
- `ScanSpec::keep_dir` is not normalised, so `in-dir` silently falls back to the oldest copy.
- Observers run under a lock that every hashing worker shares.
- `Throttle` makes concurrency promises it only keeps because of that lock.
- Fatal glob errors print their message twice.

## Warnings

### WR-01: `ScanSpec::keep_dir` is never absolutized, so `Strategy::InDir` silently keeps the wrong copy

**File:** `crates/twins-core/src/pipeline.rs:85-90`, `crates/twins-core/src/pipeline.rs:334`
**Issue:** The walk absolutizes every root with `safety::absolutize` (`scan/walk.rs:270`), so every `FileMeta::path()` is absolute. `ScanSpec::keep_dir` stores the caller's path unchanged. `Keeper::new` only "cleans it lexically" (`keep.rs:73-79`), and `Keeper::in_dir` is a plain `path.starts_with(d)` (`keep.rs:125-127`). A relative keep dir such as `Photos` or `./Photos` therefore never matches any file. `InDir` then falls back to `mtime` order without any error, and the copy *inside* the chosen directory can end up in `remove`. `ScanSpec::strategy(Strategy::InDir)` without `keep_dir` is also accepted and quietly behaves like `Oldest`. Keep-one still holds, so no data is lost today. But the pipeline is now the single entry point that phase 2 clean will execute, and a plan that contradicts the user's explicit keep choice is a correctness bug in that plan.
**Fix:** Normalise and validate in the pipeline, the same way roots are handled:
```rust
fn plan(spec: &ScanSpec, groups: &[Group], stats: Stats, errors: u64) -> ScanOutcome {
    let keep_dir = spec.keep_dir.as_deref().and_then(crate::safety::absolutize);
    let keeper = Keeper::new(spec.strategy, keep_dir);
    // ...
}
```
In `run()`, also reject `Strategy::InDir` with `keep_dir == None` before the walk starts, by adding a `PipelineError` variant or a `ScanError` usage variant. Add a pipeline test with a relative keep dir.

### WR-02: The observer runs while every hashing worker waits on the same mutex

**File:** `crates/twins-core/src/group/find.rs:323-339`; contract in `crates/twins-core/src/observe.rs:158-161`
**Issue:** `compute_keys` takes the `done` mutex and calls `opts.progress(...)` while holding it. In the pipeline that call goes to `observer.on_event`, which runs the shell's code. Every hashing worker must take the same lock after each file, so hashing throughput is capped by observer latency. If the observer blocks, all of hashing stops. Examples: a stderr TTY paused with XOFF, a bounded IPC channel to the future Tauri UI, or a slow write. The `Observer` documentation says the opposite. It says `on_event` is "called from worker threads, possibly concurrently", which tells implementors that core will not serialize them. The lock exists only to make `done` strictly monotonic for observers.
**Fix:** Pick one approach and make the documentation match it:
- (a) Keep the lock, but put the cheap admission decision (a throttle check) in core before the user callback runs, and state in `Observer` that progress calls are serialized and that a blocking observer stalls hashing.
- (b) Use `AtomicU64::fetch_add` for `done` and call `progress` outside any lock. Observers then take the max of `done`, which `TerminalObserver` can do trivially.

Either way, add a test that uses a deliberately slow observer, to pin down the intended behaviour.

### WR-03: `Throttle`'s concurrency guarantees are only true for serialized callers

**File:** `crates/twins-core/src/observe.rs:182-185`, `crates/twins-core/src/observe.rs:203-205`, `crates/twins-core/src/observe.rs:234-262`
**Issue:** The type is documented as lock-free and callable "for every file" from hashing threads. It promises that a zero interval "forwards everything" and that it "never reorders the events it forwards". Neither promise holds under concurrent `Progress` callers:
- `admit()` does load, then compare, then CAS. With `interval == 0`, two concurrent callers both see `due == true`, and the loser's `compare_exchange` fails, so its event is dropped.
- Forwarding (`self.inner.on_event(event)`) happens after the CAS, outside any ordering. A non-final `Progress{done: 5}` admitted just before the final `Progress{done: total}` can reach `inner` after it, which leaves a UI showing `5 / total` once the stage has finished.

Today this works only because `find` serializes every `Progress` under the `done` mutex (WR-02) and the walk reports from one thread. If WR-02 is fixed with option (b), or the app feeds `Throttle` from several threads, both problems appear.
**Fix:** One option is to document the precondition ("`Progress` events must be delivered serially; `Throttle` does not order concurrent callers") and drop the "lock-free" and "zero forwards everything" claims. The other is to guard admit-and-forward with a small `Mutex<u64>` instead of the CAS. That is cheap at one lock per event, and the throttle then keeps its documented guarantees on its own.

### WR-04: Fatal glob errors print the parser message twice

**File:** `crates/twins-cli/src/run.rs:115-120`, `crates/twins-core/src/scan/mod.rs:34-41`; masked by `crates/twins-cli/tests/cli_test.rs:301-303`
**Issue:** `ScanError::Glob` formats `{source}` into its own message and also marks `source` as `#[source]`. `describe()` joins the whole `err.chain()`, so the inner error appears twice. I reproduced it:
```
twins: invalid exclude pattern "[": error parsing glob '[': unclosed character class; missing ']': error parsing glob '[': unclosed character class; missing ']'
```
The new characterization test `usage_errors_exit_2_with_the_same_message` uses `starts_with` for this one case, so the duplication is never locked in or caught.
**Fix:** Remove `{source}` from the `Glob` display string (`#[error("invalid exclude pattern {pattern:?}")]`) so the chain adds it once. Alternatively, have `describe` skip a cause whose text the previous message already ends with. Then make the test assert the exact stderr.

## Info

### IN-01: A second Ctrl+C leaves the progress line on the terminal

**File:** `crates/twins-cli/src/run.rs:67-73`
**Issue:** `register_conditional_shutdown` calls `_exit(130)` from the signal handler, so `Finished` never fires and `\r\x1b[K` is never written. The shell prompt then appears after a stale `[4/5] full hash  12 / 40`. There is also a small window between the two `register` calls where a SIGINT is swallowed: the default action has already been replaced, but the flag action is not registered yet.
**Fix:** This is acceptable as an escape hatch. A comment noting the leftover line would help, or the handler could write `\r\x1b[K` with a raw `write(2)` before exiting, if you want to use `signal_hook::low_level` to do it.

### IN-02: The "files could not be read" count includes things that are not unreadable files

**File:** `crates/twins-cli/src/run.rs:50-53`, `crates/twins-core/src/pipeline.rs:145-149`, `crates/twins-core/src/pipeline.rs:247`
**Issue:** `ScanOutcome::errors` adds walk errors, which include unreadable *directories*, and `GroupError::HashCollision`, which is a readable file whose content differs. The CLI reports all of them as "N files could not be read", including the ungrammatical "1 files".
**Fix:** Count collisions separately, or reword the message to "N entries skipped". Pluralize correctly.

### IN-03: Verbose skip lines print the path twice

**File:** `crates/twins-cli/src/progress.rs:94-98`
**Issue:** `HashError`, `walkdir::Error` and `GroupError::HashCollision` already include the path in their `Display`. The output reads `skip /x/b: open /x/b: Permission denied (os error 13)`. This is pre-existing, but `Event::FileSkipped { path, reason }` now fixes this shape in the public event contract for the app.
**Fix:** Build `reason` from the underlying `io::Error` and the op only, without the path, when emitting `FileSkipped` in `pipeline::skipped`.

### IN-04: Walk progress freezes during the stat phase

**File:** `crates/twins-core/src/scan/walk.rs:128-138`, `crates/twins-core/src/pipeline.rs:281-282`
**Issue:** `Progress` is emitted only while listing. The parallel `lstat` pass over every listed file emits nothing, so on a large home folder the line sits at `[1/4] walk  N files` for the whole stat phase. The comment calling listing "the slow part" is unverified. walkdir takes the file type from `d_type`, so `lstat` is per-file work of similar cost.
**Fix:** Also report from `handle_file`, for example `done = candidates + skipped` against `total = Some(files.len())` per root, or document the freeze.

### IN-05: Exit code 2 is used for errors that are not usage errors

**File:** `crates/twins-cli/src/run.rs:87-103`, `crates/twins-core/src/scan/walk.rs:113-122`
**Issue:** Every `PipelineError::Scan` maps to 2, including `ScanError::Io`. That covers EACCES or EIO when stat-ing a root, and a failure to build the *walk* thread pool. A failure to build the *hash* pool maps to 1. CLAUDE.md defines 2 as usage errors and 1 as I/O.
**Fix:** Map `ScanError::Io(_)` to 1, or give the walk pool failure its own variant.

### IN-06: The SIGINT tests never run in CI

**File:** `crates/twins-cli/tests/cli_test.rs:399-442`, `.github/workflows/ci.yml:20,29`
**Issue:** The only end-to-end coverage of exit 130 and the double-Ctrl+C behaviour is `#[ignore]`, and CI never passes `--ignored`. `spawn_busy_scan` also relies on a fixed 700 ms sleep and on 32 GiB of APFS sparse files taking longer than that to hash.
**Fix:** Add a separate CI step, `cargo test -p twins-cli --test cli_test -- --ignored sigint`, possibly allowed to fail. Alternatively, poll stderr or wait for a readiness marker instead of a fixed sleep.

### IN-07: Small API and documentation inconsistencies in the new public surface

**File:** `crates/twins-core/src/observe.rs:102-111`, `crates/twins-core/src/pipeline.rs:160-172`, `crates/twins-core/src/pipeline.rs:280`, `crates/twins-core/src/group/find.rs:47-48`
**Issue:**
- `Outcome` is not `#[non_exhaustive]`, unlike `Stage` and `Event`.
- `PipelineError::Scan` is a public tuple variant, so `PipelineError::Scan(ScanError::Cancelled)` can be constructed despite the "never holds" doc, and `is_cancelled()` would return false for it.
- `walk_stage` silently replaces any `cancel` flag the caller set on the `scan::Options` it passed to `ScanSpec::new`.
- The stage-start marker is documented as "sent from the calling thread", but it is sent from a pool worker inside `pool.install`.

**Fix:** Mark `Outcome` `#[non_exhaustive]`. Document that the token overrides `scan::Options::cancel`. Correct the thread wording in `find.rs`.

### IN-08: The project docs still say Rust 1.85 after the MSRV bump

**File:** `Cargo.toml:10`, `.github/workflows/ci.yml:22-29`
**Issue:** `rust-version` went from 1.85 to 1.90, which the code needs: let-chains in `progress.rs:87`, `is_multiple_of` in `human.rs:71`, and the clippy 1.90 lint in `report.rs`. `.claude/CLAUDE.md` still says "Rust 1.85+" and "rust 1.85" in several places.
**Fix:** Update CLAUDE.md, and the README if it states an MSRV.

---

_Reviewed: 2026-10-02T20:47:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
