---
phase: 01-core-pipeline-refactor
reviewed: 2026-10-02T21:02:00Z
depth: standard
iteration: 2
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
  warning: 1
  info: 9
  total: 10
status: issues_found
---

# Phase 01: Code Review Report (iteration 2)

**Reviewed:** 2026-10-02T21:02:00Z
**Depth:** standard
**Files Reviewed:** 23
**Status:** issues_found

## Summary

This pass re-reviews the phase after the iteration-1 fixes (fc41ba7 WR-01, 3ae882f WR-02, 94462b9 WR-03, d336d52 WR-04). It concentrates on the two concurrency changes: the `Ticker` in `group/find.rs` and the `Mutex<Gate>` in `Throttle`.

Gates on the current tree: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all pass. I ran `group_test` and `observe_test` 30 times in a row with no failures.

Status of the iteration-1 warnings:

- **WR-02 (Ticker): fixed, no regression found.** I traced the protocol under the C++/Rust SC model:
  - A worker whose claim CAS fails read `busy == true`. That read lies between the holder's claim and the holder's `store(false)` in the single total order, so the holder's post-release `done.load` sees that worker's `fetch_add`. No count is lost.
  - `reported` is accessed `Relaxed`, but every hand-off goes through the `SeqCst` (release/acquire) `busy` store and CAS, so the next holder reads the previous holder's value.
  - Only one thread holds the slot at a time, so `on_progress` is never called concurrently and `done` rises by one per call.
  - `par_iter().collect()` joins the holder before the next stage's `done == 0` marker, so stages never interleave.
  - A panicking callback leaves `busy` set, but the panic propagates out of `find`, so nothing hangs.
  - The new blocked-callback test is bounded by a 10 s deadline and uses an explicit 4-thread pool, so it fails rather than hangs on slow CI.
- **WR-03 (Throttle): fixed, no regression found.** Admission and forwarding of `Progress` and `StageStarted` now happen under one lock. The stale-`done` filter only applies within a stage and resets on `StageStarted`. In the pipeline, walk counts are cumulative across roots (`Counters` is shared), so they never look stale. Size grouping (`done == total`) and the Ticker's final `(total, total)` always pass. `FileSkipped` and `Finished` bypass the lock, as documented.
- **WR-04: fixed.** `describe()` drops a cause only when the message already ends with it. This covers `ScanError::Glob`, `FsError::Io` and `FindError::ThreadPool`, and the CLI test now asserts the exact stderr.
- **WR-01: only partly fixed.** Relative and `.`/`..` keep dirs now resolve. But matching is still a lexical `starts_with` between two paths that can come from different places, and the plan still falls back silently. I reproduced this against the current tree, so the warning is carried forward as WR-01 below.

All eight iteration-1 Info findings are still valid and are carried forward, with line numbers updated. One new Info finding (IN-09) covers the Ticker documentation overstating its throughput guarantee.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: `in-dir` still silently keeps the wrong copy when the keep dir's spelling differs from the walk's (symlinked cwd, case, `/private`)

**File:** `crates/twins-core/src/pipeline.rs:273-283`; matching in `crates/twins-core/src/group/keep.rs` (`Keeper::in_dir`, `path.starts_with(d)`)
**Issue:** The WR-01 fix sends the keep dir through `safety::absolutize`. That function is purely lexical, and its relative case joins `std::env::current_dir()`. On macOS, `getcwd` returns the *physical* path, so `/tmp/x` becomes `/private/tmp/x`. Walk paths, on the other hand, keep the root exactly as the caller spelled it. `Path::starts_with` also compares bytes, while the default APFS volume is case-insensitive.

So any of these spellings still matches no file. The keeper then falls back to mtime without any error, and the copy *inside* the chosen directory is put in `remove`:

- a relative keep dir typed after `cd`-ing through a symlink (`/tmp`, `/var`, any symlinked folder);
- a keep dir with different letter case (`~/photos` for `~/Photos`);
- `/private/tmp/...` against a root given as `/tmp/...`;
- a typo, or a directory outside every root.

I reproduced this against the current tree. Root `/tmp/twins-kd-probe`, with the newer copy in `Keep/b.bin`:
```
cwd = /private/tmp/twins-kd-probe
keep_dir=Keep                               -> keep /tmp/twins-kd-probe/a.bin   (wrong)
keep_dir=/tmp/twins-kd-probe/keep           -> keep /tmp/twins-kd-probe/a.bin   (wrong)
keep_dir=/private/tmp/twins-kd-probe/Keep   -> keep /tmp/twins-kd-probe/a.bin   (wrong)
keep_dir=/tmp/twins-kd-probe/Keep           -> keep /tmp/twins-kd-probe/Keep/b.bin
```
Keep-one still holds, so no data is lost today. But this pipeline is the single entry point that phase 2 clean will execute, and a plan that contradicts the user's explicit keep choice without saying so is the exact bug WR-01 described. The new test only covers spellings that agree lexically: it builds the relative path from `current_dir()` and the tree path the same way, so it cannot catch this.
**Fix:** Resolve the keep dir against the filesystem and map it onto a root's spelling, or fail loudly:
```rust
fn keeper(spec: &ScanSpec) -> Result<Keeper, PipelineError> {
    if spec.strategy != Strategy::InDir {
        return Ok(Keeper::new(spec.strategy, None));
    }
    let dir = spec.keep_dir.as_deref().ok_or(PipelineError::MissingKeepDir)?;
    let real = std::fs::canonicalize(dir)
        .map_err(|_| PipelineError::KeepDir(dir.to_path_buf()))?;
    // Re-express the keep dir in the walk's spelling of the root that holds it.
    for root in spec.walk_options().roots() {
        let abs = safety::absolutize(root).ok_or_else(|| PipelineError::KeepDir(dir.to_path_buf()))?;
        if let Ok(real_root) = std::fs::canonicalize(&abs)
            && let Ok(rest) = real.strip_prefix(&real_root)
        {
            return Ok(Keeper::new(Strategy::InDir, Some(abs.join(rest))));
        }
    }
    Err(PipelineError::KeepDirOutsideRoots(dir.to_path_buf())) // new usage variant -> exit 2
}
```
On macOS, `realpath` returns the on-disk case, so this also fixes the case mismatch. Add pipeline tests for a different-case keep dir, a keep dir given through `/private/tmp` while the root uses `/tmp`, and a keep dir outside every root.

## Info

### IN-01: A second Ctrl+C leaves the progress line on the terminal

**File:** `crates/twins-cli/src/run.rs:67-73`
**Issue:** Still valid. `register_conditional_shutdown` calls `_exit(130)` from the signal handler, so `Finished` never fires and `\r\x1b[K` is never written. The shell prompt then appears after a stale `[4/5] full hash  12 / 40`. There is also a small window between the two `register` calls in which a SIGINT is swallowed: the default action is already replaced, but the flag action is not registered yet.
**Fix:** Add a comment that accepts the leftover line. Alternatively, write `\r\x1b[K` with a raw `write(2)` from a `signal_hook::low_level` handler before exiting.

### IN-02: The "files could not be read" count includes things that are not unreadable files

**File:** `crates/twins-cli/src/run.rs:50-53`, `crates/twins-core/src/pipeline.rs:148-152`, `crates/twins-core/src/pipeline.rs:265`
**Issue:** Still valid. `ScanOutcome::errors` adds walk errors, which include unreadable *directories*, and `GroupError::HashCollision`, which is a readable file whose content differs. The CLI reports all of them as "N files could not be read", including the ungrammatical "1 files".
**Fix:** Count collisions separately, or reword the message to "N entries skipped". Pluralize correctly.

### IN-03: Verbose skip lines print the path twice

**File:** `crates/twins-cli/src/progress.rs:94-98`, `crates/twins-core/src/pipeline.rs:301-306`
**Issue:** Still valid. The `Display` of `HashError`, `walkdir::Error` and `GroupError::HashCollision` already includes the path, so the output reads `skip /x/b: open /x/b: Permission denied (os error 13)`. `Event::FileSkipped { path, reason }` now fixes this shape in the public event contract for the app.
**Fix:** When `pipeline::skipped` emits `FileSkipped`, build `reason` from the underlying `io::Error` and the op only, without the path.

### IN-04: Walk progress freezes during the stat phase

**File:** `crates/twins-core/src/scan/walk.rs:128-138`, `crates/twins-core/src/scan/walk.rs:195`, `crates/twins-core/src/pipeline.rs:316-317`
**Issue:** Still valid. `Progress` is emitted only while listing. The parallel `lstat` pass emits nothing, so on a large root the line stays at `[1/4] walk  N files` for the whole stat phase. The comment calling listing "the slow part" is unverified: walkdir takes the file type from `d_type`, so `lstat` is per-file work of similar cost.
**Fix:** Also report from `handle_file`, or document the freeze and remove the "slow part" claim.

### IN-05: Exit code 2 is used for errors that are not usage errors

**File:** `crates/twins-cli/src/run.rs:92`, `crates/twins-core/src/scan/walk.rs:113-122`
**Issue:** Still valid. `PipelineError::Scan(_)` maps to 2, and that includes `ScanError::Io`: EACCES or EIO when stat-ing a root, and a failure to build the *walk* thread pool, which is wrapped as `ScanError::Io`. A failure to build the *hash* pool (`PipelineError::Find`) maps to 1. CLAUDE.md defines 2 as usage errors and 1 as I/O.
**Fix:** Map `PipelineError::Scan(ScanError::Io(_))` to 1, or give the walk pool failure its own variant.

### IN-06: The SIGINT tests never run in CI

**File:** `crates/twins-cli/tests/cli_test.rs:376-442`, `.github/workflows/ci.yml:20,29`
**Issue:** Still valid. The only end-to-end coverage of exit 130 and the double-Ctrl+C behaviour is `#[ignore]`. Neither the `test` job nor the new `msrv` job passes `--ignored`. `spawn_busy_scan` also relies on a fixed 700 ms sleep (line 385).
**Fix:** Add a CI step `cargo test -p twins-cli --test cli_test -- --ignored sigint`, possibly allowed to fail. Alternatively, wait for a readiness marker on stderr instead of sleeping.

### IN-07: Small API and documentation inconsistencies in the new public surface

**File:** `crates/twins-core/src/observe.rs:102-111`, `crates/twins-core/src/pipeline.rs:162-182`, `crates/twins-core/src/pipeline.rs:315`, `crates/twins-core/src/group/find.rs:47-48`, `crates/twins-core/src/group/find.rs:143-145`
**Issue:** Still valid. Each item below needs a small fix:
- `Outcome` is not `#[non_exhaustive]`, unlike `Stage` and `Event`. `PipelineError` is not `#[non_exhaustive]` either, and it has just gained `MissingKeepDir` and `KeepDir`, with phase 2 set to add more.
- `PipelineError::Scan` is a public tuple variant, so `PipelineError::Scan(ScanError::Cancelled)` can be built despite the "never holds" doc, and `is_cancelled()` would return false for it.
- `walk_stage` silently replaces any `cancel` flag set on the `scan::Options` given to `ScanSpec::new`.
- The stage-start marker is documented as made "from the calling thread" in both the `Progress` and `Options::on_progress` docs. It is actually sent from a pool worker inside `pool.install` (`compute_keys` and `find_in_pool` run in the pool).

**Fix:**
- Mark `Outcome` and `PipelineError` `#[non_exhaustive]`.
- Document that the token overrides `scan::Options::cancel`.
- Correct the thread wording, for example to "before any worker reports for that stage".

### IN-08: The project docs still say Rust 1.85 after the MSRV bump

**File:** `Cargo.toml:10`, `.github/workflows/ci.yml:22-29`, `.claude/CLAUDE.md:20,33,39,89`
**Issue:** Still valid. `rust-version` is 1.90, which the code needs (let-chains, `is_multiple_of`, clippy 1.90). `.claude/CLAUDE.md` still says "Rust 1.85+" in four places.
**Fix:** Update CLAUDE.md, and the README if it states an MSRV.

### IN-09: The Ticker documentation says a slow callback never delays hashing, but the reporting worker stops hashing for as long as others keep it busy

**File:** `crates/twins-core/src/group/find.rs:147-153`, `crates/twins-core/src/group/find.rs:398-419`, `crates/twins-core/src/observe.rs:166-171`
**Issue:** The behaviour is correct, but the claim "a slow callback therefore delays the progress it reports, not the hashing" overstates it.
- The slot holder reports *every* pending count, one callback call each. After it releases the slot, it re-claims whenever `done` has moved.
- With a slow observer and other workers ticking faster than it reports, one worker can stay stuck as the reporter for the whole stage. Hashing parallelism drops by one worker: by half with `--jobs 2`, and to zero hashing progress during each callback with `--jobs 1`.
- The stage also waits for the whole backlog to drain, at one callback per file.

In the CLI the callback is `Throttle`, which is cheap, so this does not matter there. It matters for the app's observer, which the docs are written for.
**Fix:** Reword the docs to say that a slow callback costs at most one hashing worker, and that the stage waits until every count has been delivered. Optionally, let the holder coalesce the backlog by reporting only the newest `done`. That would mean relaxing the "rises by exactly one" contract, which `Throttle` does not need.

---

_Reviewed: 2026-10-02T21:02:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
