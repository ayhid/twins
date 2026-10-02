---
phase: 01-core-pipeline-refactor
reviewed: 2026-10-02T21:12:30Z
depth: standard
iteration: 3
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
  info: 12
  total: 13
status: issues_found
---

# Phase 01: Code Review Report (iteration 3)

**Reviewed:** 2026-10-02T21:12:30Z
**Depth:** standard
**Files Reviewed:** 23
**Status:** issues_found

## Summary

This pass checks the iteration-2 WR-01 fix (e8c1c54). The fix makes `pipeline::keeper` canonicalize the keep dir and map it onto the walk's spelling of the first root that contains it. It also adds `KeepDirOutsideRoots` and `KeepDir { path, source }`.

Gates on the current tree: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all pass.

What the fix gets right. I verified each point in the code, and the spelling cases with a scratch program built against the current `twins-core`:

- **Strategies other than in-dir are unaffected.** `keeper` returns at `pipeline.rs:298-300` before it reads `keep_dir`, so the keep dir is never resolved or required for those strategies. The CLI never sets `InDir`.
- **The spellings from iteration 2 now match:** a relative keep dir from a symlinked cwd, `/private/tmp` against `/tmp`, a symlinked root, a symlinked subdirectory and a different letter case.
- **A keep dir equal to the root works.** `walked.join("")` adds a trailing separator, which `Path::starts_with` ignores, so every file matches and the keeper falls back to the oldest copy. That is correct.
- **Exit codes are correct and tested.** `KeepDir` with `NotFound` or `NotADirectory` exits 2, other `KeepDir` errors exit 1, `KeepDirOutsideRoots` exits 2, and `Scan(Io{op:"resolve"})` exits 2, the same as the walk's own `stat` failure. `describe()` prints the `#[source]` of `KeepDir` once.
- **Unreadable roots are handled.** A root with mode `000` still canonicalizes, because `realpath` only needs search permission on the parents, so the keeper never rejects a root the walk would accept. A root that does not exist fails as `Scan(Io)` before any event other than `Finished`.
- **Overlapping roots spelled differently are safe.** For `/tmp/x` and `/private/tmp/x`, the walk visits each file twice. `Keeper::choose` filters out entries that share the keeper's identity, so the second spelling of the kept file is never listed for removal.

The fix is still incomplete. `keeper` maps the keep dir onto roots *in the order the caller gave them*, but the walk first drops any root that sits lexically inside another (`normalise_roots`, `walk.rs:274-281`). When the first matching root is one that gets dropped, and its spelling differs physically from the surviving root's (a symlink, or a different letter case), the rebuilt keep dir names a path the walk never produces. The plan then silently keeps the oldest copy and puts the copy inside the keep dir in `remove`, which is the same harm as WR-01. I reproduced this; see WR-01 below.

Three new Info findings came out of the same code path:

- **IN-10:** error precedence now differs when `InDir` is selected.
- **IN-11:** a keep dir under a path the walk skips is accepted without error.
- **IN-12:** an existing walk behaviour: a root that is a symlink nested in another root is silently never scanned.

All nine earlier Info findings are still valid and are carried forward with updated line numbers.

## Narrative Findings (AI reviewer)

## Warnings

### WR-01: The in-dir keep dir can still be mapped onto a root the walk drops, so the copy in the keep dir is planned for removal

**File:** `crates/twins-core/src/pipeline.rs:313-330`; root de-duplication in `crates/twins-core/src/scan/walk.rs:264-283`
**Issue:**
- `keeper` goes through `spec.walk_options().roots()` in the order given and returns `walked.join(rest)` for the first root whose canonical path contains the keep dir.
- The walk does not walk those roots as given. `normalise_roots` sorts the absolutized roots and drops every root that `starts_with` an earlier one *lexically*. `collect_files` uses `follow_links(false)`, so the walk names a dropped root's files after the surviving root's physical directory names.
- Suppose a dropped root is lexically nested but physically spelled differently. Two examples: `R/shortcut`, where `shortcut -> keep`, and `R/KEEP` on a case-insensitive volume. If that root comes first, the keeper's dir becomes `R/shortcut/...` or `R/KEEP/...`. No walk path starts with that, so `Keeper::in_dir` matches nothing, the strategy silently falls back to the oldest copy, and the copy inside the user's keep dir goes into `remove`.

I reproduced this on the current tree with a scratch binary linked against `twins-core`. The tree is `a.bin` (older), `keep/b.bin` (newer) and `shortcut -> keep`:

```
roots [R/shortcut, R]  keep_dir R/shortcut  -> keep R/a.bin       remove [R/keep/b.bin]   (wrong)
roots [R, R/shortcut]  keep_dir R/shortcut  -> keep R/keep/b.bin  remove [R/a.bin]
roots [R/KEEP, R]      keep_dir R/keep      -> keep R/a.bin       remove [R/keep/b.bin]   (wrong)
roots [R, R/KEEP]      keep_dir R/keep      -> keep R/keep/b.bin  remove [R/a.bin]
```

The result depends only on the order of the roots. Listing the keep dir as an extra root is a natural thing to do ("scan `~` and `~/Archive`, keep what is in `~/Archive`"), so this input is plausible. Keep-one still holds, so nothing is lost today. But this is the single entry point that phase 2's clean will execute, and the plan contradicts the user's explicit keep choice without saying so: the exact defect WR-01 has tracked since iteration 1.

The new tests miss it. `in_dir_keeps_the_copy_under_a_keep_dir_in_the_second_root` uses two disjoint roots, and none of the tests passes a root nested inside another.

**Fix:** Map the keep dir onto the roots the walk will actually walk. Have `scan` expose the normalised list, for example by making `normalise_roots` `pub(crate)` and re-exporting it as `scan::walk_roots(&Options) -> Result<Vec<PathBuf>, ScanError>`, and use it in `keeper`:

```rust
// pipeline.rs, keeper()
let roots = scan::walk_roots(spec.walk_options())?; // validates + de-duplicates like the walk
for walked in &roots {
    let real_root = std::fs::canonicalize(walked).map_err(|source| {
        ScanError::Io(FsError::Io { op: "resolve", path: walked.clone(), source })
    })?;
    if let Ok(rest) = real.strip_prefix(&real_root) {
        return Ok(Keeper::new(Strategy::InDir, Some(walked.join(rest))));
    }
}
Err(PipelineError::KeepDirOutsideRoots(dir.to_path_buf()))
```

This also fixes IN-10, because the roots are then validated before the keep dir checks. Add a pipeline test with roots `[t.path("shortcut"), t.root()]` and `[t.path("KEEP"), t.root()]` (the second only on a case-insensitive volume) that asserts `keep/b.bin` survives.

## Info

### IN-01: A second Ctrl+C leaves the progress line on the terminal

**File:** `crates/twins-cli/src/run.rs:67-73`
**Issue:** Still valid. `register_conditional_shutdown` calls `_exit(130)` from the signal handler, so `Finished` never fires and `\r\x1b[K` is never written. The shell prompt then appears after a stale progress line such as `[4/5] full hash  12 / 40`. There is also a small window between the two `register` calls in which a SIGINT is swallowed.
**Fix:** Add a comment that accepts the leftover line, or write `\r\x1b[K` with a raw `write(2)` from a `signal_hook::low_level` handler before exiting.

### IN-02: The "files could not be read" count includes things that are not unreadable files

**File:** `crates/twins-cli/src/run.rs:50-53`, `crates/twins-core/src/pipeline.rs:152-156`, `crates/twins-core/src/pipeline.rs:283`
**Issue:** Still valid. `ScanOutcome::errors` also counts walk errors on unreadable *directories* and `GroupError::HashCollision`, which is a readable file whose content differs. The CLI reports all of them as "N files could not be read", including the ungrammatical "1 files".
**Fix:** Count collisions separately, or reword the message to "N entries skipped", and pluralize correctly.

### IN-03: Verbose skip lines print the path twice

**File:** `crates/twins-cli/src/progress.rs:98`, `crates/twins-core/src/pipeline.rs:358-363`
**Issue:** Still valid. The `Display` of `HashError`, `walkdir::Error` and `GroupError::HashCollision` already includes the path, so a line reads `skip /x/b: open /x/b: Permission denied (os error 13)`. `Event::FileSkipped { path, reason }` now locks this shape into the public event contract for the app.
**Fix:** In `pipeline::skipped`, build `reason` from the op and the underlying `io::Error` only, without the path.

### IN-04: Walk progress freezes during the stat phase

**File:** `crates/twins-core/src/scan/walk.rs:128-138`, `crates/twins-core/src/scan/walk.rs:195`, `crates/twins-core/src/pipeline.rs:373-374`
**Issue:** Still valid. `Progress` is emitted only while directories are listed. The parallel `lstat` pass emits nothing, so on a large root the line stays at `[1/4] walk  N files` for the whole stat phase. The comment calling listing "the slow part" has not been checked.
**Fix:** Also report from `handle_file`, or document the freeze and remove the "slow part" claim.

### IN-05: Exit code 2 is used for errors that are not usage errors

**File:** `crates/twins-cli/src/run.rs:93-95`, `crates/twins-core/src/scan/walk.rs:113-122`, `crates/twins-core/src/pipeline.rs:319-325`
**Issue:** Still valid, and widened by the fix. `PipelineError::Scan(_)` maps to 2. That includes these I/O failures:
- `ScanError::Io` from EACCES or EIO on a root;
- a failure to build the walk thread pool;
- the keeper's new `ScanError::Io { op: "resolve" }`.

`KeepDir` already distinguishes `NotFound` (2) from other I/O failures (1), so the two paths now disagree.
**Fix:** Map `PipelineError::Scan(ScanError::Io(_))` to 1, except `NotFound`, following the `KeepDir` rule. Give the walk pool failure its own variant.

### IN-06: The SIGINT tests never run in CI

**File:** `crates/twins-cli/tests/cli_test.rs:385`, `crates/twins-cli/tests/cli_test.rs:401-429`, `.github/workflows/ci.yml:20,29`
**Issue:** Still valid. The only end-to-end coverage of exit 130 and the double-Ctrl+C behaviour is marked `#[ignore]`, and neither CI job passes `--ignored`. `spawn_busy_scan` also relies on a fixed 700 ms sleep.
**Fix:** Add a CI step that runs `cargo test -p twins-cli --test cli_test -- --ignored sigint`, possibly allowed to fail. Alternatively, wait for a readiness marker on stderr instead of sleeping.

### IN-07: Small API and documentation inconsistencies in the new public surface

**File:** `crates/twins-core/src/observe.rs:104`, `crates/twins-core/src/pipeline.rs:166-198`, `crates/twins-core/src/pipeline.rs:372`, `crates/twins-core/src/group/find.rs:47-48`, `crates/twins-core/src/group/find.rs:143-145`
**Issue:** Still valid:
- `Outcome` and `PipelineError` are not `#[non_exhaustive]`. `PipelineError` gained another variant in this iteration (`KeepDirOutsideRoots`), and `KeepDir` changed from a tuple variant to a struct variant. Both are breaking changes for any downstream exhaustive `match`.
- `PipelineError::Scan` is a public tuple variant, so `Scan(ScanError::Cancelled)` can be built even though the docs say it never holds that.
- `walk_stage` silently replaces any `cancel` flag already set on the `scan::Options` passed in.
- The stage-start marker is documented as coming "from the calling thread", but it is sent from a pool worker inside `pool.install`.

**Fix:**
- Mark `Outcome` and `PipelineError` `#[non_exhaustive]`.
- Document that the token overrides `scan::Options::cancel`.
- Change the thread wording to "before any worker reports for that stage".

### IN-08: The project docs still say Rust 1.85 after the MSRV bump

**File:** `Cargo.toml:10`, `.github/workflows/ci.yml:22-29`, `.claude/CLAUDE.md:20,33,39,89`
**Issue:** Still valid. `rust-version` is 1.90, which the code needs (let-chains, `is_multiple_of`, and now `ErrorKind::NotADirectory`). `.claude/CLAUDE.md` still says 1.85 in four places.
**Fix:** Update `CLAUDE.md`, and the README if it states an MSRV.

### IN-09: The Ticker documentation says a slow callback never delays hashing, but the reporting worker stops hashing for as long as it keeps reporting

**File:** `crates/twins-core/src/group/find.rs:147-153`, `crates/twins-core/src/observe.rs:166-171`
**Issue:** Still valid. The worker that holds the reporting slot delivers every pending count, one callback call each, and claims the slot again whenever `done` has moved. With a slow observer, one worker can stay the reporter for the whole stage. That costs one hashing worker, all of the hashing with `--jobs 1`, and the stage waits for the whole backlog to drain. This does not matter for `Throttle` in the CLI, but it does for the app's observer.
**Fix:** Reword the docs to say a slow callback costs at most one hashing worker and that the stage waits until every count has been delivered. Optionally, coalesce the backlog.

### IN-10: With in-dir selected, root errors are reported as keep-dir errors

**File:** `crates/twins-core/src/pipeline.rs:264`, `crates/twins-core/src/pipeline.rs:313-330`
**Issue:** `keeper` now runs before `normalise_roots` validates the roots, so the error the user sees depends on the strategy:
- No roots gives `KeepDirOutsideRoots` instead of `ScanError::NoRoots`.
- A protected root such as `/usr/share`, a file root or a remote root, with no other root holding the keep dir, gives `"<keep>: keep directory is not inside any scanned root"` instead of `ProtectedRoot`, `NotDirectory` or `RemoteRoot`. I reproduced this for the protected-only and empty-roots cases.
- A missing protected root gives `resolve <root>: No such file or directory` instead of `protected location`.

Exit codes are unchanged (2), but the message points at the wrong argument.
**Fix:** Validate the roots first. The WR-01 fix (`scan::walk_roots` called at the top of `keeper`) does this as a side effect.

### IN-11: A keep dir under a path the walk skips is accepted, and the scan quietly does not use it

**File:** `crates/twins-core/src/pipeline.rs:85-97`, `crates/twins-core/src/pipeline.rs:287-296`, `crates/twins-core/src/scan/walk.rs:167-177`
**Issue:** A keep dir inside a root but under a path the walk skips passes every check in `keeper`. Examples: an `--exclude` glob, `node_modules`, `~/Library`, a remote mount, or a symlink such as `~/Dropbox -> ~/Library/CloudStorage/Dropbox`. Its files are never scanned. I reproduced this with `exclude(["**/keep"])`: no group was formed and the run succeeded.

This is conservative: files in the keep dir are never grouped, so they are never removed. But a copy outside the keep dir that the user expected to be removed survives as the oldest copy. The docs say an unusable keep dir "fails the run before the walk", which is not true for this case.
**Fix:** After mapping, check each component of the keep dir below the root against `Rules::skip_dir` and the Library/remote checks, and fail with a new usage variant (`KeepDirExcluded`). Otherwise, state this case in the `ScanSpec::keep_dir` and `scan` docs.

### IN-12: A root that is a symlink nested in another root is silently never scanned

**File:** `crates/twins-core/src/scan/walk.rs:274-281`
**Issue:** This behaviour predates the phase (14c25f6) and lies in a reviewed file. `normalise_roots` drops a root when it lexically `starts_with` another root, on the assumption that the outer walk covers it. But the walk uses `follow_links(false)`, so a nested root that is a symlink is not covered. For `twins scan ~ ~/Dropbox`, where `~/Dropbox -> ~/Library/CloudStorage/Dropbox`, the root `~/Dropbox` is dropped and its files are never scanned, with no error or skip count. The cause is the same as WR-01's.
**Fix:** De-duplicate on canonical paths, and keep the walk's spelling for the root that survives. Alternatively, drop a nested root only when `canonicalize(nested).starts_with(canonicalize(outer))`.

---

_Reviewed: 2026-10-02T21:12:30Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
