---
phase: 01-core-pipeline-refactor
fixed_at: 2026-10-02T21:08:08Z
review_path: .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
iteration: 2
findings_in_scope: 1
fixed: 1
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-10-02T21:08:08Z
**Source review:** .planning/workstreams/default/phases/01-core-pipeline-refactor/01-REVIEW.md
**Iteration:** 2

**Summary:**
- Findings in scope: 1 (WR-01. The fix scope is critical_warning, so the 9 Info findings are out of scope.)
- Fixed: 1
- Skipped: 0

**Verification:** every gate ran in the main checkout (`/Users/ayoub/projects/twins`, branch `main`, no worktree), on the final tree:
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` all pass.
- The two spelling regression tests were run against the pre-fix source and failed there. They keep `a.bin`, the wrong copy, instead of `keep/b.bin`.

## Iteration 1 (summary)

Four warnings were fixed in iteration 1, and the iteration-2 review confirmed three of them with no regression:
- WR-01 keep dir absolutized, `fc41ba7`. Only partly fixed; finished in this iteration.
- WR-02 `Ticker` for hash progress, `3ae882f`.
- WR-03 `Throttle` behind a `Mutex<Gate>`, `94462b9`.
- WR-04 a repeated error source printed once, `d336d52`.

## Fixed Issues

### WR-01: `in-dir` still silently keeps the wrong copy when the keep dir's spelling differs from the walk's (symlinked cwd, case, `/private`)

**Files modified:** `crates/twins-core/src/pipeline.rs`, `crates/twins-cli/src/run.rs`, `crates/twins-core/tests/pipeline_test.rs`
**Commit:** e8c1c54
**Status:** fixed: requires human verification (plan logic: which copy survives)
**Applied fix:**
- `pipeline::keeper` follows the review's proposed fix. Before the walk, it resolves the keep dir on disk: `safety::absolutize` (the same lexical step the roots get), then `fs::canonicalize`. On macOS, `realpath` returns each component in its stored case; I checked this on this machine.
- The keep dir must be a directory. A file fails with `NotADirectory`.
- For each root, in the order given, the keeper canonicalizes the walk's spelling of the root (`absolutize(root)`). If the resolved keep dir lies under it, the keeper rebuilds the dir as `walked_root.join(rest)`. `Keeper` then matches it lexically against walk paths that use the same spelling. This handles:
  - `/private/var/...` against `/var/...`;
  - a root or keep dir reached through a symlink;
  - a symlinked subdirectory inside the root;
  - a different letter case;
  - a relative keep dir after `cd`-ing through a symlink.
- The run now fails loudly before the walk instead of falling back to mtime:
  - New `PipelineError::KeepDirOutsideRoots(path)`: the keep dir lies outside every root. This includes a symlink inside a root that points outside it, which the walk never follows. Exit code 2.
  - `PipelineError::KeepDir` is now `{ path, source: io::Error }`. It covers a keep dir that does not exist, is not a directory, or cannot be reached. Exit code 2 for `NotFound` and `NotADirectory`, 1 otherwise (for example `PermissionDenied`, or no current directory).
  - A root that cannot be canonicalized before a match fails right away as `PipelineError::Scan(ScanError::Io { op: "resolve", .. })`. It is never skipped, because skipping could plan with a keep dir that matches no file.
- Docs updated: `ScanSpec::keep_dir`, the `# Errors` section of `scan`, the `keeper` comment, and the `exit_code` doc.
- New core tests in `pipeline_test.rs`:
  - `in_dir_matches_a_keep_dir_spelled_differently_from_the_root` covers five (root, keep dir) spellings:
    - root through a symlink alias, keep dir given directly;
    - root through the alias, keep dir in canonical `/private` form;
    - root direct, keep dir through the alias;
    - root direct, keep dir in canonical `/private` form;
    - keep dir through a symlinked subdirectory inside the root.
    It fails on the pre-fix code.
  - `in_dir_matches_a_keep_dir_in_a_different_case`: uses `KEEP` for `keep`. It detects the volume at run time: on a case-insensitive volume it asserts that `keep/b.bin` is kept, and on a case-sensitive one that the run fails with `NotFound`. It fails on the pre-fix code.
  - `in_dir_rejects_a_keep_dir_outside_every_root`: a separate temp dir, and an in-root symlink to it. Asserts that only `Finished` is emitted.
  - `in_dir_rejects_a_keep_dir_that_is_not_a_directory`: a missing path and a regular file.
  - `in_dir_keeps_the_copy_under_a_keep_dir_in_the_second_root`.
  - `in_dir_fails_on_a_root_it_cannot_resolve`.
  - The iteration-1 test `in_dir_keeps_the_copy_under_an_unnormalised_keep_dir` now uses the shared assertion helper.
- New CLI unit tests: `exit_code_maps_unusable_keep_dirs_to_2` and `exit_code_maps_an_unreachable_keep_dir_to_1`.
- Notes for the reviewer:
  - When several roots resolve to overlapping real directories but are not lexically nested, such as `/tmp/x` and `/private/tmp/x`, the walk visits the files twice. That behavior predates this fix. The keep dir is mapped onto the first such root only. The keeper is still a file in the keep dir, and copies with the same identity are never removed.
  - A keep dir inside a root but under an excluded path (a glob, `node_modules`, `~/Library`) still matches nothing and falls back without an error. The review did not raise this, so it was left as is.
  - The CLI does not yet expose `--keep`/`in-dir`. The new behavior is reachable through the core API, which phase 2's clean will use.

---

_Fixed: 2026-10-02T21:08:08Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 2_
